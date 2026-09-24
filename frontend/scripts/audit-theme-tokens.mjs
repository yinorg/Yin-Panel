import { readdir, readFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', 'src')
const visual = /\b(color|background(?:-color|-image)?|border(?:-color|-radius|-width)?|box-shadow|text-shadow|filter|backdrop-filter|opacity|font(?:-family|-size|-weight)?|padding(?:-[\w]+)?|margin(?:-[\w]+)?|gap|transition(?:-[\w]+)?|transform)\s*:\s*([^;{}]+)/gim
const componentFiles = new Set([
  'components/common/HoverButton/Button.vue',
  'components/common/ItemCard/index.vue',
  'components/common/ItemIcon/index.vue',
  'components/deskModule/SearchBox/index.vue',
  'components/deskModule/SystemMonitor/index.vue',
  'views/home/components/AppIcon/index.vue',
  'views/home/components/CommandCenter/index.vue',
  'views/home/components/WallpaperLayer.vue',
  'views/home/index.vue',
])

async function collect(directory) {
  const entries = await readdir(directory, { withFileTypes: true })
  const files = []
  for (const entry of entries) {
    const filename = path.join(directory, entry.name)
    if (entry.isDirectory()) files.push(...await collect(filename))
    else if (/\.(vue|css|less)$/.test(entry.name)) files.push(filename)
  }
  return files
}

const findings = []
for (const filename of await collect(root)) {
  const relative = path.relative(path.dirname(root), filename).replaceAll(path.sep, '/')
  if (!componentFiles.has(relative)) continue
  const source = await readFile(filename, 'utf8')
  const blocks = filename.endsWith('.vue')
    ? [...source.matchAll(/<style\b[^>]*>([\s\S]*?)<\/style>/gi)].map(match => ({ text: match[1], offset: match.index + match[0].indexOf(match[1]) }))
    : [{ text: source, offset: 0 }]
  for (const block of blocks) {
    for (const match of block.text.matchAll(visual)) {
      const value = match[2].trim()
      if (/^(?:var\(|inherit\b|initial\b|unset\b|none\b|transparent\b|currentColor\b|0(?:\s|$))/.test(value)) continue
      const line = source.slice(0, block.offset + match.index).split('\n').length
      const declaration = `${match[1]}: ${value}`.replace(/[\r\n\t ]+/g, ' ').trim()
      findings.push({ file: relative, line, declaration })
    }
  }
}

const byFile = new Map()
for (const finding of findings) {
  const list = byFile.get(finding.file) || []
  list.push(finding)
  byFile.set(finding.file, list)
}

console.log(`Theme token audit: ${findings.length} literal visual declarations across ${byFile.size} theme-controlled component files.`)
for (const [filename, fileFindings] of [...byFile].sort(([a], [b]) => a.localeCompare(b))) {
  console.log(`\n${filename} (${fileFindings.length})`)
  for (const finding of fileFindings.slice(0, 8))
    console.log(`  ${finding.line}: ${finding.declaration}`)
  if (fileFindings.length > 8) console.log(`  ... ${fileFindings.length - 8} more`)
}

if (findings.length) process.exitCode = 1
