// Service worker registration.
//
// The generated worker caches the theme manifest and grant. A change to those
// changes which theme boots, so an outdated worker is not a cosmetic problem: it
// silently keeps serving the previous revision and the previous theme code.
//
// Browsers check `/sw.js` for a new worker on navigation, but only activate the
// result on the *next* load, so a deployment is otherwise invisible until every
// tab is closed. This asks for the update explicitly and reloads once, which is
// what makes a deploy take effect for someone who simply keeps the panel open.
//
// The reload is guarded so it can only happen once per worker version, and only
// when there is no user input at risk: `controllerchange` fires when the new
// worker takes over.
if ('serviceWorker' in navigator) {
  window.addEventListener('load', () => {
    navigator.serviceWorker.register('/sw.js', { scope: '/' }).then((registration) => {
      // No controller yet means this is the very first install; nothing to update.
      if (!navigator.serviceWorker.controller) return
      // Fires when a newly installed worker takes over from the old one.
      navigator.serviceWorker.addEventListener('controllerchange', () => {
        // The session was transferred to the new worker. Reloading is what actually
        // puts the new caching rules in front of the page; without it the document
        // keeps running under the worker's old behaviour.
        window.location.reload()
      })
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