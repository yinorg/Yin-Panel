import type { InjectionKey, Ref } from 'vue'

export interface MonitorSnapshot {
  CPU_INFO?: SystemMonitor.CPUInfo
  MEMORY_INFO?: SystemMonitor.MemoryInfo
  NETWORK_INFO?: { bytesRecv: number; bytesSent: number }[]
}

export const monitorSnapshotKey: InjectionKey<Ref<MonitorSnapshot | null>> = Symbol('monitorSnapshot')
