//go:build !linux

package monitor

// 非 Linux 平台没有 cgroup，始终回退到宿主机口径。

func readCgroupMemory() (cgroupMemoryReading, bool) {
	return cgroupMemoryReading{}, false
}

func readCgroupCPUQuotaCores() (float64, bool) {
	return 0, false
}

func readCgroupCPUSetCount() (int, bool) {
	return 0, false
}
