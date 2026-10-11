import { onUnmounted, watch } from 'vue'

const BOOT_ELEMENT_ID = 'yin-boot'
const BOOT_BAR_SELECTOR = '[data-boot-bar]'
/** Never leave the boot layer on screen. Past this the app must be reachable even
 *  when the theme is still loading — a stuck overlay blocks every interaction,
 *  which is worse than the flash it exists to hide. */
const BOOT_LEAVE_TIMEOUT_MS = 6000
const BOOT_FADE_MS = 400

let dismissed = false

/**
 * Advance the boot layer's progress bar.
 *
 * The bar carries real milestones (app mounted → home data → theme resolved →
 * theme painted) rather than a fabricated animation, so it never claims to know
 * more than the shell does.
 */
export function setBootProgress(percent: number) {
  const bar = document.querySelector<HTMLElement>(BOOT_BAR_SELECTOR)
  if (!bar) return
  bar.style.width = `${Math.min(Math.max(percent, 0), 100)}%`
}

/**
 * Remove the pre-bundle boot layer painted by `index.html`.
 *
 * Idempotent, so every owner (the home shell, the Core-only routes) can call it
 * without coordinating. The layer sits outside `#app` (Vue's mount cannot clear
 * it) and covers the Core's default light canvas until the theme — or the fallback —
 * has painted, so the first frames never flash a white page or a white
 * space-selector pill. It also sits below Naive's modal layer, so a late dismissal
 * can never block a dialog.
 */
export function dismissBootOverlay() {
  if (dismissed) return
  dismissed = true
  setBootProgress(100)
  const element = document.getElementById(BOOT_ELEMENT_ID)
  if (!element) return

  let finished = false
  const finish = () => {
    if (finished) return
    finished = true
    const remove = () => element.remove()
    if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
      remove()
      return
    }
    element.classList.add('yin-boot--leaving')
    element.addEventListener('transitionend', remove, { once: true })
    // `transitionend` does not fire when the layer is already transparent or the
    // tab is hidden, so removal cannot depend on it alone.
    setTimeout(remove, BOOT_FADE_MS)
  }

  // `?bootHold=<ms>` keeps the layer up so the design can be reviewed. The hold is
  // measured from the first paint, capped in the inline script, and a click retires
  // it immediately — it can never become a way to get stuck.
  const bootWindow = window as unknown as { __yinBootHold?: number, __yinBootStart?: number }
  const hold = Number(bootWindow.__yinBootHold) || 0
  const started = Number(bootWindow.__yinBootStart) || 0
  const remaining = Math.max(hold - (started ? performance.now() - started : 0), 0)
  if (remaining > 0) {
    const onPointer = () => {
      element.removeEventListener('pointerdown', onPointer)
      finish()
    }
    element.addEventListener('pointerdown', onPointer)
    setTimeout(finish, remaining)
    return
  }

  finish()
}

export interface BootOverlaySignals {
  /** The home data has settled. */
  dataReady: () => boolean
  /** The theme package is known (its palette is about to be applied). */
  themeResolved: () => boolean
  /** The theme host mounted and rendered the home. */
  themeReady: () => boolean
  /** The Core decided the theme cannot render, so the C3 fallback is taking over. */
  fallbackVisible: () => boolean
  /** Offline with no cached home: the Core's own notice must be reachable. */
  offlineUnavailable: () => boolean
  /** A modal must be able to take focus: the theme consent prompt or the access-code prompt. */
  modalPending: () => boolean
}

/** Drive the boot layer from the home's real readiness and retire it once it can paint. */
export function useBootOverlay(signals: BootOverlaySignals) {
  const stopProgress = watch(
    () => [signals.dataReady(), signals.themeResolved()] as const,
    ([data, theme]) => { setBootProgress(theme ? 85 : data ? 60 : 30) },
    { immediate: true },
  )

  const stop = watch(
    () => signals.themeReady() || signals.fallbackVisible() || signals.offlineUnavailable() || signals.modalPending(),
    ready => { if (ready) dismissBootOverlay() },
    { immediate: true },
  )

  const timeout = setTimeout(dismissBootOverlay, BOOT_LEAVE_TIMEOUT_MS)

  onUnmounted(() => {
    stop()
    stopProgress()
    clearTimeout(timeout)
  })
}
