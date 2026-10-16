import { shallowRef } from 'vue'
import type { useHomeShell } from './useHomeShell'

export type HomeShellInstance = ReturnType<typeof useHomeShell>

/**
 * The one home-shell view model, for the surfaces that mount into it.
 *
 * `HomeShell` used to hand this to its children with `provide`/`inject`, which
 * only works while the shell is a static import: the shell now arrives as its own
 * chunk, so a route that mounts in the same tick — `/theme/:view` opened directly
 * or reloaded — would inject nothing and stay empty forever. A module-level ref
 * is the same single instance without the ordering requirement: the surface reads
 * it reactively and starts working as soon as the shell registers.
 */
export const homeShell = shallowRef<HomeShellInstance | null>(null)
