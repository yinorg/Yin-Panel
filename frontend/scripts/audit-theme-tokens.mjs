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
  'components/deskModule/SystemMonitor/AppIconSystemMonitor/index.vue',
  'components/deskModule/SystemMonitor/components/GenericMonitorCard/index.vue',
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
const missing = []
const visualUtility = /^(?:rounded(?:-.+)?|shadow(?:-.+)?|text-(?:xs|sm|base|lg|xl|white|black|gray-.+|zinc-.+|\[.+\])|bg-(?:white|black|gray-.+|zinc-.+|\[.+\])|font-(?:medium|semibold|bold|extrabold)|(?:p|px|py|pt|pb|pl|pr|m|mx|my|mt|mb|ml|mr|gap|gap-x|gap-y)-.+|transition(?:-.+)?|duration-.+)$/
const knownVisualExceptions = new Set(['styles/lib/github-markdown.less', 'styles/lib/highlight.less'])
for (const relative of componentFiles) {
  try { await readFile(path.join(root, relative), 'utf8') }
  catch { missing.push(relative) }
}
const files = await collect(root)
const componentSources = new Map()
for (const filename of files) {
	const relative = path.relative(root, filename).replaceAll(path.sep, '/')
	const themeConsumer = relative.startsWith('components/common/') || relative.startsWith('components/deskModule/') || relative.startsWith('views/home/')
	if (!themeConsumer && !/\.(css|less)$/.test(filename)) continue
	const source = await readFile(filename, 'utf8')
	componentSources.set(relative, source)
	if (knownVisualExceptions.has(relative)) continue
	const blocks = filename.endsWith('.vue')
		? [
		  ...[...source.matchAll(/<template\b[^>]*>([\s\S]*?)<\/template>/gi)].map(match => ({ text: match[1], offset: match.index + match[0].indexOf(match[1]), kind: 'template' })),
		  ...[...source.matchAll(/<style\b[^>]*>([\s\S]*?)<\/style>/gi)].map(match => ({ text: match[1], offset: match.index + match[0].indexOf(match[1]), kind: 'style' })),
		]
		: [{ text: relative === 'styles/global.less' ? source.replace(/:root\s*\{[\s\S]*?\n\}/, match => match.replace(/[^\n]/g, ' ')) : source, offset: 0, kind: 'style' }]
	for (const block of blocks) {
		const sourceText = block.kind === 'template'
			? [...block.text.matchAll(/\bclass\s*=\s*(["'])(.*?)\1|\b:style\s*=\s*(["'])(.*?)\3/gis)].map(match => ({ text: match[2] || match[4] || '', offset: match.index, kind: match[2] ? 'class' : 'style' }))
			: [{ text: block.text, offset: 0 }]
		for (const fragment of sourceText) {
			if (fragment.kind === 'class') {
				for (const utility of fragment.text.split(/\s+/).map(token => token.replace(/^['"]|['",]+$/g, '')).filter(token => visualUtility.test(token)))
					findings.push({ file: relative, line: source.slice(0, block.offset + fragment.offset).split('\n').length, declaration: `class: ${utility}`, kind: 'template' })
				continue
			}
			const text = fragment.kind === 'style'
				? fragment.text.replace(/([a-z])([A-Z])/g, '$1-$2').replace(/[{},]/g, ';')
				: fragment.text
			for (const match of text.matchAll(visual)) {
				const value = match[2].trim()
				if (/^(?:var\(|inherit\b|initial\b|unset\b|none\b|transparent\b|currentColor\b|0(?:\s|$))/.test(value) || /var\(--yin-/.test(value)) continue
				if (fragment.kind === 'style' && block.kind === 'template' && !/(?:\d+(?:\.\d+)?(?:px|rem|vh|vw|%|deg|ms)|#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(|(?:linear|radial)-gradient|color-mix|calc\()/i.test(value)) continue
				const line = source.slice(0, block.offset + fragment.offset + match.index).split('\n').length
				const declaration = `${match[1]}: ${value}`.replace(/[\r\n\t ]+/g, ' ').trim()
				findings.push({ file: relative, line, declaration, kind: fragment.kind || block.kind })
			}
		}
	}
}

const byFile = new Map()
for (const finding of findings) {
  const list = byFile.get(finding.file) || []
  list.push(finding)
  byFile.set(finding.file, list)
}

const declaredComponentTokens = new Set()
for (const source of componentSources.values()) {
	for (const match of source.matchAll(/--(yin-component-[a-z0-9-]+)\s*:/g)) declaredComponentTokens.add(match[1])
}
const allSource = [...componentSources.values(), await readFile(path.join(root, 'hooks/useTheme.ts'), 'utf8')].join('\n')
const consumerSource = allSource.replace(/--yin-component-[a-z0-9-]+\s*:/g, '')
const unconsumed = [...declaredComponentTokens].filter((token) => {
	const cssConsumer = new RegExp(`var\\(--${token}(?:[,)]|\\s)`).test(allSource)
	const runtimeConsumer = consumerSource.includes(token.replace(/^yin-/, ''))
	return !cssConsumer && !runtimeConsumer
})
console.log(`Theme token audit: scanned ${componentSources.size} home/common/deskModule and stylesheet files; found ${findings.length} literal visual declarations across ${byFile.size} files; ${unconsumed.length} component defaults have no var() consumer.`)
if (unconsumed.length) console.error(`Unconsumed component defaults: ${unconsumed.join(', ')}`)
if (missing.length) console.error(`Configured audit files missing: ${missing.join(', ')}`)
for (const [filename, fileFindings] of [...byFile].sort(([a], [b]) => a.localeCompare(b))) {
  console.log(`\n${filename} (${fileFindings.length})`)
  for (const finding of fileFindings.slice(0, 8))
		console.log(`  ${finding.line} (${finding.kind}): ${finding.declaration}`)
  if (fileFindings.length > 8) console.log(`  ... ${fileFindings.length - 8} more`)
}

if (missing.length || byFile.size === 0 || unconsumed.length) process.exitCode = 1
