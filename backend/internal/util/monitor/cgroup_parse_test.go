package monitor

import (
	"math"
	"testing"
)

func TestParseCgroupV2MemoryMax(t *testing.T) {
	cases := []struct {
		name    string
		content string
		want    uint64
		ok      bool
	}{
		{"limited", "536870912\n", 536870912, true},
		{"no trailing newline", "1073741824", 1073741824, true},
		{"unlimited", "max\n", 0, false},
		{"empty", "", 0, false},
		{"zero", "0", 0, false},
		{"invalid", "abc", 0, false},
		{"huge treated as unlimited", "9223372036854771712", 0, false},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			got, ok := parseCgroupV2MemoryMax(tc.content)
			if ok != tc.ok || got != tc.want {
				t.Fatalf("parseCgroupV2MemoryMax(%q) = (%d, %v), want (%d, %v)", tc.content, got, ok, tc.want, tc.ok)
			}
		})
	}
}

func TestParseCgroupV1MemoryLimit(t *testing.T) {
	cases := []struct {
		name    string
		content string
		want    uint64
		ok      bool
	}{
		{"limited", "268435456\n", 268435456, true},
		{"unlimited sentinel", "9223372036854771712\n", 0, false},
		{"empty", "\n", 0, false},
		{"zero", "0", 0, false},
		{"invalid", "max", 0, false},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			got, ok := parseCgroupV1MemoryLimit(tc.content)
			if ok != tc.ok || got != tc.want {
				t.Fatalf("parseCgroupV1MemoryLimit(%q) = (%d, %v), want (%d, %v)", tc.content, got, ok, tc.want, tc.ok)
			}
		})
	}
}

func TestParseCgroupV2CPUMax(t *testing.T) {
	cases := []struct {
		name    string
		content string
		want    float64
		ok      bool
	}{
		{"two cores", "200000 100000\n", 2, true},
		{"half core", "50000 100000\n", 0.5, true},
		{"unlimited", "max 100000\n", 0, false},
		{"zero quota", "0 100000\n", 0, false},
		{"missing period", "200000\n", 0, false},
		{"empty", "", 0, false},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			got, ok := parseCgroupV2CPUMax(tc.content)
			if ok != tc.ok || math.Abs(got-tc.want) > 1e-9 {
				t.Fatalf("parseCgroupV2CPUMax(%q) = (%v, %v), want (%v, %v)", tc.content, got, ok, tc.want, tc.ok)
			}
		})
	}
}

func TestParseCgroupV1CPUQuota(t *testing.T) {
	cases := []struct {
		name   string
		quota  string
		period string
		want   float64
		ok     bool
	}{
		{"two cores", "200000", "100000", 2, true},
		{"unlimited quota", "-1", "100000", 0, false},
		{"zero period", "200000", "0", 0, false},
		{"invalid", "abc", "100000", 0, false},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			got, ok := parseCgroupV1CPUQuota(tc.quota, tc.period)
			if ok != tc.ok || math.Abs(got-tc.want) > 1e-9 {
				t.Fatalf("parseCgroupV1CPUQuota(%q, %q) = (%v, %v), want (%v, %v)", tc.quota, tc.period, got, ok, tc.want, tc.ok)
			}
		})
	}
}

func TestParseMemoryStatValue(t *testing.T) {
	v2 := "anon 1024\ninactive_file 4096\nactive_file 2048\n"
	if got, ok := parseMemoryStatValue(v2, "inactive_file"); !ok || got != 4096 {
		t.Fatalf("inactive_file = (%d, %v), want (4096, true)", got, ok)
	}
	v1 := "cache 8192\ntotal_inactive_file 2048\nrss 512\n"
	if got, ok := parseMemoryStatValue(v1, "total_inactive_file"); !ok || got != 2048 {
		t.Fatalf("total_inactive_file = (%d, %v), want (2048, true)", got, ok)
	}
	if _, ok := parseMemoryStatValue(v1, "missing"); ok {
		t.Fatal("missing key should not be found")
	}
}

func TestParseCPUSetCount(t *testing.T) {
	cases := []struct {
		content string
		want    int
		ok      bool
	}{
		{"0-3,8,10-11", 7, true},
		{"0", 1, true},
		{"3-1", 0, false},
		{"", 0, false},
		{"abc", 0, false},
	}
	for _, tc := range cases {
		got, ok := parseCPUSetCount(tc.content)
		if ok != tc.ok || got != tc.want {
			t.Fatalf("parseCPUSetCount(%q) = (%d, %v), want (%d, %v)", tc.content, got, ok, tc.want, tc.ok)
		}
	}
}

func TestParseSelfCgroup(t *testing.T) {
	v2 := "0::/system.slice/docker-abc.scope\n"
	if got := parseSelfCgroup(v2, 2, ""); got != "/system.slice/docker-abc.scope" {
		t.Fatalf("v2 path = %q", got)
	}
	if got := parseSelfCgroup("0::/\n", 2, ""); got != "/" {
		t.Fatalf("v2 root path = %q", got)
	}
	if got := parseSelfCgroup("nope", 2, ""); got != "/" {
		t.Fatalf("v2 missing = %q", got)
	}

	v1 := "11:memory:/docker/abc\n4:cpu:/docker/abc\n5:cpuset:/docker/abc\n"
	if got := parseSelfCgroup(v1, 1, "memory"); got != "/docker/abc" {
		t.Fatalf("v1 memory path = %q", got)
	}
	if got := parseSelfCgroup(v1, 1, "cpu"); got != "/docker/abc" {
		t.Fatalf("v1 cpu path = %q", got)
	}
	if got := parseSelfCgroup(v1, 1, "blkio"); got != "/" {
		t.Fatalf("v1 missing controller = %q", got)
	}
}

func TestEffectiveCores(t *testing.T) {
	cases := []struct {
		name     string
		host     int
		quota    float64
		quotaOK  bool
		cpuset   int
		cpusetOK bool
		want     int
	}{
		{"no limits", 64, 0, false, 0, false, 64},
		{"quota only two cores", 64, 2, true, 0, false, 2},
		{"half core quota rounds up", 64, 0.5, true, 0, false, 1},
		{"cpuset only", 64, 0, false, 4, true, 4},
		{"quota and cpuset take min", 64, 8, true, 4, true, 4},
		{"quota smaller than cpuset", 64, 2, true, 4, true, 2},
		{"clamp to host", 2, 8, true, 0, false, 2},
		{"never below one", 64, 0, false, 1, true, 1},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			got := effectiveCores(tc.host, tc.quota, tc.quotaOK, tc.cpuset, tc.cpusetOK)
			if got != tc.want {
				t.Fatalf("effectiveCores = %d, want %d", got, tc.want)
			}
		})
	}
}

func TestQuotaPercent(t *testing.T) {
	// 宿主 64 核，配额 2 核。
	hostUsages := func(perCore float64) []float64 {
		usages := make([]float64, 64)
		for i := range usages {
			usages[i] = perCore
		}
		return usages
	}

	if got := quotaPercent(hostUsages(0), 2); got != 0 {
		t.Fatalf("idle = %v, want 0", got)
	}
	// 跑满 1 核：宿主占比 1/64，换算到 2 核配额 = 50%。
	if got := quotaPercent(hostUsages(100.0/64.0), 2); math.Abs(got-50) > 0.01 {
		t.Fatalf("one core = %v, want 50", got)
	}
	// 跑满 2 核 = 100%。
	if got := quotaPercent(hostUsages(100.0*2/64.0), 2); math.Abs(got-100) > 0.01 {
		t.Fatalf("two cores = %v, want 100", got)
	}
	// 无配额时退回宿主总体占用。
	if got := quotaPercent(hostUsages(100), 64); math.Abs(got-100) > 0.01 {
		t.Fatalf("no quota full = %v, want 100", got)
	}
	// 封顶 100。
	if got := quotaPercent(hostUsages(100), 1); got != 100 {
		t.Fatalf("over quota = %v, want 100", got)
	}
	if got := quotaPercent(nil, 2); got != 0 {
		t.Fatalf("empty = %v, want 0", got)
	}
}
