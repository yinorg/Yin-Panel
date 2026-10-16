// Service worker registration.
//
// The generated worker caches the theme manifest and grant, so an outdated worker is
// not a cosmetic problem: it silently keeps serving the previous revision and the
// previous theme code. The build sets `skipWaiting` + `clientsClaim`, so a newly
// deployed worker takes over as soon as it installs; this script only has to ask for
// the update and tell the page about it.
//
// A handover leaves the open document running the previous build. Reloading it right
// away is the one thing users notice, so instead:
//
// - the page is told (`yin:update-ready`) and puts a dot on the refresh button, which
//   the user may click or ignore;
// - the reload happens by itself only once the tab is hidden — the user is somewhere
//   else, so nothing is seen — and only when it cannot destroy work: no text field
//   has focus, no dialog is open and the browser is online. Switching tabs mid-edit is
//   ordinary, so those gates matter more, not less, on this path;
// - clicking the dot asks for the reload directly (`yin:update-apply`).
//
// A tab that is never hidden keeps running the build it loaded and picks the new one
// up on the next navigation. Every reload this script performs is marked, so
// `index.html` drops the boot layer before its first paint.
const RELOAD_FLAG = 'yin-sw-reload'

function isTextFieldFocused() {
  const active = document.activeElement
  return !!active && (active.tagName === 'INPUT' || active.tagName === 'TEXTAREA' || active.isContentEditable === true)
}

// Naive unmounts a closed modal (its `display-directive` defaults to `if`), so any
// wrapper left in the DOM belongs to a dialog the user may still be working in.
function hasOpenDialog() {
  return !!document.querySelector('.n-modal-body-wrapper')
}

function canReloadUnseen() {
  return document.hidden
    && !isTextFieldFocused()
    && !hasOpenDialog()
    && navigator.onLine !== false
}

function reloadUnseen() {
  try { window.sessionStorage.setItem(RELOAD_FLAG, '1') }
  catch (error) {
    // Storage can be refused; the reload still happens, the boot layer simply stays.
  }
  window.location.reload()
}

if ('serviceWorker' in navigator) {
  window.addEventListener('load', () => {
    navigator.serviceWorker.register('/sw.js', { scope: '/' }).then((registration) => {
      // The first worker to claim this page is not an update — it is the install the
      // page just asked for. Only a later handover means the loaded build is stale,
      // and the listener has to be armed from the start: returning early on a first
      // visit left the whole session without one.
      let sawController = !!navigator.serviceWorker.controller
      let handoverSeen = false

      function applyWhenUnseen() {
        if (!handoverSeen || !canReloadUnseen()) return
        reloadUnseen()
      }

      // `skipWaiting` + `clientsClaim` make the new worker take over immediately, so
      // this fires while the page is still running the previous build.
      navigator.serviceWorker.addEventListener('controllerchange', () => {
        if (!sawController) {
          sawController = true
          return
        }
        handoverSeen = true
        window.dispatchEvent(new CustomEvent('yin:update-ready'))
        applyWhenUnseen()
      })

      // The two moments worth re-checking: the tab was hidden, or the browser came
      // back online (an offline reload would only fail).
      document.addEventListener('visibilitychange', applyWhenUnseen)
      window.addEventListener('online', applyWhenUnseen)
      window.addEventListener('yin:update-apply', reloadUnseen)

      // Poll rather than relying solely on the browser's own update check: that check
      // does not run while the tab stays open, which is the common case here.
      const poll = window.setInterval(async () => {
        try {
          await registration.update()
        }
        catch {
          // Offline or an update in progress. The next tick retries; clearing the
          // worker with /clear.html is the manual escape hatch.
        }
      }, 60_000)
      window.addEventListener('pagehide', () => window.clearInterval(poll))
    })
  })
}
