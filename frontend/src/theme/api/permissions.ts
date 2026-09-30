import type { ThemeCommand, ThemePermission } from './v1'

const commandPermissions: Record<ThemeCommand, ThemePermission> = {
  'space.select': 'spaces.read',
  'space.toggleSide': 'spaces.read',
  'item.open': 'items.read',
  'item.create': 'items.write',
  'item.update': 'items.write',
  'item.delete': 'items.write',
  'items.reorder': 'items.write',
  'group.create': 'groups.write',
  'group.update': 'groups.write',
  'group.delete': 'groups.write',
  'groups.reorder': 'groups.write',
  'search.submit': 'items.read',
  'data.refresh': 'groups.read',
  'editor.open': 'items.write',
  'commandCenter.open': 'items.read',
  'ui.openCoreSurface': 'preferences.read',
  // The network mode (LAN/WAN) is a user-level display/navigation preference.
  'network.setMode': 'preferences.write',
  // Geometry telemetry: the theme telling the Core where its layout landed.
  'layout.report': 'diagnostics.report',
  // Gated like the command centre, which is the only thing a forwarded key can
  // open, so losing the grant is a visible no-op rather than a silent bypass.
  'input.forwardKey': 'items.read',
  // Same class of action as `item.open`: following a link out of the panel.
  'link.open': 'items.read',
}

export function requiredCommandPermission(command: ThemeCommand): ThemePermission {
  return commandPermissions[command]
}
