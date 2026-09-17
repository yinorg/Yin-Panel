#!/usr/bin/env node

import fs from 'node:fs/promises'
import { existsSync } from 'node:fs'
import os from 'node:os'
import path from 'node:path'

const sourceFile = path.resolve('tmp/51kim-bookmarks.html')
const retryFile = path.resolve('tmp/51kim-icon-retry.html')
const extensionPath = process.env.YIN_PANEL_EXTENSION || '/home/hsy/project/yin-panel-extension'
const panelUrl = process.env.YIN_PANEL_URL || 'http://127.0.0.1:3002/'
const proxy = process.env.ICON_PROXY || 'http://192.168.31.10:7890'
const timeout = Number(process.env.ICON_TIMEOUT || 30000)
const batchSize = Number(process.env.ICON_BATCH_SIZE || 20)
const e2eRoot = process.env.YIN_PANEL_E2E || '/home/hsy/project/yin-panel-e2e'

const decode = value => value.replaceAll('&quot;', '"').replaceAll('&lt;', '<').replaceAll('&gt;', '>').replaceAll('&amp;', '&')
const encode = value => value.replaceAll('&', '&amp;').replaceAll('"', '&quot;').replaceAll('<', '&lt;').replaceAll('>', '&gt;')

function parseBookmarks(html) {
  const records = []
  const groups = /<DT><H3[^>]*>([\s\S]*?)<\/H3>[\s\S]*?<DL><p>([\s\S]*?)<\/DL>/gi
  for (const group of html.matchAll(groups)) {
    for (const item of group[2].matchAll(/<DT><A[^>]*HREF="([^"]+)"[^>]*>([\s\S]*?)<\/A>/gi))
      records.push({ group: decode(group[1]).trim() || '其他', title: decode(item[2]).trim(), url: decode(item[1]) })
  }
  return [...new Map(records.map(item => [item.url, item])).values()]
}

function render(records) {
  const lines = ['<!DOCTYPE NETSCAPE-Bookmark-file-1>', '<META HTTP-EQUIV="Content-Type" CONTENT="text/html; charset=UTF-8">', '<TITLE>51kim icon retry list</TITLE>', '<H1>51kim icon retry list</H1>', '<DL><p>']
  for (const group of [...new Set(records.map(item => item.group))]) {
    lines.push(`    <DT><H3 ADD_DATE="0">${encode(group)}</H3>`, '    <DL><p>')
    for (const item of records.filter(item => item.group === group)) lines.push(`        <DT><A HREF="${encode(item.url)}">${encode(item.title)}</A>`)
    lines.push('    </DL><p>')
  }
  lines.push('</DL><p>', '')
  return lines.join('\n')
}

async function atomicWrite(records) {
  const temporary = `${retryFile}.${process.pid}.tmp`
  await fs.writeFile(temporary, render(records), 'utf8')
  await fs.rename(temporary, retryFile)
}

async function initialize() {
  const records = parseBookmarks(await fs.readFile(sourceFile, 'utf8'))
  if (!records.length) throw new Error(`No bookmarks found in ${sourceFile}`)
  await atomicWrite(records)
  console.log(`Initialized ${records.length} unique bookmarks`)
}

async function loadPlaywright() {
  return import(path.join(e2eRoot, 'node_modules/playwright/index.mjs'))
}

async function requestIcons(page, urls) {
  return page.evaluate(({ urls, timeout }) => new Promise((resolve, reject) => {
    const requestId = `retry-${Date.now()}-${Math.random()}`
    const timer = setTimeout(() => { window.removeEventListener('message', listener); reject(new Error('extension timeout')) }, timeout)
    function listener(event) {
      const data = event.data
      if (event.source !== window || data?.source !== 'yin-panel-extension' || data.requestId !== requestId) return
      clearTimeout(timer); window.removeEventListener('message', listener); resolve(Array.isArray(data.items) ? data.items : [])
    }
    window.addEventListener('message', listener)
    window.postMessage({ source: 'yin-panel', type: 'fetch-icons', requestId, urls }, '*')
  }), { urls, timeout })
}

async function run() {
  if (process.argv.includes('--initialize')) return initialize()
  if (!existsSync(retryFile)) throw new Error(`Missing ${retryFile}; run --initialize first`)
  const { chromium } = await loadPlaywright()
  const records = parseBookmarks(await fs.readFile(retryFile, 'utf8'))
  let remaining = records
  let site = 0; let publicCount = 0; let generated = 0; let failed = 0
  const userDataDir = await fs.mkdtemp(path.join(os.tmpdir(), 'yin-panel-icon-retry-'))
  let context
  try {
    context = await chromium.launchPersistentContext(userDataDir, {
      headless: false,
      args: [`--disable-extensions-except=${extensionPath}`, `--load-extension=${extensionPath}`, '--no-proxy-server'],
    })
    const page = await context.newPage()
    const deadline = Date.now() + 15000
    let worker
    while (!worker && Date.now() < deadline) {
      worker = context.serviceWorkers().find(item => item.url().startsWith('chrome-extension://'))
      if (!worker) await page.waitForTimeout(100)
    }
    if (!worker) throw new Error('Chrome extension service worker was not loaded')
    const proxyValue = new URL(proxy)
    await worker.evaluate(({ scheme, host, port }) => chrome.storage.sync.set({ iconProxy: { scheme, host, port } }), {
      scheme: proxyValue.protocol.slice(0, -1), host: proxyValue.hostname, port: Number(proxyValue.port),
    })
    await page.goto(panelUrl, { waitUntil: 'domcontentloaded', timeout })
    const pending = [...remaining]
    for (let offset = 0; offset < pending.length; offset += batchSize) {
      const batch = pending.slice(offset, offset + batchSize)
      let items
      try { items = await requestIcons(page, batch.map(item => item.url)) } catch (error) {
        failed += batch.length
        console.log(`[failed] retained ${batch.length} URLs for the next run`)
        console.log(`[failed] batch: ${error.message}`)
        continue
      }
      const successful = new Set()
      for (const item of items) {
        if (item.source === 'site' || item.source === 'public') {
          successful.add(item.url)
          if (item.source === 'site') site++; else publicCount++
          console.log(`[success] ${item.source} ${item.url}`)
        } else {
          generated++
          console.log(`[keep] ${item.url} - ${item.reason || item.error || 'generated icon'}`)
        }
      }
      remaining = remaining.filter(item => !successful.has(item.url))
      await atomicWrite(remaining)
    }
  } finally {
    await context?.close()
    await fs.rm(userDataDir, { recursive: true, force: true })
  }
  console.log(JSON.stringify({ total: records.length, remaining: remaining.length, removed: records.length - remaining.length, site, public: publicCount, generated, failed }, null, 2))
}

run().catch(error => { console.error(error.stack || error.message); process.exitCode = 1 })
