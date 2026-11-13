#!/usr/bin/env node
// What Electron's own AI agents screen says about each agent in each state it can be found in: routeRowStatus (the
// tag, whether it can be picked, the sentence), installCommand, installSentence, signInCommand and integrationOf, run
// over a grid of made-up discovery results. Writes parity/agents.json for core/src/agents_tests.rs.
//
//   node scripts/agents-goldens.mjs [--electron <checkout>]
//
// The checkout must have apps/desktop's dependencies installed (it uses that esbuild). Nothing is sent anywhere, and
// no agent on this computer is asked anything: every state is made up.

import { mkdtemp, writeFile } from 'node:fs/promises'
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

const work = await mkdtemp(join(tmpdir(), 'locust-agents-goldens-'))
await writeFile(join(work, 'entry.ts'), `
export { routeRowStatus, SIGNED_IN_DETAIL, NO_ACCOUNT_DETAIL } from ${at('apps/desktop/src/renderer/src/status.ts')}
export { installCommand, installSentence, signInCommand } from ${at('apps/desktop/src/shared/runtime-install.ts')}
export { integrationOf } from ${at('apps/desktop/src/shared/runtime-integration.ts')}
`, 'utf8')
await esbuild.build({
  entryPoints: [join(work, 'entry.ts')], bundle: true, format: 'esm', platform: 'node', target: 'node20',
  outfile: join(work, 'entry.mjs'), nodePaths: [join(DESKTOP, 'node_modules'), join(ROOT, 'node_modules')],
  absWorkingDir: ROOT, logLevel: 'warning', external: ['electron']
})
const m = await import(pathToFileURL(join(work, 'entry.mjs')).href)

const AGENTS = [
  ['codex', 'Codex CLI'], ['claude', 'Claude Code'], ['cursor', 'Cursor Agent'], ['copilot', 'Copilot CLI'],
  ['antigravity', 'Antigravity'], ['muse', 'Muse Code'], ['opencode', 'OpenCode'], ['kilo', 'Kilo'], ['gemini', 'Gemini CLI']
]
// Each state a discovery can leave an agent in, as PublicRuntimeStatus says it.
const STATES = {
  'not-installed': { installed: false, version: null, auth: 'unknown', ready: false, status: 'missing' },
  'signed-out': { installed: true, version: '1.0.0', auth: 'unauthenticated', ready: false, status: 'auth-required' },
  'unsupported': { installed: true, version: '2.0.0', auth: 'unknown', ready: false, status: 'unsupported', unsupportedNote: 'Version 2 is not supported yet. Keep version 1 for now.' },
  'unsupported-no-note': { installed: true, version: '2.0.0', auth: 'unknown', ready: false, status: 'unsupported' },
  'checking': { installed: true, version: '1.0.0', auth: 'unknown', ready: false, status: 'unknown', checking: true },
  'offline': { installed: true, version: '1.0.0', auth: 'unknown', ready: false, status: 'offline' },
  'probe-failed': { installed: true, version: null, auth: 'unknown', ready: false, status: 'probe-failed' },
  'not-ready': { installed: true, version: '1.0.0', auth: 'unknown', ready: false, status: 'unknown' },
  'ready-signed-in': { installed: true, version: '1.0.0', auth: 'authenticated', ready: true, status: 'ready' },
  'ready-no-account': { installed: true, version: '1.0.0', auth: 'not-applicable', ready: true, status: 'ready' },
  'ready-through-cli': { installed: true, version: '1.0.0', auth: 'authenticated', ready: true, status: 'ready', throughCli: true }
}
const LIMIT = 'You have reached your usage limit. It resets at 4pm.'

const rows = []
for (const [id, displayName] of AGENTS) {
  const integration = m.integrationOf(id)
  for (const [state, fields] of Object.entries(STATES)) {
    for (const active of [false, true]) {
      for (const limited of [undefined, LIMIT]) {
        const runtime = { id, displayName, ...fields }
        rows.push({ id, displayName, state, active, limited: limited ?? null, integration, status: m.routeRowStatus(runtime, integration, active, limited) })
      }
    }
  }
}
const install = Object.fromEntries(AGENTS.map(([id, displayName]) => [id, {
  integration: m.integrationOf(id),
  command: m.installCommand(id) ?? null,
  sentence: m.installSentence(id, displayName),
  signIn: m.signInCommand(id) ?? null,
  signInAgain: m.signInCommand(id, true) ?? null
}]))
const commit = execFileSync('git', ['-C', ROOT, 'rev-parse', '--short', 'HEAD'], { encoding: 'utf8' }).trim()
const out = { electronCommit: commit, generatedBy: 'scripts/agents-goldens.mjs', states: STATES, limit: LIMIT, install, rows }
await writeFile(join(here, '..', 'parity', 'agents.json'), JSON.stringify(out, null, 1) + '\n', 'utf8')
console.log(`wrote ${rows.length} rows for ${AGENTS.length} agents from Electron ${commit}`)
