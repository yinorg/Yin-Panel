package app

import "testing"

func TestDeriveMemoryLimit(t *testing.T) {
	const oneGiB = uint64(1) << 30

	cases := []struct {
		name        string
		cgroupLimit uint64
		ratio       float64
		want        uint64
		wantOK      bool
	}{
		{"default ratio applies to the cgroup limit", oneGiB, 0, 805306368, true},
		{"explicit ratio is honoured", oneGiB, 0.5, 536870912, true},
		{"negative ratio disables the adaptation", oneGiB, -1, 0, false},
		{"tiny cgroup limit is clamped to the limit itself", 16 << 20, 0, 16 << 20, true},
		{"ratio above one never exceeds the cgroup limit", oneGiB, 2, oneGiB, true},
	}

	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			got, ok := deriveMemoryLimit(testCase.cgroupLimit, testCase.ratio)
			if ok != testCase.wantOK {
				t.Fatalf("deriveMemoryLimit(%d, %v) ok = %v, want %v", testCase.cgroupLimit, testCase.ratio, ok, testCase.wantOK)
			}
			if got != testCase.want {
				t.Fatalf("deriveMemoryLimit(%d, %v) = %d, want %d", testCase.cgroupLimit, testCase.ratio, got, testCase.want)
			}
		})
	}
}
