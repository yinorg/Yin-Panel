export interface OpenableItem {
  url: string
  lanUrl?: string
  mobileUrl?: string
}

export interface ItemOpenContext {
  networkMode: 'lan' | 'wan' | string
  isMobile: boolean
  forceWan?: boolean
}

export function resolveItemOpenUrl(item: OpenableItem, context: ItemOpenContext): string {
  if (!context.forceWan && context.networkMode === 'lan')
    return item.lanUrl || item.url
  return context.isMobile && item.mobileUrl ? item.mobileUrl : item.url
}
