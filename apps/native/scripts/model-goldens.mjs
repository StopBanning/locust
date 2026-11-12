#!/usr/bin/env node
// What Electron's own model catalog makes of each agent's real model list (B4): parity/models/*.txt are the outputs
// of `claude --help`, `cursor-agent --list-models`, `opencode models --verbose`, `agy models` and Codex's app-server
// `model/list`, captured on the Mac mini on 12 Nov 2026. This runs runtime-adapters' parsers and model-catalog.ts's
// builders over them and writes parity/models.json for core/src/models_tests.rs.
//
//   node scripts/model-goldens.mjs [--electron <checkout>]
//
// The checkout must have apps/desktop's dependencies installed (it uses that esbuild). Nothing is sent anywhere.

import { mkdtemp, readFile, writeFile } from 'node:fs/promises'
import { createRequire } from 'node:module'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { execFileSync } from 'node:child_process'
import { fileURLToPath, pathToFileURL } from 'node:url'

const here = dirname(fileURLToPath(import.meta.url))
const args = process.argv.slice(2)
const option = (name, fallback) => (args.includes(name) ? args[args.indexOf(name) + 1] : fallback)
const ROOT = resolve(option('--electron', process.env.LOCUST_ELECTRON_CHECKOUT ?? join(here, '..', '..', '..')))
const DESKTOP = join(ROOT, 'apps', 'desktop')
const require = createRequire(join(DESKTOP, 'package.json'))
const esbuild = createRequire(require.resolve('vite/package.json'))('esbuild')
const at = (p) => JSON.stringify(join(ROOT, p).split('\\').join('/'))

const work = await mkdtemp(join(tmpdir(), 'locust-model-goldens-'))
await writeFile(join(work, 'entry.ts'), `
export { parseClaudeModelHints, parseCursorModelList, parseOpenCodeModelList, parseAgyModelList } from ${at('packages/runtime-adapters/src/commands.ts')}
export { parseModels, claudeModelsFrom, cursorModelsFrom, opencodeModelsFrom, copilotModelsFrom, museModelsFrom, antigravityModelsFrom, withAccountDefaults } from ${at('apps/desktop/src/main/model-catalog.ts')}
`, 'utf8')
await esbuild.build({
  entryPoints: [join(work, 'entry.ts')], bundle: true, format: 'esm', platform: 'node', target: 'node20',
  outfile: join(work, 'entry.mjs'), nodePaths: [join(DESKTOP, 'node_modules'), join(ROOT, 'node_modules')],
  absWorkingDir: ROOT, logLevel: 'warning', external: ['electron']
})
const m = await import(pathToFileURL(join(work, 'entry.mjs')).href)

const dir = join(here, '..', 'parity', 'models')
const text = (name) => readFile(join(dir, name), 'utf8')
const captured = {
  claude: await text('claude.txt'),
  cursor: await text('cursor.txt'),
  opencode: await text('opencode.txt'),
  agy: await text('agy.txt'),
  codex: JSON.parse(await text('codex-model-list.json'))
}
const hints = {
  claude: m.parseClaudeModelHints(captured.claude) ?? null,
  cursor: m.parseCursorModelList(captured.cursor) ?? null,
  opencode: m.parseOpenCodeModelList(captured.opencode) ?? null,
  antigravity: m.parseAgyModelList(captured.agy) ?? null
}
const ready = (id, extra = {}) => ({ id, readiness: 'ready', availability: 'available', version: { version: id === 'opencode' ? '1.19.5' : '1.0.0' }, ...extra })
// What a sweep finds here: each agent ready with what its list printed. Muse is on a second machine only.
const machine = (withMuse) => [
  ready('claude', { modelHints: hints.claude ?? undefined }),
  ready('cursor', { modelHints: hints.cursor ?? undefined }),
  ready('opencode', { modelHints: hints.opencode ?? undefined }),
  ready('copilot'),
  ready('antigravity', { modelHints: hints.antigravity ?? undefined, executable: { commandName: 'agy' } }),
  ...(withMuse ? [ready('muse')] : [])
]
const catalog = (runtimes, codex) => m.withAccountDefaults([
  ...m.parseModels(codex),
  ...m.claudeModelsFrom(runtimes, []),
  ...m.cursorModelsFrom(runtimes),
  ...m.opencodeModelsFrom(runtimes),
  ...m.copilotModelsFrom(runtimes),
  ...m.antigravityModelsFrom(runtimes),
  ...m.museModelsFrom(runtimes)
])
const commit = execFileSync('git', ['-C', ROOT, 'rev-parse', '--short', 'HEAD'], { encoding: 'utf8' }).trim()
const out = {
  electronCommit: commit,
  generatedBy: 'scripts/model-goldens.mjs',
  hints,
  machines: [
    { name: 'this Mac', muse: false, codex: true, models: catalog(machine(false), captured.codex) },
    { name: 'with Muse, Codex not ready', muse: true, codex: false, models: catalog(machine(true), {}) }
  ]
}
await writeFile(join(here, '..', 'parity', 'models.json'), JSON.stringify(out, null, 1) + '\n', 'utf8')
console.log(`wrote ${out.machines.map((x) => `${x.models.length} models (${x.name})`).join(', ')} from Electron ${commit}`)
