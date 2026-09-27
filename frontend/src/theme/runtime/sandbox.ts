import { createThemeApiExecutionDispatcher } from '../api/dispatcher'
import { isThemeApiRequest, isThemeEventName, type ThemeEnvironment, type ThemeEventEnvelope, type ThemeEventName, type ThemeHomeSnapshot, type ThemePermission } from '../api/v1'
import { createThemeApiClient } from '../../../packages/theme-sdk/src/index'
import { createThemeRequestGuard } from './requestGuard'
import { createThemeSandboxDocument } from './sandboxDocument'
export { createThemeSandboxDocument } from './sandboxDocument'

const MAX_MESSAGE_BYTES = 1_048_576
const HANDSHAKE_TIMEOUT = 10_000

const bootstrap = (apiClientSource: string) => `
(() => {
  const createThemeApiClient = (${apiClientSource});
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
    const mountedRegions = [];
    const mountedComponents = [];
    let tokenStyle;
    let objectUrl;
    const pending = new Map();
    function reply(message) {
      try {
        if (JSON.stringify(message).length <= 1048576) port.postMessage(message);
      } catch (_) {}
    }
    function request(request) {
      return new Promise((resolve, reject) => {
        pending.set(request.requestId, { resolve, reject });
        reply({ type: 'api-request', request });
      });
    }
    const apiClient = createThemeApiClient({ request, getSnapshot: () => snapshot, getEnvironment: () => environment });
    const api = apiClient.api;
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
        apiClient.emit(message.name, message.event);
        return;
      }
      if (message.type === 'scroll.toTop') {
        window.scrollTo({ top: 0, behavior: 'smooth' });
        document.scrollingElement?.scrollTo({ top: 0, behavior: 'smooth' });
        return;
      }
      if (message.type === 'update' && message.snapshot && Number.isSafeInteger(message.snapshot.version)) {
        snapshot = message.snapshot;
        apiClient.updateSnapshot(snapshot);
        try { await mountedView.update && mountedView.update(structuredClone(snapshot)); for (const region of mountedRegions) await region.update && region.update(structuredClone(snapshot)); for (const component of mountedComponents) await component.update && component.update(structuredClone(snapshot)); }
        catch (error) { reply({ type: 'error', message: String(error && error.message || error).slice(0, 1000) }); }
        return;
      }
      if (message.type === 'environment.update' && message.environment && typeof message.environment === 'object') {
        environment = message.environment;
        apiClient.updateEnvironment(environment);
        return;
      }
      if (message.type === 'dispose') {
        try {
          for (const operation of pending.values()) operation.reject(Object.assign(new Error('Theme runtime disposed'), { code: 'ABORTED' }));
          pending.clear();
          apiClient.dispose();
          for (const region of mountedRegions) if (region && typeof region.unmount === 'function') await region.unmount();
          for (const component of mountedComponents) if (component && typeof component.unmount === 'function') await component.unmount();
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
        apiClient.updateSnapshot(snapshot);
        apiClient.updateEnvironment(environment);
        tokenStyle = document.createElement('style');
        tokenStyle.textContent = message.tokens || '';
        document.head.appendChild(tokenStyle);
        for (const stylesheet of message.styles) {
          if (!stylesheet || typeof stylesheet.text !== 'string') throw new Error('Invalid theme stylesheet');
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
        for (const region of (message.contributions && message.contributions.regions) || []) {
          const slot = document.createElement('section');
          slot.dataset.themeRegion = region;
          slot.dataset.themeRegionSlot = region;
          root.append(slot);
          const factory = moduleDefinition.regions && moduleDefinition.regions[region];
          if (typeof factory !== 'function') throw new Error('Theme does not register contributed region ' + region);
          const regionView = await factory(slot, api, structuredClone(snapshot));
          if (!regionView || typeof regionView.unmount !== 'function') throw new Error('Theme region ' + region + ' must return an unmount function');
          mountedRegions.push(regionView);
        }
        for (const component of (message.contributions && message.contributions.components) || []) {
          const slot = document.createElement('section');
          slot.dataset.themeComponent = component;
          slot.dataset.themeComponentSlot = component;
          root.append(slot);
          const factory = moduleDefinition.components && moduleDefinition.components[component];
          if (typeof factory !== 'function') throw new Error('Theme does not register contributed component ' + component);
          const componentView = await factory(slot, api, structuredClone(snapshot));
          if (!componentView || typeof componentView.unmount !== 'function') throw new Error('Theme component ' + component + ' must return an unmount function');
          mountedComponents.push(componentView);
        }
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

export interface ThemeSandboxStylesheet {
  text: string
}

export interface ThemeSandboxOptions {
  script: string
  styles: ThemeSandboxStylesheet[]
  tokens: string
  snapshot: ThemeHomeSnapshot
  environment: Omit<ThemeEnvironment, 'apiVersion'> & { assets: Record<string, string> }
  permissions: ReadonlySet<ThemePermission>
  execute: (request: unknown) => Promise<unknown>
  onError: (error: Error) => void
  contributions?: { views?: readonly string[]; regions?: readonly string[]; components?: readonly string[] }
}

export interface ThemeSandboxHandle {
  update: (snapshot: ThemeHomeSnapshot) => void
  updateEnvironment: (environment: Omit<ThemeEnvironment, 'apiVersion'> & { assets: Record<string, string> }) => void
  updateTokens: (cssText: string) => void
  emit: (name: ThemeEventName, payload?: unknown) => void
  scrollToTop: () => void
  dispose: () => Promise<void>
}

export async function mountThemeSandbox(frame: HTMLIFrameElement, options: ThemeSandboxOptions): Promise<ThemeSandboxHandle> {
  frame.setAttribute('sandbox', 'allow-scripts')
  const connected = waitForFrameLoad(frame)
  frame.srcdoc = createThemeSandboxDocument(window.location.origin, bootstrap(createThemeApiClient.toString()))
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
  let eventSequence = 0
  const requestGuard = createThemeRequestGuard()

  const dispatcher = createThemeApiExecutionDispatcher({
    getContextVersion: () => options.snapshot.version,
    getPermissions: () => options.permissions,
    execute: options.execute,
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
    try {
      if (!data || typeof data !== 'object' || !jsonWithinLimit(data) || stopped && data.type !== 'disposed') return
      if (data.type === 'connected' && data.nonce === nonce && !connectedOnce) {
        connectedOnce = true
        const initMessage = cloneThemeMessage({
          type: 'init',
          script: options.script,
          styles: options.styles,
          tokens: options.tokens,
          snapshot: options.snapshot,
          environment: { ...options.environment, apiVersion: '1.0.0' },
          contributions: options.contributions,
        })
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
        try {
          if (!requestGuard.remember(data.request.requestId)) {
            channel.port1.postMessage({
              type: 'api-response',
              response: {
                protocol: 'yin-theme-api', version: 1,
                requestId: data.request.requestId, contextVersion: data.request.contextVersion,
                ok: false, error: { code: 'INVALID_ARGUMENT', message: 'Theme request IDs can only be used once' },
              },
            })
            return
          }
        }
        catch (error) {
          fail(error instanceof Error ? error.message : 'Theme request limit exceeded')
          return
        }
        const response = await dispatcher(data.request)
        if (!stopped && response) channel.port1.postMessage(cloneThemeMessage({ type: 'api-response', response }))
      }
    }
    catch (error) {
      fail(`Theme runtime messaging failed: ${error instanceof Error ? error.message : String(error)}`)
    }
  }
  channel.port1.start()

  frame.contentWindow?.postMessage({ protocol: 'yin-theme-api', type: 'connect', nonce }, '*', [channel.port2])
  handshakeTimer = setTimeout(() => fail('Theme setup or mount timed out'), HANDSHAKE_TIMEOUT)
  await ready

  const sendEvent = (name: ThemeEventName, payload?: unknown) => {
    if (stopped || !isThemeEventName(name)) return
    const event: ThemeEventEnvelope = cloneThemeMessage({
      contextVersion: options.snapshot.version,
      sequence: ++eventSequence,
      payload,
    })
    const message = { type: 'event', name, event }
    if (!jsonWithinLimit(message)) {
      fail('Theme event exceeds the 1 MiB message limit')
      return
    }
    channel.port1.postMessage(message)
  }

  return {
    update(snapshot) {
      const safeSnapshot = cloneThemeMessage(snapshot)
      options.snapshot = safeSnapshot
      const message = { type: 'update', snapshot: safeSnapshot }
      if (!stopped && !jsonWithinLimit(message)) {
        fail('Theme context snapshot exceeds the 1 MiB message limit')
        return
      }
      if (!stopped) channel.port1.postMessage(message)
    },
    updateEnvironment(environment) {
      const safeEnvironment = cloneThemeMessage(environment)
      options.environment = safeEnvironment
      const message = { type: 'environment.update', environment: safeEnvironment }
      if (!stopped && !jsonWithinLimit(message)) {
        fail('Theme environment exceeds the 1 MiB message limit')
        return
      }
      if (!stopped) channel.port1.postMessage(message)
    },
    updateTokens(cssText) {
      if (!stopped) channel.port1.postMessage({ type: 'tokens.update', cssText })
    },
    emit: sendEvent,
    scrollToTop() {
      if (!stopped) channel.port1.postMessage({ type: 'scroll.toTop' })
    },
    dispose() {
      if (stopped) return Promise.resolve()
      sendEvent('theme.disposing')
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

function cloneThemeMessage<T>(value: T): T {
  const serialized = JSON.stringify(value)
  if (serialized === undefined)
    throw new Error('Theme runtime messages must contain JSON data')
  return JSON.parse(serialized) as T
}
