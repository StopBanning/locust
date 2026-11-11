#!/usr/bin/env node
// Faces for 100 teammate ids, computed by Electron's own avatar.ts (seedAvatar, botFor), so the Rust rule
// (src/look/faces.rs) is held to it by a test. Writes parity/faces.json.
//
//   node scripts/face-goldens.mjs [--electron <checkout>]

import { mkdtemp, writeFile } from 'node:fs/promises'
import { createRequire } from 'node:module'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { execFileSync } from 'node:child_process'

const here = dirname(fileURLToPath(import.meta.url))
const args = process.argv.slice(2)
const ROOT = resolve(args.includes('--electron') ? args[args.indexOf('--electron') + 1] : process.env.LOCUST_ELECTRON_CHECKOUT ?? join(here, '..', '..', '..'))
const DESKTOP = join(ROOT, 'apps', 'desktop')
const require = createRequire(join(DESKTOP, 'package.json'))
const esbuild = createRequire(require.resolve('vite/package.json'))('esbuild')
const work = await mkdtemp(join(tmpdir(), 'locust-face-goldens-'))
await esbuild.build({ entryPoints: [join(DESKTOP, 'src/shared/avatar.ts')], bundle: true, format: 'esm', platform: 'node', outfile: join(work, 'avatar.mjs'), logLevel: 'warning' })
const { seedAvatar, botFor } = await import(pathToFileURL(join(work, 'avatar.mjs')).href)

// Ids of the shapes the store makes and a few it must survive: ascii, a UUID, digits, a name in another script, an emoji.
const ids = ['tm_wren', 'tm_atlas', 'tm_sable', 'tm_quill', 'codec', 'argon', 'scout', 'a', 'b', 'tm_a1', 'tm_a2']
for (let i = 0; ids.length < 92; i += 1) ids.push(`tm_${(i * 2654435761 >>> 0).toString(36)}`)
ids.push('00000000-0000-4000-8000-000000000000', '1234567890', 'tm_ÄÖÜ', 'tm_日本語', 'tm_🦗', 'tm_x'.repeat(16), 'Z', 'tm-dash_under')
const faces = ids.map((id) => {
  const a = seedAvatar(id)
  const bot = botFor(a)
  return { id, headwear: a.headwear, accessory: a.accessory, mouth: a.mouth, shape: bot.shape, face: bot.face }
})
const commit = execFileSync('git', ['-C', ROOT, 'rev-parse', '--short=8', 'HEAD'], { encoding: 'utf8' }).trim()
const out = join(here, '..', 'parity', 'faces.json')
await writeFile(out, JSON.stringify({ electronCommit: commit, generatedBy: 'scripts/face-goldens.mjs', faces }, null, 1) + '\n')
console.log(`${faces.length} faces from Electron ${commit} -> ${out}`)
