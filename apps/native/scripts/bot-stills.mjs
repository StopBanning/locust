#!/usr/bin/env node
// Every bot a teammate's seeded look can be, drawn still by Electron's own TeammateBot (C4: "bots start as still
// pictures exported from Electron's own drawing; moving bots come in Phase D").
//
//   node scripts/bot-stills.mjs [--electron <checkout>] [--size 96]
//
// The checkout must have apps/desktop's dependencies installed (it uses that Electron and esbuild). Writes
// assets/bots/<shape>-<face>-<hue>.png for the twenty derived shapes, both faces and the eleven hues, with terminal
// faces on and plastic shading (the app's defaults, botLook.ts), each on a transparent ground; then src/look/bots.rs,
// which compiles them in. Nothing is sent anywhere.

import { mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises'
import { createRequire } from 'node:module'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { spawn } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const here = dirname(fileURLToPath(import.meta.url))
const args = process.argv.slice(2)
const option = (name, fallback) => (args.includes(name) ? args[args.indexOf(name) + 1] : fallback)
const ROOT = resolve(option('--electron', process.env.LOCUST_ELECTRON_CHECKOUT ?? join(here, '..', '..', '..')))
const SIZE = Number(option('--size', '96'))
const DESKTOP = join(ROOT, 'apps', 'desktop')
const OUT = join(here, '..', 'assets', 'bots')
const RUST = join(here, '..', 'src', 'look', 'bots.rs')
const require = createRequire(join(DESKTOP, 'package.json'))
const esbuild = createRequire(require.resolve('vite/package.json'))('esbuild')
const electron = require('electron')
const src = (path) => JSON.stringify(join(DESKTOP, 'src/renderer/src', path).split('\\').join('/'))
const shared = (path) => JSON.stringify(join(DESKTOP, 'src/shared', path).split('\\').join('/'))

// The twenty shapes a look is derived from (avatar.ts DERIVED_SHAPES) and the eleven hues (teammate-store HUES).
const avatarSource = await readFile(join(DESKTOP, 'src/shared/avatar.ts'), 'utf8')
const shapes = [...avatarSource.slice(avatarSource.indexOf('export const BOT_SHAPES'), avatarSource.indexOf('] as const')).matchAll(/'([a-z]+)'/g)].map((m) => m[1]).slice(0, 20)
const tokens = await readFile(join(DESKTOP, 'src/renderer/src/tokens.css'), 'utf8')
const hues = [...new Set([...tokens.matchAll(/--lc-hue-([a-z]+):/g)].map((m) => m[1]))]
const faces = ['eyes', 'mouth']
if (shapes.length !== 20 || hues.length !== 11) throw new Error(`expected 20 shapes and 11 hues, found ${shapes.length} and ${hues.length}`)

const CELL = SIZE + 8
const cells = []
for (const hue of hues) for (const face of faces) for (const shape of shapes) cells.push({ shape, face, hue })
const COLS = shapes.length
const ROWS = cells.length / COLS

const ENTRY = `
import ${src('tokens.css')}
import ${src('shell.css')}
import { createElement as h } from 'react'
import { createRoot } from 'react-dom/client'
import { TeammateBot } from ${src('components/TeammateBot.tsx')}
const CELLS = ${JSON.stringify(cells)}
function Page() {
  return h('div', { style: { display: 'grid', gridTemplateColumns: 'repeat(${COLS}, ${CELL}px)', gridAutoRows: '${CELL}px', background: 'transparent' } },
    CELLS.map((c, i) => h('div', { key: i, style: { width: ${CELL}, height: ${CELL}, display: 'flex', alignItems: 'center', justifyContent: 'center' } },
      h(TeammateBot, { hue: c.hue, avatar: { headwear: 0, accessory: 0, mouth: 0, bot: { shape: c.shape, face: c.face } }, size: ${SIZE}, activity: 'idle', presence: 'none', motion: 'subtle' }))))
}
createRoot(document.getElementById('root')).render(h(Page))
`
void shared

await mkdir(OUT, { recursive: true })
const work = await mkdtemp(join(tmpdir(), 'locust-bot-stills-'))
await writeFile(join(work, 'entry.jsx'), ENTRY, 'utf8')
await esbuild.build({
  entryPoints: [join(work, 'entry.jsx')],
  bundle: true,
  format: 'iife',
  platform: 'browser',
  target: 'es2022',
  jsx: 'automatic',
  outfile: join(work, 'page.js'),
  define: { 'process.env.NODE_ENV': '"production"', 'import.meta.env': '{}' },
  loader: { '.woff2': 'file', '.woff': 'file', '.ttf': 'file', '.png': 'file', '.svg': 'file', '.webp': 'file', '.jpg': 'file', '.jpeg': 'file', '.gif': 'file', '.mp3': 'file', '.wav': 'file' },
  nodePaths: [join(DESKTOP, 'node_modules')],
  absWorkingDir: DESKTOP,
  logLevel: 'warning'
})
await writeFile(join(work, 'page.html'), '<!doctype html><html class="lc-theme-dark"><head><link rel="stylesheet" href="page.css"></head><body style="background:transparent;margin:0"><div id="root"></div><script src="page.js"></script></body></html>', 'utf8')
await writeFile(join(work, 'main.cjs'), `
const { app, BrowserWindow } = require('electron')
const { writeFileSync } = require('node:fs')
const { join } = require('node:path')
app.commandLine.appendSwitch('force-device-scale-factor', '1')
app.on('window-all-closed', () => {})
setTimeout(() => { process.stderr.write('bot-stills: timed out\\n'); app.exit(2) }, 120000)
const cells = ${JSON.stringify(cells)}
app.whenReady().then(async () => {
  const win = new BrowserWindow({ show: false, transparent: true, backgroundColor: '#00000000', width: ${COLS * CELL}, height: ${ROWS * CELL}, useContentSize: true, webPreferences: { backgroundThrottling: false } })
  await win.loadFile(${JSON.stringify(join(work, 'page.html'))})
  await new Promise((r) => setTimeout(r, 5000))
  const sheet = await win.webContents.capturePage()
  const scale = sheet.getSize().width / ${COLS * CELL}
  cells.forEach((c, i) => {
    const x = (i % ${COLS}) * ${CELL}, y = Math.floor(i / ${COLS}) * ${CELL}
    const cell = sheet.crop({ x: Math.round(x * scale), y: Math.round(y * scale), width: Math.round(${CELL} * scale), height: Math.round(${CELL} * scale) })
    const sized = scale === 1 ? cell : cell.resize({ width: ${CELL}, height: ${CELL}, quality: 'best' })
    writeFileSync(join(${JSON.stringify(OUT)}, c.shape + '-' + c.face + '-' + c.hue + '.png'), sized.toPNG())
  })
  process.stdout.write('wrote ' + cells.length + ' stills, sheet scale ' + scale + '\\n')
  app.quit()
}).catch((error) => { process.stderr.write('bot-stills: ' + String(error && error.stack || error) + '\\n'); app.exit(1) })
`, 'utf8')
const code = await new Promise((done) => {
  const child = spawn(electron, [join(work, 'main.cjs')], { windowsHide: true, stdio: ['ignore', 'inherit', 'inherit'] })
  child.on('close', done)
})
if (code !== 0) process.exit(code ?? 1)

const rust = [
  "//! Generated by scripts/bot-stills.mjs from Electron's own TeammateBot. Do not edit: run the script again.",
  '#![allow(dead_code)]',
  '',
  `/// The twenty shapes a seeded look is derived from (avatar.ts DERIVED_SHAPES), in order.`,
  `pub const SHAPES: [&str; 20] = [${shapes.map((s) => `"${s}"`).join(', ')}];`,
  `/// The eleven hues a teammate can wear, in tokens.css's order.`,
  `pub const HUES: [&str; 11] = [${hues.map((s) => `"${s}"`).join(', ')}];`,
  `/// Each still's edge in pixels: a ${SIZE}-pixel bot in a ${CELL}-pixel cell.`,
  `pub const CELL: u32 = ${CELL};`,
  '',
  '/// The still for a bot, as PNG bytes; None for a shape, face or hue it was not drawn in.',
  'pub fn still(shape: &str, face: &str, hue: &str) -> Option<&\'static [u8]> {',
  '    Some(match (shape, face, hue) {',
  ...cells.map((c) => `        ("${c.shape}", "${c.face}", "${c.hue}") => include_bytes!("../../assets/bots/${c.shape}-${c.face}-${c.hue}.png"),`),
  '        _ => return None,',
  '    })',
  '}',
  ''
].join('\n')
await writeFile(RUST, rust, 'utf8')
console.log(`wrote ${RUST}`)
