#!/usr/bin/env node
// What Electron's own discovery decides about each agent from what its checks answer: runtime-adapters'
// discoverInstalledRuntimes, run with a made-up command runner, locator and file check, then main's publicStatus.
// Writes parity/discovery.json for core/src/agents_tests.rs.
//
//   node scripts/discovery-goldens.mjs [--electron <checkout>]
//
// The help texts are the real ones this Mac's agents printed on 12 Nov 2026 (parity/agents/*-help.txt, read-only
// `--help` commands, the one home path in them replaced); everything else each check answers is made up per scenario.
// No agent is run and nothing is sent anywhere. The checkout must have apps/desktop's dependencies installed.

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

const work = await mkdtemp(join(tmpdir(), 'locust-discovery-goldens-'))
await writeFile(join(work, 'entry.ts'), `
export { discoverInstalledRuntimes } from ${at('packages/runtime-adapters/src/discovery.ts')}
export { publicStatus } from ${at('apps/desktop/src/main/runtime-discovery.ts')}
export { plannedRuntimes } from ${at('apps/desktop/src/shared/runtime-integration.ts')}
`, 'utf8')
await esbuild.build({
  entryPoints: [join(work, 'entry.ts')], bundle: true, format: 'esm', platform: 'node', target: 'node20',
  outfile: join(work, 'entry.mjs'), nodePaths: [join(DESKTOP, 'node_modules'), join(ROOT, 'node_modules')],
  absWorkingDir: ROOT, logLevel: 'warning', external: ['electron']
})
const m = await import(pathToFileURL(join(work, 'entry.mjs')).href)

const help = async (id) => readFile(join(here, '..', 'parity', 'agents', `${id}-help.txt`), 'utf8').catch(() => '')
// What each agent answers when all is well: its version line, its sign-in check, its model list.
const WELL = {
  codex: { command: 'codex', version: 'codex-cli 0.153.0', ready: 'Logged in using ChatGPT' },
  claude: { command: 'claude', version: '2.1.309 (Claude Code)', ready: '{"loggedIn": true, "authMethod": "claude.ai"}' },
  cursor: { command: 'cursor-agent', version: '2026.11.10-abc1234', ready: 'Logged in as someone@example.com', models: 'Available models\n\nauto - Auto\ngpt-6 - GPT-6\n' },
  gemini: { command: 'gemini', version: '0.40.0', ready: 'No previous sessions found.' },
  opencode: { command: 'opencode', version: '1.19.5', ready: 'opencode/big-pickle\nopencode/deepseek-v4.1-flash-free\n', models: 'opencode/big-pickle\n{"id":"big-pickle"}\nopencode/deepseek-v4.1-flash-free\n{"id":"deepseek-v4.1-flash-free"}\n' },
  copilot: { command: 'copilot', version: 'GitHub Copilot CLI 1.0.83' },
  muse: { command: 'muse', version: 'Muse Code 1.3.0' },
  kilo: { command: 'kilo', version: '7.8.18' }
}
// Each scenario: how the version and sign-in checks answer, and whether the credentials file is there.
const SCENARIOS = {
  'not-found': { found: false },
  'signed-in': {},
  'signed-out-in-words': { readiness: { exitCode: 1, stdout: '', stderr: 'Not logged in. Run the login command first.' } },
  'failing-silently': { readiness: { exitCode: 1, stdout: '', stderr: '' } },
  'timed-out': { readiness: { exitCode: null, stdout: '', stderr: '', timedOut: true } },
  'version-fails': { version: { exitCode: 1, stdout: '', stderr: 'error' } },
  'no-help': { help: { exitCode: 1, stdout: '', stderr: '' } },
  'credentials-missing': { credentials: false },
  'version-three': { versionText: '3.0.0' }
}

const out = []
for (const [id, well] of Object.entries(WELL)) {
  const helpText = await help(id)
  for (const [name, s] of Object.entries(SCENARIOS)) {
    const asked = []
    const runner = {
      async run(command) {
        const joined = command.args.join(' ')
        asked.push(joined)
        const ok = (stdout) => ({ exitCode: 0, stdout, stderr: '' })
        if (joined === '--version' && s.version !== undefined) return s.version
        if (joined === '--version' && (id === 'copilot' || id === 'muse' || id === 'kilo') && s.readiness !== undefined) return s.readiness
        if (joined === '--version') return ok(s.versionText ?? well.version)
        if (/--help$/.test(joined)) return s.help ?? ok(helpText)
        if (well.models !== undefined && (joined === '--list-models' || joined === 'models --verbose')) return s.readiness ?? ok(well.models)
        return s.readiness ?? ok(well.ready ?? '')
      }
    }
    const locator = { async find(commandName) {
      if (s.found === false || commandName !== well.command) return undefined
      return { commandName, discoveredPath: `/usr/local/bin/${commandName}`, executablePath: `/usr/local/bin/${commandName}`, prefixArgs: [], kind: 'native' }
    } }
    const statFile = async (path) => {
      if (s.credentials === false) throw new Error(`no ${path}`)
      return { size: 10, mtimeMs: 0 }
    }
    const [found] = await m.discoverInstalledRuntimes({
      runner, locator, statFile, only: new Set([id]), readinessFromVersion: m.plannedRuntimes(),
      credentialLookup: { variables: {}, homeDirectory: '/home/someone' }
    })
    const status = m.publicStatus(found)
    out.push({ id, scenario: name, status, readiness: found.readiness, version: found.version?.version ?? null, asked })
  }
}
const commit = execFileSync('git', ['-C', ROOT, 'rev-parse', '--short', 'HEAD'], { encoding: 'utf8' }).trim()
await writeFile(join(here, '..', 'parity', 'discovery.json'), JSON.stringify({ electronCommit: commit, generatedBy: 'scripts/discovery-goldens.mjs', well: WELL, scenarios: SCENARIOS, results: out }, null, 1) + '\n', 'utf8')
console.log(`wrote ${out.length} discovery results for ${Object.keys(WELL).length} agents from Electron ${commit}`)
