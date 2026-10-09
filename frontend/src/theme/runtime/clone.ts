/**
 * Theme runtime messages and snapshots cross a boundary that only accepts plain
 * data. A Vue reactive proxy (for example the snapshot's `error`, which the Core
 * assigns to a ref) is not structured-cloneable and throws, and a theme should
 * never receive a proxy into the Core's own state. A JSON round-trip produces the
 * plain copy both paths need.
 */
export function cloneThemeMessage<T>(value: T): T {
  const serialized = JSON.stringify(value)
  if (serialized === undefined)
    throw new Error('Theme runtime messages must contain JSON data')
  return JSON.parse(serialized) as T
}
