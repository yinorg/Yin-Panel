import type { ThemeCollectionStatus, ThemeGroup, ThemeHomePresentation, ThemeHomeSnapshot, ThemeItem, ThemeSearchEngine, ThemeSpace } from '../../theme/api/v1'
// @ts-expect-error Node's strip-types runner requires an explicit source extension.
import { toThemeItemIcon } from './iconifyResource.ts'
// @ts-expect-error Node's strip-types runner requires an explicit source extension.
import { defaultFooterHtml } from '../../constants/panelFooter.ts'

// The pre-theme default brand name. It is treated as "not customised" so the
// name can follow the active panel side (Yin-Panel / Yang-Panel).
const DEFAULT_LOGO_TEXT = 'Yin-Panel'

function configuredLogoText(value: unknown): string {
  return typeof value === 'string' && value.trim() !== '' ? value : DEFAULT_LOGO_TEXT
}

interface SearchEngineSource {
  id?: string
  title?: string
  iconSrc?: string
  url?: string
}

interface PresentationSource {
  homeLayout?: string
  iconStyle?: number | string
  iconTextColor?: string
  iconTextInfoHideDescription?: boolean
  iconTextIconHideTitle?: boolean
  logoText?: string
  logoImageSrc?: string
  clockShow?: boolean
  clockShowSecond?: boolean
  clockColor?: string
  searchBoxShow?: boolean
  searchBoxSearchIcon?: boolean
  marginTop?: number
  marginBottom?: number
  maxWidth?: number
  maxWidthUnit?: string
  marginX?: number
  footerHtml?: string
  systemMonitorShow?: boolean
  systemMonitorShowTitle?: boolean
  /** Current LAN/WAN mode; 'wan' unless the Core reports otherwise. */
  networkMode?: string
  netModeChangeButtonShow?: boolean
  /** True for a signed-in user, false for an anonymous public-link visitor. */
  signedIn?: boolean
}

interface SearchConfigurationSource {
  engines?: readonly SearchEngineSource[]
  currentSearchEngine?: SearchEngineSource
}

interface SpaceSource {
  id: number
  name: string
  side?: 'yin' | 'yang'
  pairedSpaceId?: number
  canSelect?: boolean
}

interface GroupSource {
  id: number
  parentId?: number | null
  title?: string
  icon?: string
  items?: readonly ItemSource[]
}

interface ItemSource {
  id: number
  title: string
  description?: string
  icon?: { itemType: number; src?: string; fileName?: string; text?: string; backgroundColor?: string; resolvedSrc?: string } | null
  sort?: number
}

export function createThemeHomeSnapshot(input: {
  version: number
  status: ThemeCollectionStatus
  spaces: readonly SpaceSource[]
  activeSpaceId?: number
  activeSpaceSide?: 'yin' | 'yang'
  activeSpacePairedId?: number
  activeSpaceCanEdit?: boolean
  activeSpaceCapabilities?: readonly string[]
  groups: readonly GroupSource[]
  canWrite: boolean
  canWriteGroups?: boolean
  /** Permissions the theme is authorized to use at all, independent of the active
   *  Space. Core-surface entries key off this so a theme mounted without a grant
   *  can hide entries it could never use. */
  permissions?: readonly string[]
  /** Viewport height through the bottom edge of Core's fixed monitor layer. */
  monitorReservedHeight?: number
  presentation?: PresentationSource
  searchConfiguration?: SearchConfigurationSource
  error?: { code: string; message: string }
}): ThemeHomeSnapshot {
  const spaces: ThemeSpace[] = input.spaces.map(space => ({
    id: String(space.id),
    name: space.name,
    side: space.side,
    pairedSpaceId: space.pairedSpaceId === undefined ? undefined : String(space.pairedSpaceId),
    capabilities: space.canSelect === false ? [] : ['space.select'],
  }))
  const groups: ThemeGroup[] = []
  const items: ThemeItem[] = []
  for (const group of input.groups) {
    const groupId = String(group.id)
    groups.push({
      id: groupId,
      spaceId: input.activeSpaceId === undefined ? '' : String(input.activeSpaceId),
      parentId: group.parentId == null ? undefined : String(group.parentId),
      title: group.title || '',
      icon: group.icon,
      itemIds: (group.items || []).map(item => String(item.id)),
      capabilities: input.activeSpaceCanEdit === true && input.canWriteGroups ? ['groups.write'] : [],
    })
    for (const item of group.items || []) {
      items.push({
        id: String(item.id),
        groupId,
        title: item.title,
        description: item.description,
        icon: toThemeItemIcon(item.icon),
        sort: item.sort ?? 0,
        capabilities: input.activeSpaceCanEdit === true && input.canWrite ? ['item.open', 'item.update', 'item.delete'] : ['item.open'],
      })
    }
  }
  const presentation = mapPresentation(input.presentation, input.searchConfiguration, input.monitorReservedHeight, input.activeSpaceSide)
  const capabilities = [
    ...(input.activeSpaceCanEdit === true && input.canWrite ? ['items.write'] : []),
    ...(input.activeSpaceCanEdit === true && input.canWriteGroups ? ['groups.write'] : []),
  ]
  return {
    version: input.version,
    status: input.status,
    error: input.error,
    spaces,
    activeSpaceId: input.activeSpaceId === undefined ? undefined : String(input.activeSpaceId),
    activeSpaceSide: input.activeSpaceSide,
    activeSpacePairedId: input.activeSpacePairedId === undefined ? undefined : String(input.activeSpacePairedId),
    activeSpaceCapabilities: input.activeSpaceCapabilities || [],
    groups,
    items,
    ...(capabilities.length ? { capabilities } : {}),
    ...(input.permissions ? { permissions: input.permissions } : {}),
    ...(presentation ? { presentation } : {}),
  }
}

function mapPresentation(source?: PresentationSource, searchConfiguration?: SearchConfigurationSource, monitorReservedHeight?: number, activeSide?: 'yin' | 'yang'): ThemeHomePresentation | undefined {
  if (!source && !searchConfiguration) return undefined
  const defaults = {
    homeLayout: 'standard',
    iconStyle: 1,
    iconTextColor: '#ffffff',
    iconTextInfoHideDescription: false,
    iconTextIconHideTitle: false,
    logoText: 'Yin-Panel',
    logoImageSrc: '',
    clockShow: true,
    clockShowSecond: true,
    searchBoxShow: true,
    searchBoxSearchIcon: true,
    marginTop: 10,
    marginBottom: 10,
    maxWidth: 1200,
    maxWidthUnit: 'px',
    marginX: 5,
    footerHtml: defaultFooterHtml,
    systemMonitorShow: false,
    systemMonitorShowTitle: true,
    ...source,
  }
  const engines: ThemeSearchEngine[] = (searchConfiguration?.engines || []).flatMap((engine) => {
    if (!engine.id || !engine.title || !engine.iconSrc) return []
    return [{ id: engine.id, title: engine.title, iconSrc: engine.iconSrc }]
  })
  const selectedId = searchConfiguration?.currentSearchEngine?.url
    ? engines.find((engine) => {
        const source = searchConfiguration.engines?.find(candidate => candidate.id === engine.id)
        return !!source && source.title === searchConfiguration.currentSearchEngine?.title
          && source.url === searchConfiguration.currentSearchEngine?.url
          && source.iconSrc === searchConfiguration.currentSearchEngine?.iconSrc
      })?.id
    : undefined
  const maxWidthUnit = ['px', '%', 'rem', 'vw'].includes(defaults.maxWidthUnit || '')
    ? defaults.maxWidthUnit as ThemeHomePresentation['content']['maxWidthUnit']
    : 'px'
  return {
    layout: defaults.homeLayout === 'directory' ? 'directory' : 'standard',
    iconStyle: defaults.iconStyle === 0 || defaults.iconStyle === 'info' ? 'info' : 'icon',
    iconTextColor: optionalText(defaults.iconTextColor),
    iconTextInfoHideDescription: defaults.iconTextInfoHideDescription === true,
    iconTextIconHideTitle: defaults.iconTextIconHideTitle === true,
    // 'Yin-Panel' is the built-in default rather than a user choice. Whenever
    // the configured text is still that default, derive the name from the
    // active side so switching to Yang-Panel updates the theme's logo; a
    // genuinely customised logo is passed through untouched.
    logoText: configuredLogoText(defaults.logoText) === DEFAULT_LOGO_TEXT
      ? (activeSide === 'yang' ? 'Yang-Panel' : DEFAULT_LOGO_TEXT)
      : configuredLogoText(defaults.logoText),
    logoImageSrc: optionalText(defaults.logoImageSrc),
    clock: {
      visible: defaults.clockShow !== false,
      showSeconds: defaults.clockShowSecond !== false,
      color: optionalText(defaults.clockColor),
    },
    search: {
      visible: defaults.searchBoxShow !== false,
      itemFilterEnabled: defaults.searchBoxSearchIcon !== false,
      engines,
      currentEngineId: selectedId && engines.some(engine => engine.id === selectedId) ? selectedId : undefined,
    },
    content: {
      marginTopPercent: boundedFinite(defaults.marginTop, 10, 0, 100),
      marginBottomPercent: boundedFinite(defaults.marginBottom, 10, 0, 100),
      maxWidth: boundedFinite(defaults.maxWidth, 1200, 1, 100000),
      maxWidthUnit,
      marginX: boundedFinite(defaults.marginX, 5, 0, 10000),
    },
    footerHtml: typeof defaults.footerHtml === 'string' ? defaults.footerHtml : '',
    monitor: {
      visible: defaults.systemMonitorShow === true,
      showTitle: defaults.systemMonitorShowTitle !== false,
      ...(monitorReservedHeight !== undefined ? { reservedHeight: boundedFinite(monitorReservedHeight, 0, 0, 100000) } : {}),
    },
    // 'lan' is the only value that differs; anything else means WAN.
    network: {
      mode: defaults.networkMode === 'lan' ? 'lan' : 'wan',
      switchVisible: defaults.netModeChangeButtonShow === true,
    },
    signedIn: defaults.signedIn === true,
  }
}

function optionalText(value: unknown): string | undefined {
  return typeof value === 'string' && value.length > 0 ? value : undefined
}

function boundedFinite(value: unknown, fallback: number, min: number, max: number): number {
  const number = typeof value === 'number' ? value : Number(value)
  if (!Number.isFinite(number) || number < min || number > max) return fallback
  return number
}
