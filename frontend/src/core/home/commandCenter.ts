export interface HomeCommandDefinition {
  key: string
  label: string
}

export interface HomeCommandItem {
  id?: number
  title?: string
  url?: string
  description?: string
}

export function filterHomeCommands<Command extends HomeCommandDefinition>(commands: Command[], query: string): Command[] {
  const needle = query.replace(/^\//, '').trim().toLowerCase()
  return commands.filter(command => command.key.includes(needle) || command.label.toLowerCase().includes(needle))
}

export function filterHomeCommandItems<Item extends HomeCommandItem>(query: string, remote: Item[], local: Item[]): Item[] {
  const needle = query.trim().toLowerCase()
  if (!needle || query.startsWith('/')) return []
  const seen = new Set<number>()
  return [...remote, ...local].filter((item) => {
    if (item.id !== undefined && seen.has(Number(item.id))) return false
    if (item.id !== undefined) seen.add(Number(item.id))
    return [item.title, item.url, item.description].some(value => value?.toLowerCase().includes(needle))
  })
}

export function findHomeCommandItem<Item extends HomeCommandItem>(query: string, remote: Item[], local: Item[]): Item | undefined {
  const needle = query.trim().toLowerCase()
  return [...remote, ...local].find(item => !needle || [item.title, item.url, item.description].some(value => value?.toLowerCase().includes(needle)))
}

export function parseHomeCommand(query: string): { command: string; argument: string } {
  const [command = '', ...parts] = query.trim().replace(/^\//, '').split(/\s+/)
  return { command, argument: parts.join(' ') }
}

export function isHomeCommandWrite(command: string): boolean {
  return ['add', 'group', 'space', 'settings', 'edit'].includes(command)
}

export function getInitialHomeCommandSelection(query: string): number {
  return query.startsWith('/') ? 0 : -1
}

export function moveHomeCommandSelection(current: number, offset: number, length: number): number {
  if (length <= 0) return current
  return ((current + offset) % length + length) % length
}

export function createHomeCommandSearch<Item extends HomeCommandItem>(options: {
  getSpaceId: () => number | undefined
  search: (spaceId: number, query: string) => Promise<Item[]>
}) {
  let generation = 0

  function invalidate() {
    generation++
  }

  async function search(query: string): Promise<Item[] | undefined> {
    const requestGeneration = ++generation
    const keyword = query.trim()
    if (!keyword || query.startsWith('/')) return []
    const spaceId = options.getSpaceId()
    if (spaceId === undefined) return []
    try {
      const result = await options.search(spaceId, keyword)
      if (requestGeneration !== generation || options.getSpaceId() !== spaceId) return undefined
      return result
    }
    catch {
      return requestGeneration === generation ? [] : undefined
    }
  }

  return { search, invalidate }
}
