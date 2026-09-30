//go:build linux

package monitor

import (
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

const (
	cgroupRoot      = "/sys/fs/cgroup"
	procSelfCgroup  = "/proc/self/cgroup"
	cgroupV2Version = 2
	cgroupV1Version = 1
)

// cgroupReader 负责在 Linux 上探测 cgroup 版本并读取限额文件。
type cgroupReader struct {
	version    int
	memoryDirs []string
	cpuDirs    []string
	cpusetDirs []string
}

func newCgroupReader() cgroupReader {
	if _, err := os.Stat(filepath.Join(cgroupRoot, "cgroup.controllers")); err == nil {
		dir := filepath.Join(cgroupRoot, readSelfCgroup(cgroupV2Version, ""))
		return cgroupReader{
			version:    cgroupV2Version,
			memoryDirs: []string{dir, cgroupRoot},
			cpuDirs:    []string{dir, cgroupRoot},
			cpusetDirs: []string{dir, cgroupRoot},
		}
	}
	if _, err := os.Stat(filepath.Join(cgroupRoot, "memory", "memory.limit_in_bytes")); err == nil {
		return cgroupReader{
			version: cgroupV1Version,
			memoryDirs: []string{
				filepath.Join(cgroupRoot, "memory", readSelfCgroup(cgroupV1Version, "memory")),
				filepath.Join(cgroupRoot, "memory"),
			},
			cpuDirs: []string{
				filepath.Join(cgroupRoot, "cpu", readSelfCgroup(cgroupV1Version, "cpu")),
				filepath.Join(cgroupRoot, "cpu"),
			},
			cpusetDirs: []string{
				filepath.Join(cgroupRoot, "cpuset", readSelfCgroup(cgroupV1Version, "cpuset")),
				filepath.Join(cgroupRoot, "cpuset"),
			},
		}
	}
	return cgroupReader{}
}

// readCgroupMemory 读取当前进程所在 cgroup 的内存限额与用量。
// 未探测到有效限额（无 cgroup、无限制）时返回 ok=false。
func readCgroupMemory() (cgroupMemoryReading, bool) {
	return newCgroupReader().readMemory()
}

// readCgroupCPUQuotaCores 返回 cgroup CPU 配额对应的核心数。
func readCgroupCPUQuotaCores() (float64, bool) {
	return newCgroupReader().readCPUQuotaCores()
}

// readCgroupCPUSetCount 返回 cpuset 绑定的逻辑核数量。
func readCgroupCPUSetCount() (int, bool) {
	return newCgroupReader().readCPUSetCount()
}

func (r cgroupReader) readMemory() (cgroupMemoryReading, bool) {
	if r.version == 0 {
		return cgroupMemoryReading{}, false
	}

	var limit uint64
	var ok bool
	if r.version == cgroupV2Version {
		content, found := readFirstFile(r.memoryDirs, "memory.max")
		if !found {
			return cgroupMemoryReading{}, false
		}
		limit, ok = parseCgroupV2MemoryMax(content)
	} else {
		content, found := readFirstFile(r.memoryDirs, "memory.limit_in_bytes")
		if !found {
			return cgroupMemoryReading{}, false
		}
		limit, ok = parseCgroupV1MemoryLimit(content)
	}
	if !ok {
		return cgroupMemoryReading{}, false
	}

	usageFile, inactiveKey := "memory.current", "inactive_file"
	if r.version == cgroupV1Version {
		usageFile, inactiveKey = "memory.usage_in_bytes", "total_inactive_file"
	}
	usageContent, found := readFirstFile(r.memoryDirs, usageFile)
	if !found {
		return cgroupMemoryReading{}, false
	}
	usage, err := strconv.ParseUint(strings.TrimSpace(usageContent), 10, 64)
	if err != nil {
		return cgroupMemoryReading{}, false
	}

	reading := cgroupMemoryReading{limit: limit, usage: usage}
	if statContent, found := readFirstFile(r.memoryDirs, "memory.stat"); found {
		if inactive, found := parseMemoryStatValue(statContent, inactiveKey); found {
			reading.inactiveFile = inactive
		}
	}
	return reading, true
}

func (r cgroupReader) readCPUQuotaCores() (float64, bool) {
	if r.version == 0 {
		return 0, false
	}
	if r.version == cgroupV2Version {
		content, found := readFirstFile(r.cpuDirs, "cpu.max")
		if !found {
			return 0, false
		}
		return parseCgroupV2CPUMax(content)
	}
	quotaContent, found := readFirstFile(r.cpuDirs, "cpu.cfs_quota_us")
	if !found {
		return 0, false
	}
	periodContent, found := readFirstFile(r.cpuDirs, "cpu.cfs_period_us")
	if !found {
		return 0, false
	}
	return parseCgroupV1CPUQuota(quotaContent, periodContent)
}

func (r cgroupReader) readCPUSetCount() (int, bool) {
	if r.version == 0 {
		return 0, false
	}
	names := []string{"cpuset.cpus.effective", "cpuset.cpus"}
	if r.version == cgroupV1Version {
		names = []string{"cpuset.effective_cpus", "cpuset.cpus"}
	}
	for _, name := range names {
		content, found := readFirstFile(r.cpusetDirs, name)
		if !found {
			continue
		}
		if count, ok := parseCPUSetCount(content); ok {
			return count, true
		}
	}
	return 0, false
}

func readSelfCgroup(version int, controller string) string {
	content, err := os.ReadFile(procSelfCgroup)
	if err != nil {
		return "/"
	}
	return parseSelfCgroup(string(content), version, controller)
}

func readFirstFile(dirs []string, name string) (string, bool) {
	for _, dir := range dirs {
		if dir == "" {
			continue
		}
		content, err := os.ReadFile(filepath.Join(dir, name))
		if err == nil {
			return string(content), true
		}
	}
	return "", false
}
