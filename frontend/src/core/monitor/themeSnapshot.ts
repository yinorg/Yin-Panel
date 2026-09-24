import type { ThemeMonitorSnapshot } from '../../../packages/theme-sdk/src/index'

export interface CoreMonitorSnapshot {
  CPU_INFO?: SystemMonitor.CPUInfo
  MEMORY_INFO?: SystemMonitor.MemoryInfo
  NETWORK_INFO?: SystemMonitor.NetIOCountersInfo[]
}

export function normalizeMonitorSnapshot(source: CoreMonitorSnapshot, capturedAt = new Date().toISOString()): ThemeMonitorSnapshot {
  const cpuUsage = source.CPU_INFO?.usages?.[0]
  const cpuPercent = Number.isFinite(cpuUsage) ? Number(cpuUsage) : 0
  const memory = source.MEMORY_INFO
  return {
    capturedAt,
    cpu: source.CPU_INFO ? {
      coreCount: source.CPU_INFO.coreCount,
      model: source.CPU_INFO.model,
      usagePercent: cpuPercent,
    } : undefined,
    memory: memory ? {
      total: memory.total,
      used: memory.used,
      free: memory.free,
      usedPercent: Number.isFinite(memory.usedPercent) ? memory.usedPercent : 0,
    } : undefined,
    network: source.NETWORK_INFO?.map(counter => ({
      name: counter.name,
      bytesSent: counter.bytesSent,
      bytesRecv: counter.bytesRecv,
    })),
  }
}
