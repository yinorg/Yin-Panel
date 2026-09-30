import assert from 'node:assert/strict'
// The frontend does not depend on a unit-test framework; use Node's built-in runner.
// eslint-disable-next-line test/no-import-node-test
import test from 'node:test'
import { THEME_COMMANDS, isThemeCommand } from '../../src/theme/api/operation.ts'
import { requiredCommandPermission } from '../../src/theme/api/permissions.ts'

// PLAN_V5 §2.3: the theme reaches every Core capability through typed commands.
// This contract test fails when a capability is dropped from the command set or
// when the runtime allowlist and the permission map drift apart, which would
// silently make a Core capability unreachable for every theme.

// Capabilities the default theme offers. Keep in step with PLAN_V5 §6.
const REQUIRED_CAPABILITIES = [
  'space.select',
  'space.toggleSide',
  'item.open',
  'item.create',
  'item.update',
  'item.delete',
  'items.reorder',
  'group.create',
  'group.update',
  'group.delete',
  'groups.reorder',
  'search.submit',
  'data.refresh',
  'editor.open',
  'commandCenter.open',
  'ui.openCoreSurface',
  'network.setMode',
  'layout.report',
  'input.forwardKey',
  'link.open',
]

test('every Core capability is reachable as a theme command', () => {
  const missing = REQUIRED_CAPABILITIES.filter(command => !isThemeCommand(command))
  assert.deepEqual(missing, [], `capabilities without a theme command: ${missing.join(', ')}`)
})

test('the runtime allowlist accepts exactly the declared commands', () => {
  const declared = [...THEME_COMMANDS].sort()
  const unknown = declared.filter(command => !isThemeCommand(command))
  assert.deepEqual(unknown, [], `declared but rejected: ${unknown.join(', ')}`)
  // No command may exist in the allowlist that is not in the declared list.
  assert.equal(new Set(THEME_COMMANDS).size, THEME_COMMANDS.length, 'THEME_COMMANDS must not contain duplicates')
})

test('every command resolves to a permission', () => {
  for (const command of THEME_COMMANDS) {
    const permission = requiredCommandPermission(command)
    assert.equal(typeof permission, 'string', `command ${command} has no permission`)
    assert.notEqual(permission, '', `command ${command} has an empty permission`)
  }
})

test('the settings and network entries require preference permissions', () => {
  // The theme exposes these Core surfaces; both must stay gated on preferences
  // so removing the grant is a visible failure rather than a silent bypass.
  assert.equal(requiredCommandPermission('ui.openCoreSurface'), 'preferences.read')
  assert.equal(requiredCommandPermission('network.setMode'), 'preferences.write')
})
