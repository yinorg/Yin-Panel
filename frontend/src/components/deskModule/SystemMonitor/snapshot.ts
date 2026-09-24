import type { InjectionKey, Ref } from 'vue'

export interface MonitorSnapshot {
  CPU_INFO?: SystemMonitor.CPUInfo
  MEMORY_INFO?: SystemMonitor.MemoryInfo
  NETWORK_INFO?: { bytesRecv: number; bytesSent: number }[]
  DISK_INFO?: Record<string, SystemMonitor.DiskInfo>
}

export const monitorSnapshotKey: InjectionKey<Ref<MonitorSnapshot | null>> = Symbol('monitorSnapshot')
