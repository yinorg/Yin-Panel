package cache

import (
	"fmt"
	"sync"
	"time"
	"yin-panel/internal/infra/kvcache"
	"yin-panel/internal/util/monitor"
)

type Monitor struct {
	Cache            kvcache.Cacher[interface{}]
	cpuCapability    sync.Once
	memoryCapability sync.Once
	diskCapability   sync.Once
	netCapability    sync.Once
	cpuErr           error
	memoryErr        error
	diskErr          error
	netErr           error
	snapshotMu       sync.RWMutex
	snapshot         map[string]any
}

const cacheSecond = 3

func (a *Monitor) Start() {
	if a.snapshot != nil {
		return
	}
	a.snapshot = make(map[string]any)
	a.collect()
	go func() {
		ticker := time.NewTicker(cacheSecond * time.Second)
		defer ticker.Stop()
		for range ticker.C {
			a.collect()
		}
	}()
}

func (a *Monitor) collect() {
	values := make(map[string]any)
	if value, err := monitor.GetCPUInfo(); err == nil {
		values[monitor.SystemmonitorCpuInfo] = value
	}
	if value, err := monitor.GetMemoryInfo(); err == nil {
		values[monitor.SystemmonitorMemoryInfo] = value
	}
	if value, err := monitor.GetNetIOCountersInfo(); err == nil {
		values["NETWORK_INFO"] = value
	}
	a.snapshotMu.Lock()
	for key, value := range values {
		a.snapshot[key] = value
	}
	a.snapshotMu.Unlock()
}

func (a *Monitor) sharedSnapshot(key string) (any, error) {
	a.snapshotMu.RLock()
	value, ok := a.snapshot[key]
	a.snapshotMu.RUnlock()
	if !ok {
		return nil, fmt.Errorf("monitor data unavailable: %s", key)
	}
	return value, nil
}

func (a *Monitor) GetSnapshot() (map[string]any, error) {
	a.snapshotMu.RLock()
	defer a.snapshotMu.RUnlock()
	if len(a.snapshot) == 0 {
		return nil, fmt.Errorf("monitor snapshot unavailable")
	}
	result := make(map[string]any, len(a.snapshot))
	for key, value := range a.snapshot {
		result[key] = value
	}
	return result, nil
}

func (a *Monitor) GetNetState() (any, error) {
	if a.snapshot != nil {
		return a.sharedSnapshot("NETWORK_INFO")
	}
	a.netCapability.Do(func() { _, a.netErr = monitor.GetNetIOCountersInfo() })
	if a.netErr != nil {
		return nil, a.netErr
	}
	if v, ok := a.Cache.Get("NETWORK_INFO"); ok {
		return v, nil
	}
	value, err := monitor.GetNetIOCountersInfo()
	if err != nil {
		return nil, err
	}
	a.Cache.Set("NETWORK_INFO", value, cacheSecond*time.Second)
	return value, nil
}

func (a *Monitor) GetCpuState() (any, error) {
	if a.snapshot != nil {
		return a.sharedSnapshot(monitor.SystemmonitorCpuInfo)
	}
	a.cpuCapability.Do(func() {
		_, a.cpuErr = monitor.GetCPUInfo()
	})
	if a.cpuErr != nil {
		return nil, a.cpuErr
	}
	if v, ok := a.Cache.Get(monitor.SystemmonitorCpuInfo); ok {
		return v, nil
	}

	cpuInfo, err := monitor.GetCPUInfo()
	if err != nil {
		return nil, err
	}

	a.Cache.Set(monitor.SystemmonitorCpuInfo, cpuInfo, cacheSecond*time.Second)
	return cpuInfo, nil
}

func (a *Monitor) GetMemonyState() (any, error) {
	if a.snapshot != nil {
		return a.sharedSnapshot(monitor.SystemmonitorMemoryInfo)
	}
	a.memoryCapability.Do(func() {
		_, a.memoryErr = monitor.GetMemoryInfo()
	})
	if a.memoryErr != nil {
		return nil, a.memoryErr
	}
	if v, ok := a.Cache.Get(monitor.SystemmonitorMemoryInfo); ok {
		return v, nil
	}

	memoryInfo, err := monitor.GetMemoryInfo()
	if err != nil {
		return nil, err
	}

	a.Cache.Set(monitor.SystemmonitorMemoryInfo, memoryInfo, cacheSecond*time.Second)
	return memoryInfo, nil
}

func (a *Monitor) GetDiskStateByPath(path string) (any, error) {
	a.diskCapability.Do(func() {
		_, a.diskErr = monitor.GetDiskInfoByPath(path)
	})
	if a.diskErr != nil {
		return nil, a.diskErr
	}
	disk := monitor.SystemmonitorDiskInfo + path
	if v, ok := a.Cache.Get(disk); ok {
		return v, nil
	}

	diskState, err := monitor.GetDiskInfoByPath(path)
	if err != nil {
		return nil, err
	}

	a.Cache.Set(disk, diskState, cacheSecond*time.Second)
	return diskState, nil
}
