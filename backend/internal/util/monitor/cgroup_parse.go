package monitor

import (
	"math"
	"strconv"
	"strings"
)

// cgroupMemoryReading 描述一次 cgroup 内存读取结果。
// limit 为 0 表示未探测到有效限额，调用方应回退到宿主机口径。
type cgroupMemoryReading struct {
	limit        uint64
	usage        uint64
	inactiveFile uint64
}

// cgroupV1UnlimitedThreshold 是 cgroup v1 用来表示“无限制”的近似值
// （PAGE_COUNTER_MAX，见内核 page_counter.h）。
const cgroupV1UnlimitedThreshold = uint64(0x7FFFFFFFFFFFF000)

// parseCgroupV2MemoryMax 解析 cgroup v2 的 memory.max。
// "max"、空值或非法值返回 ok=false。
func parseCgroupV2MemoryMax(content string) (uint64, bool) {
	value := strings.TrimSpace(content)
	if value == "" || value == "max" {
		return 0, false
	}
	limit, err := strconv.ParseUint(value, 10, 64)
	if err != nil || limit == 0 || limit >= cgroupV1UnlimitedThreshold {
		return 0, false
	}
	return limit, true
}

// parseCgroupV1MemoryLimit 解析 cgroup v1 的 memory.limit_in_bytes。
// 超大值（表示无限制）返回 ok=false。
func parseCgroupV1MemoryLimit(content string) (uint64, bool) {
	value := strings.TrimSpace(content)
	if value == "" {
		return 0, false
	}
	limit, err := strconv.ParseUint(value, 10, 64)
	if err != nil || limit == 0 || limit >= cgroupV1UnlimitedThreshold {
		return 0, false
	}
	return limit, true
}

// parseCgroupV2CPUMax 解析 cgroup v2 的 cpu.max，格式为 "<quota> <period>"。
// quota 为 "max" 或非正数时返回 ok=false；否则返回 quota/period 的核心数。
func parseCgroupV2CPUMax(content string) (float64, bool) {
	fields := strings.Fields(content)
	if len(fields) != 2 {
		return 0, false
	}
	period, err := strconv.ParseFloat(fields[1], 64)
	if err != nil || period <= 0 {
		return 0, false
	}
	if fields[0] == "max" {
		return 0, false
	}
	quota, err := strconv.ParseFloat(fields[0], 64)
	if err != nil || quota <= 0 {
		return 0, false
	}
	return quota / period, true
}

// parseCgroupV1CPUQuota 解析 cgroup v1 的 cpu.cfs_quota_us / cpu.cfs_period_us。
func parseCgroupV1CPUQuota(quotaContent, periodContent string) (float64, bool) {
	quota, err := strconv.ParseInt(strings.TrimSpace(quotaContent), 10, 64)
	if err != nil || quota <= 0 {
		return 0, false
	}
	period, err := strconv.ParseInt(strings.TrimSpace(periodContent), 10, 64)
	if err != nil || period <= 0 {
		return 0, false
	}
	return float64(quota) / float64(period), true
}

// parseMemoryStatValue 从 memory.stat 中读取指定键的值。
func parseMemoryStatValue(content, key string) (uint64, bool) {
	for _, line := range strings.Split(content, "\n") {
		fields := strings.Fields(line)
		if len(fields) != 2 || fields[0] != key {
			continue
		}
		value, err := strconv.ParseUint(fields[1], 10, 64)
		if err != nil {
			return 0, false
		}
		return value, true
	}
	return 0, false
}

// parseCPUSetCount 解析 cpuset 列表（如 "0-3,8,10-11"）并返回 CPU 数量。
func parseCPUSetCount(content string) (int, bool) {
	value := strings.TrimSpace(content)
	if value == "" {
		return 0, false
	}
	count := 0
	for _, part := range strings.Split(value, ",") {
		part = strings.TrimSpace(part)
		if part == "" {
			continue
		}
		if dash := strings.IndexByte(part, '-'); dash >= 0 {
			start, errStart := strconv.Atoi(strings.TrimSpace(part[:dash]))
			end, errEnd := strconv.Atoi(strings.TrimSpace(part[dash+1:]))
			if errStart != nil || errEnd != nil || end < start {
				return 0, false
			}
			count += end - start + 1
			continue
		}
		if _, err := strconv.Atoi(part); err != nil {
			return 0, false
		}
		count++
	}
	if count == 0 {
		return 0, false
	}
	return count, true
}

// parseSelfCgroup 解析 /proc/self/cgroup，返回当前进程所在 cgroup 的相对路径。
// version 为 2 时匹配 "0::<path>"；version 为 1 时匹配包含指定 controller 的行。
func parseSelfCgroup(content string, version int, controller string) string {
	for _, line := range strings.Split(content, "\n") {
		fields := strings.SplitN(strings.TrimSpace(line), ":", 3)
		if len(fields) != 3 {
			continue
		}
		if version == 2 {
			if fields[0] == "0" && fields[1] == "" {
				return normalizeCgroupPath(fields[2])
			}
			continue
		}
		for _, item := range strings.Split(fields[1], ",") {
			if item == controller {
				return normalizeCgroupPath(fields[2])
			}
		}
	}
	return "/"
}

func normalizeCgroupPath(path string) string {
	trimmed := strings.Trim(strings.TrimSpace(path), "/")
	if trimmed == "" {
		return "/"
	}
	return "/" + trimmed
}

// effectiveCores 计算容器实际可用的逻辑核数（选项 C：核数与百分比都按生效配额）。
// quota 与 cpuset 同时存在时取较小值；都缺失时退回宿主机核数，并 clamp 到 [1, hostCores]。
func effectiveCores(hostCores int, quotaCores float64, quotaOK bool, cpusetCores int, cpusetOK bool) int {
	if hostCores < 1 {
		hostCores = 1
	}
	effective := hostCores
	switch {
	case quotaOK && cpusetOK:
		effective = int(math.Ceil(quotaCores))
		if cpusetCores < effective {
			effective = cpusetCores
		}
	case quotaOK:
		effective = int(math.Ceil(quotaCores))
	case cpusetOK:
		effective = cpusetCores
	}
	if effective < 1 {
		effective = 1
	}
	if effective > hostCores {
		effective = hostCores
	}
	return effective
}

// quotaPercent 把宿主机每逻辑核占用（0-100）换算成相对配额的总体占用（0-100）。
// 各核占用之和即为消耗的核心数乘以 100，除以生效核数即得配额占比。
func quotaPercent(usages []float64, effective int) float64 {
	if len(usages) == 0 || effective < 1 {
		return 0
	}
	var sum float64
	for _, usage := range usages {
		sum += usage
	}
	percent := sum / float64(effective)
	if percent > 100 {
		percent = 100
	}
	if percent < 0 {
		percent = 0
	}
	return percent
}
