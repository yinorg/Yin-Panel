import { createThemeApiExecutionDispatcher } from '../api/dispatcher'
import { createThemeApiDirectTransport } from '../api/directTransport'
import { createThemeApiClient, type ThemeAPI, type ThemeDefinition, type ThemeEnvironment, type ThemeEventName, type ThemeHomeSnapshot, type ThemeModule, type ThemeMountedView, type ThemePermission } from '../../../packages/theme-sdk/src/index'

const START_TIMEOUT = 10_000
const DISPOSE_TIMEOUT = 2_000

export interface ThemeDirectOptions {
  host: HTMLElement
  script: string
  styles: readonly { text: string }[]
  tokens: string
  snapshot: ThemeHomeSnapshot
  environment: Omit<ThemeEnvironment, 'apiVersion'>
  permissions: ReadonlySet<ThemePermission>
  execute: (request: unknown) => Promise<unknown>
  onError: (error: Error) => void
  contributions?: { views?: readonly string[]; regions?: readonly string[]; components?: readonly string[] }
  /**
   * `true` for the fullscreen trusted-route takeover: the host becomes its own
   * fixed scroll container. `false` (the default) keeps the host in normal flow
   * so the Core's shell owns the page scroll and the monitor layer keeps
   * scrolling with the theme exactly as it did over the sandbox frame.
   */
  takeover?: boolean
  /**
   * Mount the theme into the document (light DOM) instead of a shadow root, so
   * Core components it embeds are styled by the document's own CSS. The theme's
   * stylesheet must already be scoped with `scope` (see `rewriteThemeStylesheet`).
   */
  light?: boolean
  /** The container class the light-DOM styles are scoped to. */
  scope?: string
}

export interface ThemeDirectHandle {
  update: (snapshot: ThemeHomeSnapshot) => void
  updateEnvironment: (environment: Omit<ThemeEnvironment, 'apiVersion'>) => void
  updateTokens: (cssText: string) => void
  emit: (name: ThemeEventName, payload?: unknown) => void
  scrollToTop: () => void
  dispose: () => Promise<void>
}

export async function mountThemeDirect(options: ThemeDirectOptions): Promise<ThemeDirectHandle> {
  let snapshot = structuredClone(options.snapshot)
  let environment = structuredClone({ ...options.environment, apiVersion: '1.0.0' as const })
  let disposed = false
  let moduleURL = ''
  let mounted: ThemeMountedView | undefined
  const mountedRegions: ThemeMountedView[] = []
  const mountedComponents: ThemeMountedView[] = []
  let definition: ThemeDefinition | undefined
  let eventSequence = 0
  const light = options.light === true
  const scopeClass = light ? (options.scope || `yin-theme-${Math.random().toString(36).slice(2, 10)}`) : ''
  const shadow = light ? undefined : (options.host.shadowRoot || options.host.attachShadow({ mode: 'open' }))
  const root = document.createElement('div')
  root.id = 'theme-root'
  const selector = light ? `.${scopeClass}` : ':host'
  const rootSelector = light ? `.${scopeClass} #theme-root` : '#theme-root'
  const universalSelector = light ? `.${scopeClass} *` : '*'
  const baseStyle = document.createElement('style')
  const hostBox = options.takeover
    ? `${selector}{display:block;position:absolute;inset:0;overflow:auto}`
    : `${selector}{display:block;position:relative;min-height:100%;overflow:visible}`
  baseStyle.textContent = `${hostBox} ${selector},${rootSelector}{box-sizing:border-box;min-height:100%;margin:0}${universalSelector},${universalSelector}::before,${universalSelector}::after{box-sizing:inherit}${rootSelector}{min-height:100dvh}`
  const tokenStyle = document.createElement('style')
  tokenStyle.textContent = options.tokens.replace(/:root\b/g, light ? `.${scopeClass}` : ':host')
  const themeStyleSheets: HTMLStyleElement[] = []
  for (const sheet of options.styles) {
    const style = document.createElement('style')
    style.textContent = sheet.text
    themeStyleSheets.push(style)
  }
  if (light) {
    options.host.classList.add(scopeClass)
    document.head.append(baseStyle, tokenStyle, ...themeStyleSheets)
    options.host.append(root)
  }
  else {
    shadow!.replaceChildren(baseStyle, root, tokenStyle, ...themeStyleSheets)
  }
  const teardownDom = () => {
    root.remove()
    for (const style of [baseStyle, tokenStyle, ...themeStyleSheets]) style.remove()
    if (light) options.host.classList.remove(scopeClass)
    else shadow!.replaceChildren()
  }

  const dispatcher = createThemeApiExecutionDispatcher({
    getContextVersion: () => snapshot.version,
    getPermissions: () => options.permissions,
    execute: options.execute,
  })
  const transport = createThemeApiDirectTransport(dispatcher)
  const apiClient = createThemeApiClient({
    request: request => transport(request),
    getSnapshot: () => snapshot,
    getEnvironment: () => environment,
  })

  try {
    moduleURL = URL.createObjectURL(new Blob([options.script], { type: 'text/javascript' }))
    const loaded = await withTimeout(import(/* @vite-ignore */ moduleURL), START_TIMEOUT, 'Theme module import timed out') as { default?: ThemeModule }
    const themeModule = loaded.default
    if (!themeModule || themeModule.apiVersion !== '1.0.0' || typeof themeModule.setup !== 'function')
      throw new Error('Theme entrypoint must export a Theme API v1 module as default')
    const setupTask = Promise.resolve(themeModule.setup(apiClient.api as ThemeAPI))
    try {
      definition = await withTimeout(setupTask, START_TIMEOUT, 'Theme setup timed out')
    }
    catch (error) {
      void setupTask.then(lateDefinition => lateDefinition?.dispose?.()).catch(() => undefined)
      throw error
    }
    validateContributions(definition, options.contributions)
    const view = definition?.views?.home
    if (typeof view !== 'function') throw new Error('Theme does not register the contributed home view')
    const mountTask = Promise.resolve(view(root, apiClient.api as ThemeAPI, structuredClone(snapshot)))
    try {
      mounted = await withTimeout(mountTask, START_TIMEOUT, 'Theme home view mount timed out')
    }
    catch (error) {
      void mountTask.then(lateView => lateView?.unmount?.()).catch(() => undefined)
      throw error
    }
    if (!mounted || typeof mounted.unmount !== 'function') throw new Error('Theme home view must return an unmount function')
    for (const region of options.contributions?.regions || []) {
      const slot = document.createElement('section')
      slot.dataset.themeRegion = region
      slot.dataset.themeRegionSlot = region
      root.append(slot)
      const factory = definition?.regions?.[region]
      if (typeof factory !== 'function') throw new Error(`Theme does not register contributed region ${region}`)
      const regionView = await withTimeout(Promise.resolve(factory(slot, apiClient.api as ThemeAPI, structuredClone(snapshot))), START_TIMEOUT, `Theme region ${region} mount timed out`)
      if (!regionView || typeof regionView.unmount !== 'function') throw new Error(`Theme region ${region} must return an unmount function`)
      mountedRegions.push(regionView)
    }
    for (const component of options.contributions?.components || []) {
      const slot = document.createElement('section')
      slot.dataset.themeComponent = component
      slot.dataset.themeComponentSlot = component
      root.append(slot)
      const factory = definition?.components?.[component]
      if (typeof factory !== 'function') throw new Error(`Theme does not register contributed component ${component}`)
      const componentView = await withTimeout(Promise.resolve(factory(slot, apiClient.api as ThemeAPI, structuredClone(snapshot))), START_TIMEOUT, `Theme component ${component} mount timed out`)
      if (!componentView || typeof componentView.unmount !== 'function') throw new Error(`Theme component ${component} must return an unmount function`)
      mountedComponents.push(componentView)
    }
  }
  catch (error) {
    disposed = true
    apiClient.dispose()
    await Promise.allSettled([
      withTimeout(Promise.all(mountedRegions.map(region => region.unmount())), DISPOSE_TIMEOUT, 'Theme region disposal timed out'),
      withTimeout(Promise.all(mountedComponents.map(component => component.unmount())), DISPOSE_TIMEOUT, 'Theme component disposal timed out'),
      withTimeout(Promise.resolve(mounted?.unmount()), DISPOSE_TIMEOUT, 'Theme view disposal timed out'),
      withTimeout(Promise.resolve(definition?.dispose?.()), DISPOSE_TIMEOUT, 'Theme module disposal timed out'),
    ])
    teardownDom()
    if (moduleURL) URL.revokeObjectURL(moduleURL)
    throw error
  }

  return {
    update(next) {
      if (disposed) return
      snapshot = structuredClone(next)
      apiClient.updateSnapshot(snapshot)
      void Promise.resolve(mounted?.update?.(structuredClone(snapshot))).catch((error) => {
        options.onError(error instanceof Error ? error : new Error(String(error)))
      })
      for (const region of mountedRegions)
        void Promise.resolve(region.update?.(structuredClone(snapshot))).catch(error => options.onError(error instanceof Error ? error : new Error(String(error))))
      for (const component of mountedComponents)
        void Promise.resolve(component.update?.(structuredClone(snapshot))).catch(error => options.onError(error instanceof Error ? error : new Error(String(error))))
    },
    updateEnvironment(next) {
      if (disposed) return
      environment = structuredClone({ ...next, apiVersion: '1.0.0' as const })
      apiClient.updateEnvironment(environment)
    },
    updateTokens(cssText) {
      if (!disposed) tokenStyle.textContent = cssText.replace(/:root\b/g, light ? `.${scopeClass}` : ':host')
    },
    emit(name, payload) {
      if (disposed) return
      apiClient.emit(name, { contextVersion: snapshot.version, sequence: ++eventSequence, payload })
    },
    scrollToTop() {
      if (disposed) return
      const scroller = light ? options.host : shadow!.host
      scroller.scrollTo({ top: 0, behavior: 'smooth' })
      root.scrollTo({ top: 0, behavior: 'smooth' })
    },
    async dispose() {
      if (disposed) return
      disposed = true
      apiClient.emit('theme.disposing', { contextVersion: snapshot.version, sequence: ++eventSequence, payload: undefined })
      await Promise.allSettled([
        withTimeout(Promise.all(mountedRegions.map(region => region.unmount())), DISPOSE_TIMEOUT, 'Theme region disposal timed out'),
        withTimeout(Promise.all(mountedComponents.map(component => component.unmount())), DISPOSE_TIMEOUT, 'Theme component disposal timed out'),
        withTimeout(Promise.resolve(mounted?.unmount()), DISPOSE_TIMEOUT, 'Theme view disposal timed out'),
        withTimeout(Promise.resolve(definition?.dispose?.()), DISPOSE_TIMEOUT, 'Theme module disposal timed out'),
      ])
      apiClient.dispose()
      teardownDom()
      if (moduleURL) URL.revokeObjectURL(moduleURL)
    },
  }
}

function validateContributions(definition: ThemeDefinition | undefined, contributions: ThemeDirectOptions['contributions']) {
  if (!contributions) return
  for (const view of contributions.views || []) if (typeof definition?.views?.[view as keyof NonNullable<ThemeDefinition['views']>] !== 'function') throw new Error(`Theme does not register contributed view ${view}`)
  for (const region of contributions.regions || []) if (typeof definition?.regions?.[region] !== 'function') throw new Error(`Theme does not register contributed region ${region}`)
  for (const component of contributions.components || []) if (!definition?.components || !(component in definition.components)) throw new Error(`Theme does not register contributed component ${component}`)
}

function withTimeout<T>(promise: Promise<T>, timeout: number, message: string): Promise<T> {
  return new Promise((resolve, reject) => {
    const timer = window.setTimeout(() => reject(new Error(message)), timeout)
    promise.then(
      value => { window.clearTimeout(timer); resolve(value) },
      error => { window.clearTimeout(timer); reject(error) },
    )
  })
}
