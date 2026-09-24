declare module 'css-tree' {
  export interface CssNode {
    type: string
    name?: string
    value?: { type?: string; value: string }
  }

  const cssTree: {
    parse(source: string, options?: { positions?: boolean; parseValue?: boolean }): CssNode
    walk(tree: CssNode, visitor: (node: CssNode) => void): void
    generate(tree: CssNode): string
  }
  export default cssTree
}
