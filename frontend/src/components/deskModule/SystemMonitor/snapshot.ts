import type { InjectionKey, Ref } from 'vue'
import type { CoreMonitorSnapshot } from '../../../core/monitor/themeSnapshot'

export type MonitorSnapshot = CoreMonitorSnapshot

export const monitorSnapshotKey: InjectionKey<Ref<MonitorSnapshot | null>> = Symbol('monitorSnapshot')
