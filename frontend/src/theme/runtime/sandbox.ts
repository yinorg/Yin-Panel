import { createThemeApiDispatcher } from '../api/dispatcher'
import { isThemeApiRequest, type ThemeEnvironment, type ThemeHomeSnapshot, type ThemePermission } from '../api/v1'

const MAX_MESSAGE_BYTES = 1_048_576
const HANDSHAKE_TIMEOUT = 10_000

const bootstrap = `
(() => {
  const connect = (event) => {
    const data = event.data;
    if (event.source !== parent || !data || data.protocol !== 'yin-theme-api' || data.type !== 'connect' || typeof data.nonce !== 'string' || !event.ports || !event.ports[0]) return;
    removeEventListener('message', connect);
    const port = event.ports[0];
    const nonce = data.nonce;
    port.postMessage({ type: 'connected', nonce });
    let snapshot;
    let environment;
    let moduleDefinition;
    let mountedView;
    let tokenStyle;
    let objectUrl;
    const listeners = new Map();
    const pending = new Map();
    function reply(message) {
      try {
        if (JSON.stringify(message).length <= 1048576) port.postMessage(message);
      } catch (_) {}
    }
    function request(method, payload) {
      const requestId = crypto.randomUUID();
      return new Promise((resolve, reject) => {
        pending.set(requestId, { resolve, reject });
        reply({ type: 'api-request', request: { protocol: 'yin-theme-api', version: 1, requestId, contextVersion: snapshot.version, method, payload } });
      });
    }
    const api = {
      environment: { get: () => structuredClone(environment) },
      state: { getSnapshot: () => structuredClone(snapshot) },
      events: { subscribe: (name, listener) => {
        if (typeof name !== 'string' || typeof listener !== 'function') throw new TypeError('Invalid theme event subscription');
        const handlers = listeners.get(name) || new Set();
        handlers.add(listener); listeners.set(name, handlers);
        return () => handlers.delete(listener);
      } },
      commands: { execute: (command, payload = {}) => request('commands.execute', { command, arguments: payload }) },
      navigation: {
        get: () => ({ view: 'home', spaceId: snapshot.activeSpaceId }),
        navigate: destination => request('navigation.navigate', destination),
      },
      assets: { resolve: path => {
        const url = environment.assets && environment.assets[path];
        if (!url) throw new Error('Theme asset is not declared');
        return url;
      } },
      settings: { get: () => request('settings.get'), patch: value => request('settings.patch', { value }) },
      storage: {
        get: key => request('storage.get', { key }),
        set: (key, value) => request('storage.set', { key, value }),
        remove: key => request('storage.remove', { key }),
      },
    };
    port.onmessage = async ({ data: message }) => {
      if (!message || typeof message !== 'object' || JSON.stringify(message).length > 1048576) return;
      if (message.type === 'api-response' && message.response && typeof message.response.requestId === 'string') {
        const operation = pending.get(message.response.requestId);
        if (!operation) return;
        pending.delete(message.response.requestId);
        if (message.response.ok) operation.resolve(message.response.result);
        else operation.reject(Object.assign(new Error(message.response.error && message.response.error.message || 'Theme API request failed'), { code: message.response.error && message.response.error.code || 'RUNTIME_UNAVAILABLE' }));
        return;
      }
      if (message.type === 'event' && typeof message.name === 'string') {
        for (const listener of listeners.get(message.name) || []) {
          try { listener(structuredClone(message.payload)); } catch (_) {}
        }
        return;
      }
      if (message.type === 'update' && message.snapshot && Number.isSafeInteger(message.snapshot.version)) {
        snapshot = message.snapshot;
        try { await mountedView.update && mountedView.update(structuredClone(snapshot)); }
        catch (error) { reply({ type: 'error', message: String(error && error.message || error).slice(0, 1000) }); }
        return;
      }
      if (message.type === 'dispose') {
        try {
          for (const listener of listeners.get('theme.disposing') || []) listener(undefined);
          if (mountedView && typeof mountedView.unmount === 'function') await mountedView.unmount();
          if (moduleDefinition && typeof moduleDefinition.dispose === 'function') await moduleDefinition.dispose();
          if (objectUrl) URL.revokeObjectURL(objectUrl);
          reply({ type: 'disposed' });
        } catch (error) {
          reply({ type: 'error', message: String(error && error.message || error).slice(0, 1000) });
        }
        return;
      }
      if (message.type === 'tokens.update' && typeof message.cssText === 'string') {
        tokenStyle.textContent = message.cssText;
        return;
      }
      if (message.type !== 'init' || typeof message.script !== 'string' || !Array.isArray(message.styles) || !message.snapshot || !message.environment) return;
      try {
        snapshot = message.snapshot;
        environment = message.environment;
        tokenStyle = document.createElement('style');
        tokenStyle.textContent = message.tokens || '';
        document.head.appendChild(tokenStyle);
        if (message.styles.length && message.styles[0].href) {
          const base = document.createElement('base');
          base.href = message.styles[0].href;
          document.head.appendChild(base);
        }
        for (const stylesheet of message.styles) {
          if (!stylesheet || typeof stylesheet.href !== 'string' || typeof stylesheet.text !== 'string') throw new Error('Invalid theme stylesheet');
          const style = document.createElement('style');
          style.textContent = stylesheet.text;
          document.head.appendChild(style);
        }
        const source = new Blob([message.script], { type: 'text/javascript' });
        objectUrl = URL.createObjectURL(source);
        const loaded = await import(objectUrl);
        const themeModule = loaded.default;
        if (!themeModule || themeModule.apiVersion !== '1.0.0' || typeof themeModule.setup !== 'function') throw new Error('Theme entrypoint must export a Theme API v1 module as default');
        moduleDefinition = await themeModule.setup(api);
        const view = moduleDefinition && moduleDefinition.views && moduleDefinition.views.home;
        if (typeof view !== 'function') throw new Error('Theme does not register the contributed home view');
        const root = document.getElementById('theme-root');
        mountedView = await view(root, api, structuredClone(snapshot));
        if (!mountedView || typeof mountedView.unmount !== 'function') throw new Error('Theme home view must return an unmount function');
        reply({ type: 'ready' });
      } catch (error) {
        reply({ type: 'error', message: String(error && error.message || error).slice(0, 1000) });
      }
    };
    port.start();
  };
  addEventListener('message', connect);
})();
`

export function createThemeSandboxDocument(resourceOrigin: string) {
  let parsedOrigin: URL
  try {
    parsedOrigin = new URL(resourceOrigin)
  }
  catch {
    throw new Error('Unsupported theme resource origin')
  }
  if (!['http:', 'https:'].includes(parsedOrigin.protocol) || parsedOrigin.username || parsedOrigin.password)
    throw new Error('Unsupported theme resource origin')
  const origin = parsedOrigin.origin
  const policy = `default-src 'none'; script-src 'unsafe-inline' blob:; style-src 'unsafe-inline' ${origin}; img-src data: blob: ${origin}; font-src data: blob: ${origin}; media-src data: blob: ${origin}; connect-src 'none'; object-src 'none'; form-action 'none'; base-uri ${origin}`
  return `<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="${policy}"><meta name="viewport" content="width=device-width,initial-scale=1"><style>html,body,#theme-root{box-sizing:border-box;width:100%;height:100%;margin:0}*,*::before,*::after{box-sizing:inherit}body{overflow:auto}</style></head><body><div id="theme-root"></div><script>${bootstrap}</script></body></html>`
}

export interface ThemeSandboxStylesheet {
  href: string
  text: string
}

export interface ThemeSandboxOptions {
  script: string
  styles: ThemeSandboxStylesheet[]
  tokens: string
  snapshot: ThemeHomeSnapshot
  environment: Omit<ThemeEnvironment, 'apiVersion'> & { assets: Record<string, string> }
  permissions: ReadonlySet<ThemePermission>
  execute(request: unknown): Promise<unknown>
  onError(error: Error): void
}

export interface ThemeSandboxHandle {
  update(snapshot: ThemeHomeSnapshot): void
  updateTokens(cssText: string): void
  emit(name: string, payload?: unknown): void
  dispose(): Promise<void>
}

export async function mountThemeSandbox(frame: HTMLIFrameElement, options: ThemeSandboxOptions): Promise<ThemeSandboxHandle> {
  frame.setAttribute('sandbox', 'allow-scripts')
  const connected = waitForFrameLoad(frame)
  frame.srcdoc = createThemeSandboxDocument(window.location.origin)
  await connected

  const channel = new MessageChannel()
  const nonceBytes = crypto.getRandomValues(new Uint8Array(24))
  const nonce = Array.from(nonceBytes, value => value.toString(16).padStart(2, '0')).join('')
  let resolveReady: () => void
  let rejectReady: (error: Error) => void
  const ready = new Promise<void>((resolve, reject) => { resolveReady = resolve; rejectReady = reject })
  let handshakeTimer: ReturnType<typeof setTimeout> | undefined
  let stopped = false
  let connectedOnce = false

  const dispatcher = createThemeApiDispatcher({
    getContextVersion: () => options.snapshot.version,
    getPermissions: () => options.permissions,
    handlers: {
      executeCommand: (command, payload) => options.execute({ method: 'commands.execute', command, payload }),
      navigate: destination => options.execute({ method: 'navigation.navigate', destination }),
      getSettings: () => options.execute({ method: 'settings.get' }) as Promise<Record<string, unknown>>,
      patchSettings: value => options.execute({ method: 'settings.patch', value }),
      getStorage: key => options.execute({ method: 'storage.get', key }),
      setStorage: (key, value) => options.execute({ method: 'storage.set', key, value }),
      removeStorage: key => options.execute({ method: 'storage.remove', key }),
    },
  })

  const fail = (message: string) => {
    if (stopped) return
    const error = new Error(message)
    stopped = true
    if (handshakeTimer) clearTimeout(handshakeTimer)
    channel.port1.close()
    frame.srcdoc = '<!doctype html><title>Theme unavailable</title>'
    rejectReady(error)
    options.onError(error)
  }

  channel.port1.onmessage = async ({ data }) => {
    if (!data || typeof data !== 'object' || !jsonWithinLimit(data) || stopped && data.type !== 'disposed') return
    if (data.type === 'connected' && data.nonce === nonce && !connectedOnce) {
      connectedOnce = true
      const initMessage = {
        type: 'init',
        script: options.script,
        styles: options.styles,
        tokens: options.tokens,
        snapshot: options.snapshot,
        environment: { ...options.environment, apiVersion: '1.0.0' },
      }
      if (!jsonWithinLimit(initMessage)) {
        fail('Theme initialization data exceeds the 1 MiB message limit')
        return
      }
      channel.port1.postMessage(initMessage)
      return
    }
    if (data.type === 'ready' && connectedOnce) {
      if (handshakeTimer) clearTimeout(handshakeTimer)
      resolveReady()
      return
    }
    if (data.type === 'error') {
      fail(typeof data.message === 'string' ? data.message.slice(0, 1000) : 'Theme runtime failed')
      return
    }
    if (data.type === 'api-request' && isThemeApiRequest(data.request)) {
      const response = await dispatcher(data.request)
      if (!stopped && response) channel.port1.postMessage({ type: 'api-response', response })
    }
  }
  channel.port1.start()

  frame.contentWindow?.postMessage({ protocol: 'yin-theme-api', type: 'connect', nonce }, '*', [channel.port2])
  handshakeTimer = setTimeout(() => fail('Theme setup or mount timed out'), HANDSHAKE_TIMEOUT)
  await ready

  return {
    update(snapshot) {
      options.snapshot = snapshot
      const message = { type: 'update', snapshot }
      if (!stopped && !jsonWithinLimit(message)) {
        fail('Theme context snapshot exceeds the 1 MiB message limit')
        return
      }
      if (!stopped) channel.port1.postMessage(message)
    },
    updateTokens(cssText) {
      if (!stopped) channel.port1.postMessage({ type: 'tokens.update', cssText })
    },
    emit(name, payload) {
      if (!stopped) channel.port1.postMessage({ type: 'event', name, payload })
    },
    dispose() {
      if (stopped) return Promise.resolve()
      stopped = true
      return new Promise((resolve) => {
        const timer = setTimeout(() => {
          channel.port1.close()
          frame.srcdoc = '<!doctype html><title>Theme closed</title>'
          resolve()
        }, 2_000)
        channel.port1.addEventListener('message', ({ data }) => {
          if (data?.type !== 'disposed') return
          clearTimeout(timer)
          channel.port1.close()
          frame.srcdoc = '<!doctype html><title>Theme closed</title>'
          resolve()
        })
        channel.port1.postMessage({ type: 'dispose' })
      })
    },
  }
}

function waitForFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('Theme sandbox frame did not load')), HANDSHAKE_TIMEOUT)
    frame.addEventListener('load', () => { clearTimeout(timer); resolve() }, { once: true })
    frame.addEventListener('error', () => { clearTimeout(timer); reject(new Error('Theme sandbox frame failed to load')) }, { once: true })
  })
}

function jsonWithinLimit(value: unknown): boolean {
  try {
    return JSON.stringify(value).length <= MAX_MESSAGE_BYTES
  }
  catch {
    return false
  }
}
