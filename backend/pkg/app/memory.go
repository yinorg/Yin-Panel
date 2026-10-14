package app

import (
	"os"
	"runtime/debug"

	"github.com/yinorg/Yin-Panel/backend/internal/infra/config"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/zaplog"
	"github.com/yinorg/Yin-Panel/backend/internal/util/monitor"
)

// defaultMemoryLimitRatio keeps headroom below the cgroup limit for the memory
// the Go runtime does not account for in GOMEMLIMIT: the cgo heap behind the
// SQLite driver, its mmap'd database pages, goroutine stacks and the runtime's
// own bookkeeping. The runtime only knows the Go heap, so a limit at 100% of the
// cgroup budget would still let the process be OOM-killed.
const defaultMemoryLimitRatio = 0.75

// memoryLimitFloor stops a very small cgroup limit from producing a target the
// runtime would collect against continuously. The result is still clamped to the
// cgroup limit, so the derived value never exceeds it.
const memoryLimitFloor = 32 << 20

// deriveMemoryLimit turns a cgroup limit and a configured ratio into the soft
// memory limit to apply. A negative ratio disables the adaptation.
func deriveMemoryLimit(cgroupLimit uint64, ratio float64) (uint64, bool) {
	if ratio < 0 {
		return 0, false
	}
	if ratio == 0 {
		ratio = defaultMemoryLimitRatio
	}
	target := uint64(float64(cgroupLimit) * ratio)
	if target < memoryLimitFloor {
		target = memoryLimitFloor
	}
	if target > cgroupLimit {
		target = cgroupLimit
	}
	return target, true
}

// applyMemoryLimit derives a soft memory limit from the cgroup the process runs
// in. It does nothing on a host without a cgroup limit and never overrides an
// explicit GOMEMLIMIT, which the operator may have set deliberately.
func applyMemoryLimit() {
	if os.Getenv("GOMEMLIMIT") != "" {
		zaplog.Logger.Info("GOMEMLIMIT is set explicitly; leaving the runtime memory limit untouched")
		return
	}
	ratio := defaultMemoryLimitRatio
	if config.AppConfig != nil && config.AppConfig.Base.MemoryLimitRatio != 0 {
		ratio = config.AppConfig.Base.MemoryLimitRatio
	}
	cgroupLimit, ok := monitor.CgroupMemoryLimit()
	if !ok {
		if ratio < 0 {
			zaplog.Logger.Info("memory limit adaptation is disabled")
		}
		return
	}
	target, ok := deriveMemoryLimit(cgroupLimit, ratio)
	if !ok {
		zaplog.Logger.Info("memory limit adaptation is disabled")
		return
	}
	previous := debug.SetMemoryLimit(int64(target))
	zaplog.Logger.Infof(
		"GOMEMLIMIT derived from the cgroup limit: %d bytes (cgroup limit %d, ratio %.2f, previous %d)",
		target, cgroupLimit, ratio, previous,
	)
}
