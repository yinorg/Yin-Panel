import fs from 'node:fs'
import path from 'node:path'

const root = path.resolve(new URL('.', import.meta.url).pathname, '../src/locales')
const source = JSON.parse(fs.readFileSync(path.join(root, 'en-US.json'), 'utf8'))
const placeholders = value => [...String(value).matchAll(/\{[^{}]+\}/g)].map(match => match[0]).sort().join('|')

function keys(value, prefix = '') {
  return Object.entries(value).flatMap(([key, child]) => {
    const full = prefix ? `${prefix}.${key}` : key
    return child && typeof child === 'object' ? keys(child, full) : [full]
  })
}

const expected = new Set(keys(source))
let failed = false
for (const file of fs.readdirSync(root).filter(file => file.endsWith('.json') && file !== 'en-US.json')) {
  const locale = JSON.parse(fs.readFileSync(path.join(root, file), 'utf8'))
  const actual = new Set(keys(locale))
  const missing = [...expected].filter(key => !actual.has(key))
  const extra = [...actual].filter(key => !expected.has(key))
  if (missing.length || extra.length) {
    failed = true
    console.error(`${file}: missing=${missing.length}, extra=${extra.length}`)
    if (missing.length) console.error(`  missing: ${missing.slice(0, 10).join(', ')}`)
    if (extra.length) console.error(`  extra: ${extra.slice(0, 10).join(', ')}`)
  }
  for (const key of expected) {
    const sourceValue = key.split('.').reduce((value, part) => value?.[part], source)
    const localeValue = key.split('.').reduce((value, part) => value?.[part], locale)
    if (placeholders(sourceValue) !== placeholders(localeValue)) {
      failed = true
      console.error(`${file}: placeholder mismatch at ${key}`)
    }
  }
}
if (failed) process.exit(1)
console.log('All locale keys match en-US.json')
