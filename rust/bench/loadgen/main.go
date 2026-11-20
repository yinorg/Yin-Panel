// A small keep-alive load generator, written to compare the Go and Rust
// builds of the same service on the same host.
//
//   loadgen -url http://127.0.0.1:3010/api/about -n 2000 -c 32 [-pid 1234]
//           [-header "Cookie: yin_token=..."] [-json]
//
// It measures latency percentiles and throughput from the client side, and
// samples the target process's CPU time and RSS from /proc when `-pid` is set,
// so "CPU per request" is the server's own cost rather than the client's.
//
// A response only counts as a success when the transport succeeded, the body
// was read completely, the status matched, and - for API JSON - the envelope
// carried `"code": 0`. Counting only the HTTP status would let an error envelope
// (HTTP 200 with `code != 0`) inflate the throughput of a broken build.
package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"net/http"
	"os"
	"sort"
	"strconv"
	"strings"
	"sync"
	"time"
)

type sample struct {
	reason string // empty when the request succeeded
	micros int64
}

func procStat(pid string) (utime, stime uint64, rss uint64) {
	if pid == "" {
		return
	}
	data, err := os.ReadFile("/proc/" + pid + "/stat")
	if err != nil {
		return
	}
	// The comm field is parenthesised and may contain spaces, so split after
	// the last ')'. State is then field 3, utime field 14, stime field 15.
	close := strings.LastIndexByte(string(data), ')')
	if close < 0 || close+2 >= len(data) {
		return
	}
	fields := strings.Fields(string(data)[close+2:])
	if len(fields) > 12 {
		utime, _ = strconv.ParseUint(fields[11], 10, 64)
		stime, _ = strconv.ParseUint(fields[12], 10, 64)
	}
	if len(fields) > 21 {
		rss, _ = strconv.ParseUint(fields[21], 10, 64)
	}
	return
}

func procRSS(pid string) uint64 {
	if pid == "" {
		return 0
	}
	data, err := os.ReadFile("/proc/" + pid + "/status")
	if err != nil {
		return 0
	}
	for _, line := range strings.Split(string(data), "\n") {
		if strings.HasPrefix(line, "VmRSS:") {
			fields := strings.Fields(line)
			if len(fields) >= 2 {
				kb, _ := strconv.ParseUint(fields[1], 10, 64)
				return kb
			}
		}
	}
	return 0
}

// checkValidity returns "" for a good response, else why it is not counted.
func checkValidity(resp *http.Response, body []byte, expectStatus int, expectCode int) string {
	if resp.StatusCode != expectStatus {
		return fmt.Sprintf("status %d (want %d)", resp.StatusCode, expectStatus)
	}
	contentType := resp.Header.Get("Content-Type")
	if !strings.Contains(contentType, "application/json") {
		return ""
	}
	var envelope map[string]any
	if err := json.Unmarshal(body, &envelope); err != nil {
		return "invalid JSON body"
	}
	if expectCode < 0 {
		return ""
	}
	value, present := envelope["code"]
	if !present {
		return "JSON body has no envelope \"code\""
	}
	number, ok := value.(float64)
	if !ok {
		return "envelope \"code\" is not a number"
	}
	if int(number) != expectCode {
		message, _ := envelope["msg"].(string)
		if message == "" {
			return fmt.Sprintf("business code %d (want %d)", int(number), expectCode)
		}
		return fmt.Sprintf("business code %d: %s", int(number), message)
	}
	return ""
}

type report struct {
	URL          string         `json:"url"`
	Concurrency  int            `json:"concurrency"`
	Requests     int            `json:"requests"`
	OK           int            `json:"ok"`
	Failed       int            `json:"failed"`
	Failures     map[string]int `json:"failures,omitempty"`
	RPS          float64        `json:"rps"`
	P50MS        float64        `json:"p50_ms"`
	P95MS        float64        `json:"p95_ms"`
	P99MS        float64        `json:"p99_ms"`
	CPUMicrosAvg float64        `json:"cpu_us_per_request"`
	Cores        float64        `json:"cores"`
	RSSMiB       float64        `json:"rss_mib"`
	ElapsedMS    int64          `json:"elapsed_ms"`
}

func main() {
	url := flag.String("url", "", "URL to request")
	n := flag.Int("n", 2000, "total requests")
	conc := flag.Int("c", 32, "concurrent connections")
	pid := flag.String("pid", "", "server pid to sample for CPU and RSS")
	header := flag.String("header", "", `extra header, "Name: value"`)
	asJSON := flag.Bool("json", false, "print one JSON object instead of a line")
	timeout := flag.Duration("timeout", 30*time.Second, "per-request timeout")
	expectStatus := flag.Int("expect-status", 200, "status a successful request must return")
	expectCode := flag.Int("expect-code", 0, "envelope code a JSON response must carry; -1 disables the check")
	allowFailures := flag.Bool("allow-failures", false, "exit 0 even when some requests failed")
	warmup := flag.Int("warmup", 0, "unmeasured requests to send first")
	flag.Parse()
	if *url == "" {
		fmt.Fprintln(os.Stderr, "usage: loadgen -url <url> [-n N] [-c C] [-pid PID] [-header 'Name: value'] [-json]")
		os.Exit(2)
	}

	transport := &http.Transport{
		MaxIdleConns:        *conc,
		MaxIdleConnsPerHost: *conc,
		MaxConnsPerHost:     *conc,
		DisableCompression:  true,
	}
	client := &http.Client{Transport: transport, Timeout: *timeout}

	req, err := http.NewRequest(http.MethodGet, *url, nil)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	if *header != "" {
		if name, value, ok := strings.Cut(*header, ":"); ok {
			req.Header.Set(strings.TrimSpace(name), strings.TrimSpace(value))
		}
	}
	if strings.Contains(*url, "/api/login") {
		req.Method = http.MethodPost
		req.Header.Set("Content-Type", "application/json")
		body := `{"mail":"admin@yiniot.com","password":"admin@yiniot.com"}`
		req.Body = io.NopCloser(strings.NewReader(body))
		req.GetBody = func() (io.ReadCloser, error) {
			return io.NopCloser(strings.NewReader(body)), nil
		}
	}

	// send performs one request and reports whether it fully succeeded.
	send := func() sample {
		// A GET has no body and can be shared; a POST needs its own copy.
		outgoing := req
		if req.GetBody != nil {
			copied := req.Clone(context.Background())
			copied.Body, _ = req.GetBody()
			outgoing = copied
		}
		began := time.Now()
		resp, err := client.Do(outgoing)
		if err != nil {
			return sample{reason: classifyError(err)}
		}
		body, readErr := io.ReadAll(resp.Body)
		resp.Body.Close()
		if readErr != nil {
			return sample{reason: "body read failed"}
		}
		if reason := checkValidity(resp, body, *expectStatus, *expectCode); reason != "" {
			return sample{reason: reason}
		}
		return sample{micros: time.Since(began).Microseconds()}
	}

	for i := 0; i < *warmup; i++ {
		send()
	}

	utime0, stime0, _ := procStat(*pid)
	rss0 := procRSS(*pid)
	start := time.Now()

	results := make([]sample, *n)
	var wg sync.WaitGroup
	var next int64
	var mu sync.Mutex
	for worker := 0; worker < *conc; worker++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for {
				mu.Lock()
				index := next
				next++
				mu.Unlock()
				if int(index) >= *n {
					return
				}
				results[index] = send()
			}
		}()
	}
	wg.Wait()
	elapsed := time.Since(start)

	utime1, stime1, _ := procStat(*pid)
	rss1 := procRSS(*pid)

	latencies := make([]int64, 0, len(results))
	failures := map[string]int{}
	for _, result := range results {
		if result.reason != "" {
			failures[result.reason]++
			continue
		}
		latencies = append(latencies, result.micros)
	}
	sort.Slice(latencies, func(i, j int) bool { return latencies[i] < latencies[j] })
	pct := func(p float64) float64 {
		if len(latencies) == 0 {
			return 0
		}
		index := int(float64(len(latencies)-1) * p)
		return float64(latencies[index]) / 1000.0
	}
	rps := float64(len(latencies)) / elapsed.Seconds()
	// Jiffies are 1/100 s, so one unit is 10,000 µs. The cost is divided by
	// every request sent, failed ones included.
	cpuMicros := float64((utime1-utime0)+(stime1-stime0)) * 10000.0
	perRequest := 0.0
	if *n > 0 {
		perRequest = cpuMicros / float64(*n)
	}
	cores := 0.0
	if elapsed > 0 {
		cores = cpuMicros / (elapsed.Seconds() * 1e6)
	}

	outcome := report{
		URL:          *url,
		Concurrency:  *conc,
		Requests:     *n,
		OK:           len(latencies),
		Failed:       len(results) - len(latencies),
		Failures:     failures,
		RPS:          rps,
		P50MS:        pct(0.50),
		P95MS:        pct(0.95),
		P99MS:        pct(0.99),
		CPUMicrosAvg: perRequest,
		Cores:        cores,
		RSSMiB:       float64(rss1) / 1024.0,
		ElapsedMS:    elapsed.Milliseconds(),
	}
	_ = rss0

	if *asJSON {
		encoded, err := json.Marshal(outcome)
		if err != nil {
			fmt.Fprintln(os.Stderr, err)
			os.Exit(1)
		}
		fmt.Println(string(encoded))
	} else {
		fmt.Printf("%-52s c=%-2d ok=%-6d fail=%-4d rps=%8.0f p50=%6.2fms p95=%6.2fms p99=%6.2fms cpu/req=%7.1fµs cores=%5.2f rss=%5.0fMiB\n",
			*url, *conc, outcome.OK, outcome.Failed, rps, outcome.P50MS, outcome.P95MS,
			outcome.P99MS, perRequest, cores, outcome.RSSMiB)
		if len(failures) > 0 {
			keys := make([]string, 0, len(failures))
			for reason := range failures {
				keys = append(keys, reason)
			}
			sort.Strings(keys)
			for _, reason := range keys {
				fmt.Printf("  failed %d: %s\n", failures[reason], reason)
			}
		}
	}

	if outcome.Failed > 0 && !*allowFailures {
		os.Exit(1)
	}
}

func classifyError(err error) string {
	message := err.Error()
	switch {
	case strings.Contains(message, "timeout") || strings.Contains(message, "deadline exceeded"):
		return "timeout"
	case strings.Contains(message, "connection refused"):
		return "connection refused"
	case strings.Contains(message, "connection reset"):
		return "connection reset"
	case strings.Contains(message, "EOF"):
		return "connection closed early"
	default:
		return "transport error"
	}
}
