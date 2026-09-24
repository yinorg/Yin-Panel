import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { createServer } from 'node:http'
import { mkdtemp, readFile, rm, symlink, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { inflateRawSync } from 'node:zlib'
// eslint-disable-next-line test/no-import-node-test
import test from 'node:test'
import { buildTheme, createCorePreviewURL, packTheme, scaffoldTheme, testTheme, validateTheme, watchThemePreview } from '../bin/yin-theme.mjs'

test('scaffolds a sandbox Theme API v1 package that validates', async () => {
  const temp = await mkdtemp(path.join(os.tmpdir(), 'yin-theme-create-'))
  try {
    const result = await scaffoldTheme({ name: 'Editorial Desk', author: 'Example Author', directory: path.join(temp, 'theme') })
    assert.equal(result.id, 'community.editorial-desk')
    const validated = await validateTheme(result.directory)
    assert.equal(validated.id, result.id)
    assert.equal(validated.name, 'Editorial Desk')
    assert.equal(validated.resources, 2)
    assert.equal(validated.manifest.core, '>=0.4.0')
  }
  finally { await rm(temp, { recursive: true, force: true }) }
})

test('refuses to overwrite an existing theme directory', async () => {
  const temp = await mkdtemp(path.join(os.tmpdir(), 'yin-theme-create-existing-'))
  try {
    await assert.rejects(scaffoldTheme({ name: 'Example', author: 'Author', directory: temp }), /Target already exists/)
  }
  finally { await rm(temp, { recursive: true, force: true }) }
})

test('validation rejects tampered resources', async () => {
  const temp = await mkdtemp(path.join(os.tmpdir(), 'yin-theme-create-tampered-'))
  try {
    const { directory } = await scaffoldTheme({ name: 'Example Theme', author: 'Author', directory: path.join(temp, 'theme') })
    await writeFile(path.join(directory, 'styles/home.css'), '/* modified */')
    await assert.rejects(validateTheme(directory), /resource digest does not match/)
  }
  finally { await rm(temp, { recursive: true, force: true }) }
})

test('validation rejects symbolic links in package resources', async () => {
  const temp = await mkdtemp(path.join(os.tmpdir(), 'yin-theme-create-symlink-'))
  try {
    const { directory } = await scaffoldTheme({ name: 'Example Theme', author: 'Author', directory: path.join(temp, 'theme') })
    const stylesheet = path.join(directory, 'styles/home.css')
    await rm(stylesheet)
    await symlink(path.join(directory, 'README.md'), stylesheet)
    await assert.rejects(validateTheme(directory), /symbolic links are not allowed/)
  }
  finally { await rm(temp, { recursive: true, force: true }) }
})

test('packs only declared package files into a valid deterministic ZIP archive', async () => {
  const temp = await mkdtemp(path.join(os.tmpdir(), 'yin-theme-create-pack-'))
  try {
    const { directory, id } = await scaffoldTheme({ name: 'Example Theme', author: 'Author', directory: path.join(temp, 'theme') })
    const result = await packTheme(directory)
    const archive = await readFile(result.output)
    assert.equal(archive.readUInt32LE(0), 0x04034b50)
    const names = []
    let offset = 0
    while (archive.readUInt32LE(offset) === 0x04034b50) {
      const compressedSize = archive.readUInt32LE(offset + 18)
      const nameLength = archive.readUInt16LE(offset + 26)
      const extraLength = archive.readUInt16LE(offset + 28)
      const name = archive.toString('utf8', offset + 30, offset + 30 + nameLength)
      names.push(name)
      offset += 30 + nameLength + extraLength + compressedSize
    }
    assert.deepEqual(names, [
      'manifest.json', 'styles/home.css', 'tokens/dark.tokens.json',
      'tokens/light.tokens.json', 'views/home.mjs',
    ])
    const centralOffset = offset
    assert.equal(archive.readUInt32LE(centralOffset), 0x02014b50)
    const firstNameLength = archive.readUInt16LE(centralOffset + 28)
    assert.equal(archive.toString('utf8', centralOffset + 46, centralOffset + 46 + firstNameLength), 'manifest.json')
    const compressedOffset = 30 + 'manifest.json'.length
    const manifestSize = archive.readUInt32LE(centralOffset + 24)
    const compressedSize = archive.readUInt32LE(centralOffset + 20)
    const manifest = JSON.parse(inflateRawSync(archive.subarray(compressedOffset, compressedOffset + compressedSize)).toString())
    assert.equal(manifest.id, id)
    assert.equal(manifestSize, Buffer.byteLength(await readFile(path.join(directory, 'manifest.json'))))
    assert.equal(result.files, names.length)
    const repeatedArchive = await packTheme(directory, path.join(temp, 'repeat.zip'))
    assert.deepEqual(await readFile(repeatedArchive.output), archive)
    await assert.rejects(packTheme(directory), /Output already exists/)
  }
  finally { await rm(temp, { recursive: true, force: true }) }
})

test('validation rejects files outside the declared package closure', async () => {
  const temp = await mkdtemp(path.join(os.tmpdir(), 'yin-theme-create-undeclared-'))
  try {
    const { directory } = await scaffoldTheme({ name: 'Example Theme', author: 'Author', directory: path.join(temp, 'theme') })
    await writeFile(path.join(directory, 'payload.mjs'), 'globalThis.injected = true')
    await assert.rejects(validateTheme(directory), /undeclared package file: payload.mjs/)
  }
  finally { await rm(temp, { recursive: true, force: true }) }
})

test('build bundles local JavaScript imports, refreshes resource digests and keeps source intact', async () => {
  const temp = await mkdtemp(path.join(os.tmpdir(), 'yin-theme-create-build-'))
  try {
    const { directory } = await scaffoldTheme({ name: 'Build Theme', author: 'Author', directory: path.join(temp, 'source') })
    const entrypoint = 'views/home.mjs'
    const helper = 'views/helper.mjs'
    const source = `import { themeName } from './helper.mjs'\nexport default { themeName }\n`
    const helperSource = 'export const themeName = "Build Theme"\n'
    await writeFile(path.join(directory, entrypoint), source)
    await writeFile(path.join(directory, helper), helperSource)
    const manifestPath = path.join(directory, 'manifest.json')
    const manifest = JSON.parse(await readFile(manifestPath, 'utf8'))
    manifest.resources.push({
      path: helper,
      sha256: createHash('sha256').update(helperSource).digest('hex'),
      mediaType: 'text/javascript',
    })
    manifest.resources.find(resource => resource.path === entrypoint).sha256 = createHash('sha256').update(source).digest('hex')
    await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`)

    const result = await buildTheme(directory, path.join(temp, 'built'))
    const compiled = await readFile(path.join(result.directory, entrypoint), 'utf8')
    const builtManifest = JSON.parse(await readFile(path.join(result.directory, 'manifest.json'), 'utf8'))
    const digest = createHash('sha256').update(compiled).digest('hex')
    assert.match(compiled, /Build Theme/)
    assert.equal(builtManifest.resources.find(resource => resource.path === entrypoint).sha256, digest)
    assert.equal(await readFile(path.join(directory, entrypoint), 'utf8'), source)
    assert.equal((await validateTheme(result.directory)).id, manifest.id)
    await assert.rejects(buildTheme(directory, result.directory), /already exists/)
    const checked = await testTheme(directory)
    assert.equal(checked.id, manifest.id)
  }
  finally { await rm(temp, { recursive: true, force: true }) }
})

test('preview uploads a built package to Core and returns its sandbox preview URL', async () => {
  const temp = await mkdtemp(path.join(os.tmpdir(), 'yin-theme-preview-cli-'))
  const server = createServer(async (request, response) => {
    const chunks = []
    for await (const chunk of request) chunks.push(chunk)
    const body = Buffer.concat(chunks)
    assert.equal(request.method, 'POST')
    assert.equal(request.url, '/api/theme/v2/admin/preview')
    assert.equal(request.headers.authorization, 'Bearer test-admin-jwt')
    assert.match(request.headers['content-type'], /^multipart\/form-data; boundary=/)
    assert.ok(body.includes(Buffer.from([0x50, 0x4b, 0x03, 0x04])), 'request includes a ZIP package')
    response.writeHead(200, { 'content-type': 'application/json' })
    response.end(JSON.stringify({ code: 0, data: { token: 'a'.repeat(48) } }))
  })
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
  try {
    const { directory } = await scaffoldTheme({ name: 'Preview Theme', author: 'Author', directory: path.join(temp, 'theme') })
    const address = server.address()
    const previewURL = await createCorePreviewURL(directory, {
      coreURL: `http://127.0.0.1:${address.port}`,
      authToken: 'test-admin-jwt',
      mode: 'dark',
    })
    assert.equal(previewURL, `http://127.0.0.1:${address.port}/?themePreview=${'a'.repeat(48)}&themePreviewMode=dark`)
  }
  finally {
    await new Promise((resolve, reject) => server.close(error => error ? reject(error) : resolve()))
    await rm(temp, { recursive: true, force: true })
  }
})

test('preview requires an admin token and refuses remote plaintext Core URLs', async () => {
  await assert.rejects(createCorePreviewURL('.', { authToken: '' }), /YIN_THEME_AUTH_TOKEN/)
  await assert.rejects(createCorePreviewURL('.', { coreURL: 'http://core.example.test', authToken: 'secret' }), /HTTPS/)
  await assert.rejects(createCorePreviewURL('.', { coreURL: 'https://core.example.test/path', authToken: 'secret' }), /without credentials, path/)
})

test('dev watcher creates a fresh preview after token edits and stops on abort', async () => {
  const temp = await mkdtemp(path.join(os.tmpdir(), 'yin-theme-dev-preview-'))
  let previewCount = 0
  const server = createServer(async (request, response) => {
    let bodyBytes = 0
    for await (const chunk of request) {
      bodyBytes += chunk.length
    }
    assert.ok(bodyBytes > 0, 'preview request includes the package upload')
    previewCount++
    response.writeHead(200, { 'content-type': 'application/json' })
    response.end(JSON.stringify({ code: 0, data: { token: String(previewCount).padStart(48, '0') } }))
  })
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
  const abort = new AbortController()
  const messages = []
  try {
    const { directory } = await scaffoldTheme({ name: 'Watched Theme', author: 'Author', directory: path.join(temp, 'theme') })
    const address = server.address()
    const running = watchThemePreview(directory, {
      coreURL: `http://127.0.0.1:${address.port}`,
      authToken: 'test-admin-jwt',
    }, message => messages.push(message), abort.signal)

    await waitFor(() => messages.filter(message => message.startsWith('http://')).length === 1)
    const tokenPath = path.join(directory, 'tokens/light.tokens.json')
    const tokenDocument = JSON.parse(await readFile(tokenPath, 'utf8'))
    tokenDocument.$description = 'updated while the dev watcher is running'
    await writeFile(tokenPath, `${JSON.stringify(tokenDocument, null, 2)}\n`)
    await waitFor(() => messages.filter(message => message.startsWith('http://')).length === 2)
    abort.abort()
    await running

    const urls = messages.filter(message => message.startsWith('http://'))
    assert.notEqual(urls[0], urls[1])
    assert.ok(messages.includes('Watching Theme files; press Ctrl+C to stop.'))
    assert.equal(messages.some(message => message.startsWith('Preview failed:')), false)
  }
  finally {
    abort.abort()
    await new Promise((resolve, reject) => server.close(error => error ? reject(error) : resolve()))
    await rm(temp, { recursive: true, force: true })
  }
})

async function waitFor(predicate) {
  const deadline = Date.now() + 10_000
  while (!predicate()) {
    if (Date.now() >= deadline) throw new Error('Timed out waiting for Theme dev preview')
    await new Promise(resolve => setTimeout(resolve, 20))
  }
}
