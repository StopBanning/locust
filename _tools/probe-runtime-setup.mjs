// Does Settings say what each runtime has set up for itself?
//
//   node _tools/probe-runtime-setup.mjs
//
// Launches the BUILT app on a throwaway profile and a scratch folder, opens
// Settings, and reads the line under each runtime. The expectation is
// computed from THIS machine's real files -- the same files the host reads
// -- so the probe is a comparison of two readers, not a fixture: Claude
// Code's ~/.claude/settings.json hooks, Codex's ~/.codex/config.toml table
// headers. A runtime with nothing configured must say so in words.

import { spawn } from 'node:child_process'
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { join } from 'node:path'
import { localPath } from './local-path.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const NPM_DIR = join(homedir(), 'AppData', 'Roaming', 'npm')
const PORT = 9294
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const say = (line) => console.error(line)
let failures = 0
const check = (label, ok, detail = '') => {
  if (ok) say(`  [PASS] ${label}${detail ? ` -- ${detail}` : ''}`)
  else { failures += 1; say(`  [FAIL] ${label}${detail ? ` -- ${detail}` : ''}`) }
}
try {
  const already = await fetch(`http://127.0.0.1:${String(PORT)}/json/list`, { signal: AbortSignal.timeout(1500) })
  if (already.ok) { say(`  [FAIL] something is already debugging on port ${String(PORT)}`); process.exit(1) }
} catch { /* free */ }

// Independent reading of the same files, for the expectation.
const home = homedir()
const readOr = async (path) => { try { return await readFile(path, 'utf8') } catch { return undefined } }
const claudeSettings = await readOr(join(home, '.claude', 'settings.json'))
const claudeHooks = claudeSettings === undefined ? [] : Object.entries(JSON.parse(claudeSettings).hooks ?? {}).map(([event, list]) => `${event} (${String(Array.isArray(list) ? list.length : 1)})`)
const codexToml = await readOr(join(home, '.codex', 'config.toml'))
const codexServers = codexToml === undefined ? [] : [...codexToml.matchAll(/^\s*\[mcp_servers\.([^\]\s.]+)\]/gm)].map((m) => m[1])
const codexNotify = codexToml !== undefined && /^\s*notify\s*=/m.test(codexToml)
say(`expectation from this machine: claude hooks ${JSON.stringify(claudeHooks)}, codex servers ${JSON.stringify(codexServers)}, codex notify ${String(codexNotify)}`)

const workspace = await mkdtemp(join(tmpdir(), 'locust-setup-ws-'))
await writeFile(join(workspace, 'README.md'), '# scratch\n', 'utf8')
const profile = await mkdtemp(join(tmpdir(), 'locust-setup-'))
const child = spawn(ELECTRON, [APP_DIR, `--remote-debugging-port=${String(PORT)}`, `--user-data-dir=${profile}`], {
  cwd: workspace,
  env: { ...process.env, PATH: `${NPM_DIR};${process.env.PATH ?? ''}` },
  stdio: ['ignore', 'pipe', 'pipe']
})
child.stdout.on('data', () => undefined)
child.stderr.on('data', () => undefined)
try {
  let page
  for (let i = 0; i < 80 && page === undefined; i += 1) {
    await sleep(500)
    if (child.exitCode !== null) throw new Error(`app exited ${String(child.exitCode)}`)
    try {
      const list = await (await fetch(`http://127.0.0.1:${String(PORT)}/json/list`)).json()
      page = list.find((t) => t.type === 'page' && t.webSocketDebuggerUrl && !t.url.includes('#splash'))
    } catch { /* not up */ }
  }
  if (page === undefined) throw new Error('renderer never came up')
  const socket = new WebSocket(page.webSocketDebuggerUrl)
  await new Promise((res, rej) => { socket.addEventListener('open', res, { once: true }); socket.addEventListener('error', rej, { once: true }) })
  let id = 0
  const pending = new Map()
  socket.addEventListener('message', (event) => {
    const message = JSON.parse(event.data)
    const waiter = pending.get(message.id)
    if (waiter) { pending.delete(message.id); waiter(message) }
  })
  const evaluate = (expression) => new Promise((resolve_) => {
    const next = ++id
    const gaveUp = setTimeout(() => { if (pending.delete(next)) resolve_(undefined) }, 60_000)
    pending.set(next, (message) => {
      clearTimeout(gaveUp)
      const thrown = message.result?.exceptionDetails
      if (thrown !== undefined) say(`  eval threw: ${thrown.exception?.description ?? ''}`.slice(0, 200))
      resolve_(message.result?.result?.value)
    })
    socket.send(JSON.stringify({ id: next, method: 'Runtime.evaluate', params: { expression, awaitPromise: true, returnByValue: true } }))
  })

  say('1. the app starts')
  const ready = await evaluate(`(async () => {
    for (let i = 0; i < 240; i += 1) {
      const field = document.querySelector('form.command-dock textarea')
      if (field && !/Checking local runtimes/.test(field.placeholder)) return true
      await new Promise(r => setTimeout(r, 250))
    }
    return false
  })()`)
  check('discovery finished', ready === true)

  say('2. Settings lists what each runtime has set up')
  const rows = JSON.parse(await evaluate(`(async () => {
    document.querySelector('button[title="Settings (Ctrl 3)"]').click()
    for (let i = 0; i < 40; i += 1) {
      await new Promise(r => setTimeout(r, 250))
      if (document.querySelector('.lc-runtimerow__setup')) break
    }
    return JSON.stringify([...document.querySelectorAll('.lc-runtimerow')].map(r => ({
      // The name's first text node: since 0.32.0 the version sits beside it in a span.
      name: r.querySelector('.lc-runtimerow__name')?.childNodes[0]?.textContent?.trim(),
      setup: r.querySelector('.lc-runtimerow__setup')?.innerText ?? null,
      title: r.querySelector('.lc-runtimerow__setup')?.getAttribute('title') ?? null
    })))
  })()`))
  for (const row of rows) say(`       ${row.name}: ${String(row.setup)}`)
  const byName = Object.fromEntries(rows.map((r) => [r.name, r]))
  const claude = byName['Claude Code']
  check('Claude Code shows its hooks by event and count', claude !== undefined && claudeHooks.every((h) => String(claude.setup).includes(h)), String(claude?.setup))
  check('the tooltip names the file it was read from, not its contents', /read: .*settings\.json/.test(String(claude?.title)) && !/command/.test(String(claude?.title)), String(claude?.title).slice(0, 120))
  const codex = byName['Codex CLI']
  check('Codex CLI shows its MCP servers from the TOML headers', codex !== undefined && codexServers.every((s) => String(codex.setup).includes(s)), String(codex?.setup))
  check('and its notify hook', !codexNotify || /notify \(1\)/.test(String(codex?.setup)), String(codex?.setup))
  const quiet = rows.filter((r) => ['Cursor Agent', 'OpenCode', 'Copilot CLI'].includes(r.name))
  check('a runtime with nothing configured says so in words', quiet.length > 0 && quiet.every((r) => r.setup !== null && (/No MCP servers(, hooks, skills or agents| or hooks) configured/.test(String(r.setup)) || /MCP:|Hooks:|Skills:|Agents:/.test(String(r.setup)))), JSON.stringify(quiet.map((r) => [r.name, r.setup])))
  const planned = rows.filter((r) => ['Gemini CLI', 'OmniRoute', 'Antigravity'].includes(r.name))
  check('planned and hub runtimes carry no setup line', planned.every((r) => r.setup === null), JSON.stringify(planned.map((r) => [r.name, r.setup])))
  check('no command lines or secrets anywhere on the screen', !/SECRET|--token|\.exe/i.test(await evaluate(`document.querySelector('.lc-runtimelist').innerText`)))
} catch (error) {
  failures += 1
  say(`  [FAIL] ${error instanceof Error ? error.message : String(error)}`)
} finally {
  try { child.kill() } catch { /* gone */ }
  await sleep(1500)
  await rm(profile, { recursive: true, force: true }).catch(() => undefined)
  await rm(workspace, { recursive: true, force: true }).catch(() => undefined)
}
if (failures > 0) { say(`\n${String(failures)} RUNTIME SETUP PROBE FAILURE(S)`); process.exit(1) }
say('\nRUNTIME SETUP PROBE PASSED')
