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
}

export function requiredCommandPermission(command: ThemeCommand): ThemePermission {
  return commandPermissions[command]
}
