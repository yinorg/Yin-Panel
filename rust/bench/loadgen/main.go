// A small keep-alive load generator, written to compare the Go and Rust
// builds of the same service on the same host.
//
//   loadgen -url http://127.0.0.1:3010/api/about -n 2000 -c 32 [-pid 1234]
//           [-header "Cookie: yin_token=..."]
//
// It measures latency percentiles and throughput from the client side, and
// samples the target process's CPU time and RSS from /proc when `-pid` is set,
// so "CPU per request" is the server's own cost rather than the client's.
package main

import (
	"context"
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
	fail  bool
	micros int64
}

func procStat(pid string) (utime, stime uint64, rss uint64) {
	data, err := os.ReadFile("/proc/" + pid + "/stat")
	if err == nil {
		fields := strings.Fields(string(data))
		if len(fields) > 23 {
			utime, _ = strconv.ParseUint(fields[13], 10, 64)
			stime, _ = strconv.ParseUint(fields[14], 10, 64)
			rss, _ = strconv.ParseUint(fields[23], 10, 64)
		}
	}
	return
}

func main() {
	url := flag.String("url", "", "URL to request")
	n := flag.Int("n", 2000, "total requests")
	conc := flag.Int("c", 32, "concurrent connections")
	pid := flag.String("pid", "", "server pid to sample for CPU and RSS")
	header := flag.String("header", "", `extra header, "Name: value"`)
	flag.Parse()
	if *url == "" {
		fmt.Fprintln(os.Stderr, "usage: loadgen -url <url> [-n N] [-c C] [-pid PID] [-header 'Name: value']")
		os.Exit(2)
	}

	transport := &http.Transport{
		MaxIdleConns:        *conc,
		MaxIdleConnsPerHost: *conc,
		MaxConnsPerHost:     *conc,
		DisableCompression:  true,
	}
	client := &http.Client{Transport: transport, Timeout: 30 * time.Second}

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
		req.Body = io.NopCloser(strings.NewReader(`{"mail":"admin@yiniot.com","password":"admin@yiniot.com"}`))
		req.GetBody = func() (io.ReadCloser, error) {
			return io.NopCloser(strings.NewReader(`{"mail":"admin@yiniot.com","password":"admin@yiniot.com"}`)), nil
		}
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
				// A request with a body cannot be reused: `GetBody` gives each
				// worker its own copy. A GET has no body and is safe to share.
				send := req
				if req.GetBody != nil {
					copied := req.Clone(context.Background())
					copied.Body, _ = req.GetBody()
					send = copied
				}
				began := time.Now()
				resp, err := client.Do(send)
				if err != nil {
					results[index] = sample{fail: true}
					continue
				}
				_, _ = io.Copy(io.Discard, resp.Body)
				resp.Body.Close()
				results[index] = sample{micros: time.Since(began).Microseconds(), fail: resp.StatusCode >= 400}
			}
		}()
	}
	wg.Wait()
	elapsed := time.Since(start)

	utime1, stime1, _ := procStat(*pid)
	rss1 := procRSS(*pid)

	var latencies []int64
	var failures int
	for _, r := range results {
		if r.fail {
			failures++
			continue
		}
		latencies = append(latencies, r.micros)
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
	cpuMicros := float64((utime1-utime0)+(stime1-stime0)) * 10000.0 // jiffies (1/100 s) to µs
	perRequest := cpuMicros / float64(*n)
	cores := cpuMicros / (elapsed.Seconds() * 1e6)

	fmt.Printf("%-52s c=%-2d rps=%8.0f p50=%6.2fms p99=%6.2fms cpu/req=%7.1fµs cores=%5.2f rss=%5.0fMiB%s\n",
		*url, *conc, rps, pct(0.50), pct(0.99), perRequest, cores, float64(rss1)/1024.0,
		failNote(failures, len(latencies)))
	_ = rss0
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

func failNote(failures, total int) string {
	if failures == 0 {
		return ""
	}
	return fmt.Sprintf(" FAIL=%d/%d", failures, total)
}
