package monitor

import (
	"time"

	"github.com/shirou/gopsutil/v3/cpu"
	"github.com/shirou/gopsutil/v3/disk"
	"github.com/shirou/gopsutil/v3/mem"
	"github.com/shirou/gopsutil/v3/net"
)

const (
	SystemmonitorCpuInfo    = "CPU_INFO"
	SystemmonitorMemoryInfo = "MEMORY_INFO"
	SystemmonitorDiskInfo   = "DISK_INFO"
)

type CPUInfo struct {
	CoreCount int32     `json:"coreCount"`
	CPUNum    int       `json:"cpuNum"`
	Model     string    `json:"model"`
	Usages    []float64 `json:"usages"`
}

type DiskInfo struct {
	Mountpoint  string  `json:"mountpoint"`
	Total       uint64  `json:"total"`
	Used        uint64  `json:"used"`
	Free        uint64  `json:"free"`
	UsedPercent float64 `json:"usedPercent"`
}

type NetIOCountersInfo struct {
	BytesSent uint64 `json:"bytesSent"`
	BytesRecv uint64 `json:"bytesRecv"`
	Name      string `json:"name"`
}

type MemoryInfo struct {
	Total       uint64  `json:"total"`
	Free        uint64  `json:"free"`
	Used        uint64  `json:"used"`
	UsedPercent float64 `json:"usedPercent"`
}

// GetCPUInfo 获取CPU信息
func GetCPUInfo() (CPUInfo, error) {
	cpuInfoRes := CPUInfo{}
	cpuInfo, err := cpu.Info()

	if err == nil && len(cpuInfo) > 0 {
		cpuInfoRes.CoreCount = cpuInfo[0].Cores
		cpuInfoRes.Model = cpuInfo[0].ModelName
	}
	numCPU, _ := cpu.Counts(true)
	cpuInfoRes.CPUNum = numCPU
	cpuPercentages, err := cpu.Percent(time.Second, true)
	if err != nil {
		return cpuInfoRes, err
	}

	// cpuPercent 返回的是每颗宿主机逻辑核的占用。容器/虚拟化环境下，
	// 需要按 cgroup 的 CPU 配额（quota 或 cpuset）换算成“相对可用资源”的总体占用，
	// 否则限 2 核的容器会按宿主机 64 核的基数显示，百分比偏小。
	hostCores := len(cpuPercentages)
	if hostCores == 0 {
		hostCores = numCPU
	}
	quotaCores, quotaOK := readCgroupCPUQuotaCores()
	cpusetCores, cpusetOK := readCgroupCPUSetCount()
	effective := effectiveCores(hostCores, quotaCores, quotaOK, cpusetCores, cpusetOK)

	// 选项 C：核数与百分比都按生效配额上报。
	cpuInfoRes.CoreCount = int32(effective)
	cpuInfoRes.CPUNum = effective
	cpuInfoRes.Usages = []float64{quotaPercent(cpuPercentages, effective)}

	return cpuInfoRes, nil
}

// 获取内存信息 单位：MB
func GetMemoryInfo() (MemoryInfo, error) {
	memoryInfo := MemoryInfo{}
	// 获取内存信息
	memInfo, err := mem.VirtualMemory()
	if err != nil {
		return memoryInfo, err
	}

	// 容器/虚拟化环境下 mem.VirtualMemory 读的是 /proc/meminfo，即宿主机物理内存。
	// 当 cgroup 内存限额小于宿主机物理内存时，按限额口径上报，避免限 2G 却显示 64G。
	if reading, ok := readCgroupMemory(); ok && memInfo.Total > 0 && reading.limit < memInfo.Total {
		used := reading.usage
		if reading.inactiveFile < used {
			used -= reading.inactiveFile
		}
		if used > reading.limit {
			used = reading.limit
		}
		memoryInfo.Total = reading.limit
		memoryInfo.Used = used
		memoryInfo.Free = reading.limit - used
		memoryInfo.UsedPercent = float64(used) / float64(reading.limit) * 100
		return memoryInfo, nil
	}

	memoryInfo.Free = memInfo.Free
	memoryInfo.Total = memInfo.Total
	memoryInfo.Used = memInfo.Used
	memoryInfo.UsedPercent = memInfo.UsedPercent

	return memoryInfo, nil
}

func GetDiskMountpoints() ([]disk.PartitionStat, error) {
	return disk.Partitions(true)
}

func GetDiskInfoByPath(path string) (*DiskInfo, error) {
	diskInfo := DiskInfo{}
	usage, err := disk.Usage(path)
	if err != nil {
		return nil, err
	}
	diskInfo.Free = usage.Free
	diskInfo.Mountpoint = usage.Path
	diskInfo.Total = usage.Total
	diskInfo.Used = usage.Used
	diskInfo.UsedPercent = usage.UsedPercent
	return &diskInfo, nil
}

// 获取网络统计信息
func GetNetIOCountersInfo() ([]NetIOCountersInfo, error) {
	var netInfo []NetIOCountersInfo
	netStats, err := net.IOCounters(true)
	if err == nil {
		for _, netStat := range netStats {
			netInfo = append(netInfo, NetIOCountersInfo{
				BytesRecv: netStat.BytesRecv,
				BytesSent: netStat.BytesSent,
				Name:      netStat.Name,
			})

		}
	}
	return netInfo, err
}
