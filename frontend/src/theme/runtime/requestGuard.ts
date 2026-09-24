export function createThemeRequestGuard(limit = 4096) {
  const requestIds = new Set<string>()

  return {
    remember(requestId: string): boolean {
      if (requestIds.has(requestId)) return false
      if (requestIds.size >= limit) throw new Error('Theme request history limit exceeded')
      requestIds.add(requestId)
      return true
    },
  }
}
