// css-tree does not ship type declarations, so this mirrors the named exports
// the theme runtime relies on. The package exposes properties only (no default
// export), so it is imported as a namespace and consumed via these functions.
declare module 'css-tree' {
  export interface CssNode {
    type: string
    name?: string
    value?: string | { type?: string, value: string }
  }

  export function parse(source: string, options?: { positions?: boolean, parseValue?: boolean, context?: string }): CssNode
  export function walk(tree: CssNode, visitor: (node: CssNode) => void): void
  export function generate(tree: CssNode): string
}
