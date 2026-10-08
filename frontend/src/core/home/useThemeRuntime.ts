import { computed, onUnmounted, ref, watch, type Ref } from 'vue'
import { useDialog } from 'naive-ui'
import { useAuthStore, usePanelState } from '@/store'
import { useRoute } from 'vue-router'
import { getThemeRuntimeGrant, setThemeRuntimeGrant } from '@/api/theme'
import { activeThemePackage, activeThemeSlots, themePackageResolved } from '@/hooks/useTheme'
import { createThemePersistence } from '@/core/home/themePersistence'
import { createThemeSettingsSchemaValidator } from '@/core/home/themeSettingsSchema'
import { createIconifyResourceResolver } from '@/core/home/iconifyResource'
import { createThemeHomeSnapshot } from '@/core/home/themeSnapshot'
import { spaceDisplayName, type Space } from '@/api/panel/space'
import { isThemeSafeMode } from '@/theme/recovery/safeMode'
import { t } from '@/locales'
import { PanelStateNetworkModeEnum } from '@/enums'
import type { ThemeCollectionStatus, ThemeEnvironment, ThemePermission } from '@/theme/api/v1'

export interface HomeSearchEngine {
  id: string
  title: string
  iconSrc: string
  url: string
}

export type ThemeRuntimeEnvironment = Omit<ThemeEnvironment, 'apiVersion'>

/**
 * Everything about running a theme package: whether one may render at all, the
 * permission grant behind it, the snapshot handed to it, and the recovery paths
 * when it fails.
 *
 * The gating and the grant flow are mutually dependent — the grant is keyed to the
 * package revision, while the grant decides whether the package may render — so the
 * pieces that cross that boundary are read through getters and the result is
 * re-read on every change rather than cached in one direction.
 *
 * `executeThemeRequest` deliberately stays in the host: dispatching a theme request
 * needs the command bindings, and those need this composable's state, so the host
 * is the only place that can hold both.
 */
export function useThemeRuntime(input: {
  publicCode: string
  publicAccessReady: Ref<boolean>
  previewTheme: boolean
  // Data channel
  homeReady: Ref<boolean>
  themeSnapshotVersion: Ref<number>
  homeCollectionStatus: Ref<ThemeCollectionStatus>
  homeCollectionError: Ref<{ code: string, message: string } | undefined>
  spaces: Ref<Space[]>
  activeSpace: Ref<Space | null>
  items: Ref<{ id?: number | string, parentId?: number | null, title?: string, icon?: string, sort?: number, items?: readonly Panel.ItemInfo[] }[]>
  canWrite: Ref<boolean>
  /**
   * The Core's own open-address resolver, LAN/WAN and mobile variant included. The
   * theme opens "new window" bookmarks itself inside the tap and therefore needs the
   * address in the snapshot; reusing this is what stops the two paths from choosing
   * different URLs for one item.
   */
  getItemOpenUrl: (item: Panel.ItemInfo) => string
  // Environment and monitor
  environment: Ref<ThemeRuntimeEnvironment>
  /** Lazy: the monitor band composable is created after this one, and this one's
   *  `themeRuntimeActive` is what the monitor asks for in return. */
  getMonitorReservedHeight: () => number
  /** Lazy: the current search engine belongs to the command layer. */
  getSearchConfiguration: () => { engines: HomeSearchEngine[], currentSearchEngine: HomeSearchEngine }
}) {
  const route = useRoute()
  const authStore = useAuthStore()
  const panelState = usePanelState()
  const dialog = useDialog()

  function createThemeRuntimeError(code: string, message: string) {
    return Object.assign(new Error(message), { code })
  }

  const themeRuntimePackage = computed(() => activeThemePackage.value)
  const trustedRouteRevision = computed(() => route.name === 'trustedThemeHome' ? String(route.params.revision || '') : '')
  const themeExecutionMode = computed<'sandbox' | 'trusted'>(() => trustedRouteRevision.value ? 'trusted' : 'sandbox')
  const themeHomeContribution = computed(() => {
    const manifest = themeRuntimePackage.value?.manifest
    return !!manifest?.entrypoints?.script && !!manifest.contributes?.views?.includes('home') && !!manifest.runtime?.supportedModes?.includes(themeExecutionMode.value)
  })
  const themeRequiredPermissions = computed(() => (themeRuntimePackage.value?.manifest.permissions?.required || []).map(permission => permission.name as ThemePermission))

  const themeRuntimeGrant = ref<{ revision: string, executionMode: 'sandbox' | 'trusted', available: boolean, granted: boolean, permissions: ThemePermission[] }>({ revision: '', executionMode: 'sandbox', available: false, granted: false, permissions: [] })
  const trustedRuntimeAvailable = ref(false)
  const themeRuntimeGrantLoading = ref(false)
  const themeRuntimeGrantSaving = ref(false)
  const themeRuntimeConsentVisible = ref(false)
  const selectedThemeExecutionMode = ref<'sandbox' | 'trusted'>('sandbox')
  const trustedRuntimeAcknowledged = ref(false)
  const themeRuntimeFailed = ref(false)
  const themeRuntimeFailureMessage = ref('')
  const themeSafeMode = ref(isThemeSafeMode())
  let themeGrantRequestGeneration = 0
  let trustedPolicyPollTimer: ReturnType<typeof setInterval> | undefined
  let trustedThemeRestoreTimer: ReturnType<typeof setTimeout> | undefined

  const themeGrantMatches = computed(() => {
    const revision = themeRuntimePackage.value?.revision || ''
    return !!revision && themeRuntimeGrant.value.revision === revision && themeRuntimeGrant.value.executionMode === themeExecutionMode.value && themeRuntimeGrant.value.granted && themeRequiredPermissions.value.every(permission => themeRuntimeGrant.value.permissions.includes(permission))
  })

  /** Whether the theme may render at all.
   *
   *  `trusted` runs in the page's own origin, so it keeps the full grant
   *  requirement. `sandbox` runs in a cross-origin opaque-origin frame that holds
   *  no token, cannot reach this document's DOM, and still passes every command
   *  through the permission set — so the grant gates *capabilities*, not
   *  *rendering*. Requiring it to render as well only produced two bad states: a
   *  logged-out visitor on a public link always saw the Core home, and an
   *  unconsented sandbox theme left the user staring at a Core home that the theme
   *  was supposed to replace. Capabilities still stay empty without a grant, so a
   *  grant-free theme is strictly read-only. */
  const themeRuntimeAuthorized = computed(() => themeExecutionMode.value === 'sandbox' || themeGrantMatches.value)
  const themeRuntimeActive = computed(() => input.homeReady.value && !themeSafeMode.value && themeHomeContribution.value && themeRuntimeAuthorized.value && !themeRuntimeFailed.value
    // A public link may still be waiting on the access-code prompt; the Core has
    // not decided the visitor may see anything yet, so do not mount the theme.
    && (!input.publicCode || input.publicAccessReady.value))
  const themeRuntimeNeedsConsent = computed(() => input.homeReady.value && !themeSafeMode.value && themeHomeContribution.value && !!authStore.token && !input.publicCode && !themeRuntimeGrantLoading.value && !themeGrantMatches.value)

  /** Boundary C3: the theme cannot render the home, so the minimum viable list
   *  takes over instead of leaving a blank page.
   *
   *  Every reason the theme cannot render routes here, including
   *  `!themeHomeContribution` (no package loaded, or a theme that declares no home
   *  view). P3b deliberately narrowed the trigger to "the theme was going to render
   *  and could not" and left that case to the Core home, which still existed then;
   *  P4a removed that home, so this is now the only remaining owner of the home
   *  when the theme cannot take it. */
  // Only decide "the theme cannot render the home" once the package load has
  // settled. Before that the package is simply still in flight, and treating it
  // as a missing home view flashed the fallback on every cold load.
  const homeFallbackVisible = computed(() => input.homeReady.value && themePackageResolved.value && !themeRuntimeActive.value
    && (themeSafeMode.value || themeRuntimeFailed.value || !themeHomeContribution.value))
  /** Why the theme is not rendering, in the user's language. Shown under the
   *  fallback's notice so the state is never unexplained. */
  const homeFallbackReason = computed(() => {
    if (themeSafeMode.value) return t('panelHome.fallbackReasonSafeMode')
    if (themeRuntimeFailed.value) return themeRuntimeFailureMessage.value || t('themePackage.runtimeLoadFailed')
    return t('panelHome.fallbackReasonNoHomeView')
  })

  /** Permissions an anonymous public-link visitor implicitly holds for the space the
   *  link exposes. The Core already serves exactly this data to anonymous visitors
   *  over HTTP, so handing it to the theme exposes nothing new; without it the
   *  snapshot would blank every collection and the visitor would stare at an empty
   *  home. Writes are never implied. `diagnostics.report` is included because it
   *  only carries the theme's own geometry back to the Core — which is what keeps
   *  the monitor layer from falling back to a formula that overlaps the search box. */
  const PUBLIC_VISITOR_PERMISSIONS: ThemePermission[] = ['spaces.read', 'groups.read', 'items.read', 'diagnostics.report']
  const themeRuntimePermissions = computed(() => {
    if (themeGrantMatches.value) return themeRuntimeGrant.value.permissions
    // A public link has no user, so no grant record can ever exist for it. Require
    // the Core's own access decision first: a `code` link must not hand out read
    // scopes before the visitor has unlocked it.
    return input.publicCode && input.publicAccessReady.value ? PUBLIC_VISITOR_PERMISSIONS : []
  })

  const themeCanWriteGroups = computed(() => input.canWrite.value && input.activeSpace.value?.canEdit === true && themeRuntimePackage.value?.manifest.id === 'org.yin.default' && themeRuntimePermissions.value.includes('groups.write'))

  // `?yinDiag` 只在这个标签页里生效，不落盘、不进配置，所以关掉标签页就没了。
  // 它存在的意义是让「点了到底发生什么」可以直接看见：主题跑在 opaque 沙箱里，
  // 从外部（父文档、顶层 Console、其他工具）一律读不到它内部发生了什么。
  //
  // 必须同步求值，不能放进 `onMounted`：主题快照是这个 composable 内部的 computed，
  // 它可能早于挂载就被求值一次，那时开关还是 false，首帧快照就不带 diagnostics，
  // 而主题只在收到 init 那一刻读一次，之后不会再纠正。
  const themeClickTraceEnabled = (() => {
    try {
      return new URL(window.location.href).searchParams.has('yinDiag')
    } catch {
      return false
    }
  })()
  const themeRuntimeSlots = computed(() => activeThemeSlots.value)

  // Iconify identifiers in item icons need resolving to usable sources before the
  // snapshot goes out; the resolution is async and must not publish stale results.
  const iconifyResourceResolver = createIconifyResourceResolver()
  const resolvedThemeIconResources = ref<Record<string, string>>({})
  watch([themeRuntimeActive, () => [...new Set(input.items.value.flatMap(group => (group.items || [])
    .filter(item => item.icon?.itemType === 3 && typeof item.icon.text === 'string')
    .map(item => item.icon?.text || '')))].sort().join('|')], ([runtimeActive, iconIdentifiers]) => {
    const generation = iconifyResourceResolver.beginGeneration()
    if (!runtimeActive) {
      resolvedThemeIconResources.value = {}
      return
    }
    const identifiers = iconIdentifiers ? iconIdentifiers.split('|') : []
    void Promise.all(identifiers.map(async (identifier) => {
      const resource = await iconifyResourceResolver.resolve(identifier, generation)
      return resource ? [identifier, resource] as const : undefined
    })).then((resolved) => {
      if (!iconifyResourceResolver.isCurrentGeneration(generation)) return
      resolvedThemeIconResources.value = Object.fromEntries(resolved.filter((entry): entry is readonly [string, string] => !!entry))
    })
  }, { immediate: true })
  onUnmounted(() => iconifyResourceResolver.cancel())

  const themeRuntimeSnapshot = computed(() => createThemeHomeSnapshot({
    version: input.themeSnapshotVersion.value,
    status: input.homeCollectionStatus.value,
    error: input.homeCollectionError.value,
    spaces: input.spaces.value.map(space => ({ ...space, name: spaceDisplayName(space, input.spaces.value, authStore.userInfo?.id) })),
    activeSpaceId: input.activeSpace.value?.id,
    activeSpaceSide: input.activeSpace.value?.side,
    activeSpacePairedId: input.activeSpace.value?.pairedSpaceId,
    activeSpaceCanEdit: input.activeSpace.value?.canEdit === true && !input.publicCode,
    activeSpaceCapabilities: input.activeSpace.value?.pairedSpaceId ? ['space.toggleSide'] : [],
    groups: input.items.value.filter(group => Number.isSafeInteger(Number(group.id))).map(group => ({
      ...group,
      id: Number(group.id),
      items: (group.items || []).filter(item => Number.isSafeInteger(Number(item.id))).map(item => ({
        ...item,
        id: Number(item.id),
        icon: item.icon ? { ...item.icon, resolvedSrc: item.icon.text ? resolvedThemeIconResources.value[item.icon.text] : undefined } : item.icon,
      })),
    })),
    canWrite: input.canWrite.value && themeRuntimePermissions.value.includes('items.write'),
    canWriteGroups: themeCanWriteGroups.value,
    // The theme opens "new window" bookmarks itself, inside the tap, so it needs the
    // same address the Core would open. Reusing the Core's own resolver keeps LAN/WAN
    // and the mobile variant from drifting between the two.
    resolveOpenUrl: item => input.getItemOpenUrl(item as unknown as Panel.ItemInfo),
    // 诊断开关来自面板 URL 的 `?yinDiag`。必须由 Core 读、再经快照送进主题：
    // 沙箱里的主题读不到面板 URL（`about:srcdoc` 无查询串、referrer 为空、
    // `parent.location` 抛 SecurityError），快照是唯一通道。
    showClickTrace: themeClickTraceEnabled,
    permissions: themeRuntimePermissions.value,
    presentation: {
      ...panelState.panelConfig,
      networkMode: panelState.networkMode === PanelStateNetworkModeEnum.lan ? 'lan' : 'wan',
      signedIn: !!authStore.token,
    },
    monitorReservedHeight: input.getMonitorReservedHeight(),
    searchConfiguration: input.getSearchConfiguration(),
  }))

  // Resolve the grant whenever the package revision, the session or the route mode
  // changes. A trusted route additionally polls, because losing the policy while it
  // runs must drop back to the normal home rather than keep a privileged frame alive.
  watch([() => themeRuntimePackage.value?.revision, () => authStore.token, trustedRouteRevision], async ([revision, token, trustedRoute]) => {
    if (trustedThemeRestoreTimer) clearTimeout(trustedThemeRestoreTimer)
    trustedThemeRestoreTimer = undefined
    const requestGeneration = ++themeGrantRequestGeneration
    themeRuntimeFailed.value = false
    themeRuntimeFailureMessage.value = ''
    const executionMode = trustedRoute ? 'trusted' : 'sandbox'
    themeRuntimeGrant.value = { revision: revision || '', executionMode, available: false, granted: false, permissions: [] }
    trustedRuntimeAvailable.value = false
    themeRuntimeGrantLoading.value = false
    if (trustedRoute && (!token || input.publicCode || input.previewTheme)) {
      window.location.replace('/')
      return
    }
    if (trustedRoute && (!revision || trustedRoute !== revision || !themeHomeContribution.value)) {
      trustedThemeRestoreTimer = setTimeout(() => {
        if (trustedRouteRevision.value && (themeRuntimePackage.value?.revision !== trustedRouteRevision.value || !themeHomeContribution.value))
          window.location.replace('/')
      }, 10000)
      return
    }
    if (!revision || !token || !themeHomeContribution.value || input.publicCode || input.previewTheme) {
      if (trustedRoute) window.location.replace('/')
      return
    }
    themeRuntimeGrantLoading.value = true
    try {
      const result = await getThemeRuntimeGrant(revision, executionMode)
      if (requestGeneration === themeGrantRequestGeneration && result.code === 0 && result.data.revision === revision) {
        themeRuntimeGrant.value = { revision, executionMode, available: result.data.available, granted: result.data.granted, permissions: result.data.permissions as ThemePermission[] }
        if (executionMode === 'sandbox') {
          try {
            const trustedResult = await getThemeRuntimeGrant(revision, 'trusted')
            if (requestGeneration === themeGrantRequestGeneration && trustedResult.code === 0)
              trustedRuntimeAvailable.value = trustedResult.data.available
          }
          catch { trustedRuntimeAvailable.value = false }
        }
        else if (!result.data.granted || !result.data.available) {
          window.location.replace('/')
        }
      }
    }
    catch {
      if (requestGeneration === themeGrantRequestGeneration) {
        themeRuntimeGrant.value = { revision, executionMode, available: false, granted: false, permissions: [] }
        if (trustedRoute) window.location.replace('/')
      }
    }
    finally {
      if (requestGeneration === themeGrantRequestGeneration)
        themeRuntimeGrantLoading.value = false
    }
  }, { immediate: true })

  watch([trustedRouteRevision, () => authStore.token], ([revision, token]) => {
    if (trustedPolicyPollTimer) clearInterval(trustedPolicyPollTimer)
    trustedPolicyPollTimer = undefined
    if (!revision || !token) return
    trustedPolicyPollTimer = setInterval(async () => {
      try {
        const result = await getThemeRuntimeGrant(revision, 'trusted')
        if (result.code !== 0 || result.data.revision !== revision || !result.data.available || !result.data.granted)
          window.location.replace('/')
      }
      catch {
        window.location.replace('/')
      }
    }, 5000)
  }, { immediate: true })

  const themePersistence = createThemePersistence(localStorage, {
    userId: () => authStore.userInfo?.id,
    packageId: () => activeThemePackage.value?.manifest.id,
    revision: () => activeThemePackage.value?.revision,
    validateSettings: settings => validateActiveThemeSettings(settings),
  })
  const validateThemeSettingsSchema = createThemeSettingsSchemaValidator({ origin: window.location.origin })
  function validateActiveThemeSettings(settings: Record<string, unknown>) {
    return validateThemeSettingsSchema(activeThemePackage.value || undefined, settings)
  }

  function confirmThemeDelete(message: string, action: (spaceId: number) => Promise<unknown>): Promise<unknown> {
    if (!input.canWrite.value || !input.activeSpace.value) throw createThemeRuntimeError('PERMISSION_DENIED', 'The active Space is read-only')
    const confirmedSpaceId = input.activeSpace.value.id
    return new Promise((resolve, reject) => {
      let settled = false
      dialog.warning({
        title: t('common.warning'),
        content: message,
        positiveText: t('common.confirm'),
        negativeText: t('common.cancel'),
        onPositiveClick: async () => {
          settled = true
          try { resolve(await action(confirmedSpaceId)) }
          catch (error) { reject(error) }
        },
        onNegativeClick: () => {
          settled = true
          reject(createThemeRuntimeError('ABORTED', 'Delete was cancelled'))
        },
        onClose: () => {
          if (!settled) reject(createThemeRuntimeError('ABORTED', 'Delete was cancelled'))
        },
      })
    })
  }

  async function grantThemeRuntimePermissions() {
    const revision = themeRuntimePackage.value?.revision
    if (!revision || themeRuntimeGrantSaving.value) return
    const permissions = themeRequiredPermissions.value
    const executionMode = selectedThemeExecutionMode.value
    if (executionMode === 'trusted' && (!trustedRuntimeAvailable.value || !trustedRuntimeAcknowledged.value)) return
    if (input.previewTheme) {
      themeRuntimeGrant.value = { revision, executionMode, available: true, granted: true, permissions }
      return
    }
    themeRuntimeGrantSaving.value = true
    try {
      const result = await setThemeRuntimeGrant(revision, permissions, executionMode)
      if (result.code === 0) {
        if (executionMode === 'trusted') {
          window.location.assign(`/__yin/theme-trusted/${encodeURIComponent(revision)}`)
          return
        }
        themeRuntimeGrant.value = { revision, executionMode, available: true, granted: true, permissions }
        themeRuntimeConsentVisible.value = false
        themeRuntimeFailed.value = false
        themeRuntimeFailureMessage.value = ''
      }
    }
    catch {
      themeRuntimeFailureMessage.value = t('themePackage.runtimeGrantFailed')
    }
    finally {
      themeRuntimeGrantSaving.value = false
    }
  }

  function openThemeRuntimeConsent() {
    selectedThemeExecutionMode.value = 'sandbox'
    trustedRuntimeAcknowledged.value = false
    themeRuntimeConsentVisible.value = true
  }

  function handleThemeRuntimeFailure(error: Error) {
    console.error('Theme sandbox failed to start:', error)
    themeRuntimeFailed.value = true
    themeRuntimeFailureMessage.value = error.message || t('themePackage.runtimeLoadFailed')
  }

  onUnmounted(() => {
    if (trustedPolicyPollTimer) clearInterval(trustedPolicyPollTimer)
    if (trustedThemeRestoreTimer) clearTimeout(trustedThemeRestoreTimer)
  })

  return {
    themeRuntimePackage,
    trustedRouteRevision,
    themeExecutionMode,
    themeHomeContribution,
    themeRequiredPermissions,
    themeGrantMatches,
    themeRuntimeActive,
    themeRuntimeNeedsConsent,
    homeFallbackVisible,
    homeFallbackReason,
    themeRuntimePermissions,
    themeCanWriteGroups,
    themeRuntimeSlots,
    themeRuntimeSnapshot,
    themeRuntimeGrant,
    trustedRuntimeAvailable,
    themeRuntimeGrantLoading,
    themeRuntimeGrantSaving,
    themeRuntimeConsentVisible,
    selectedThemeExecutionMode,
    trustedRuntimeAcknowledged,
    themeRuntimeFailed,
    themeRuntimeFailureMessage,
    themeSafeMode,
    themePersistence,
    createThemeRuntimeError,
    confirmThemeDelete,
    grantThemeRuntimePermissions,
    openThemeRuntimeConsent,
    handleThemeRuntimeFailure,
  }
}
