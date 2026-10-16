import { defineAsyncComponent } from 'vue'

/**
 * The themed shell, in a chunk of its own.
 *
 * `HomeShell` is the gate to the whole home runtime — the theme package
 * resolver, the theme host, the data channel and the Core chrome — and none of
 * that is needed to paint the login form. Keeping the import here gives the
 * Core-only routes an entry graph without it, while the themed routes still
 * render exactly the same component.
 *
 * The loader is exported because the shell is warmed from two places: `main.ts`
 * starts it for a themed entry URL, and the login page warms it in the
 * background once its own form has painted.
 */
export function loadHomeShell() {
  return import('./HomeShell.vue')
}

export const HomeShell = defineAsyncComponent(loadHomeShell)
