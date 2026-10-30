// First launch with the runtime CLIs off PATH.
//
//   node _tools/verify-firstrun.mjs
//
// WHAT THIS DOES AND DOES NOT PROVE, because the first version of it claimed
// more than it showed. It launches with a bare PATH, which was meant to
// simulate a machine with no runtimes installed. It does not: discovery
// deliberately looks in known install locations after PATH, so it found four
// runtimes anyway -- correctly, and by design.
//
// So this is a check that **a machine where the CLIs were never added to PATH
// still works**, which is a real and common case and worth guarding. A true
// clean-machine first run cannot be simulated from a developer box that has
// the CLIs installed at all, and pretending otherwise would be the exact
// green-check-that-cannot-go-red this repo keeps finding. That case needs a
// machine, and it is the single biggest unknown before a beta.

import { spawn } from 'node:child_process'
import { mkdtemp, mkdir, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { localPath } from './local-path.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const say = (line) => console.error(line)
let failures = 0
const check = (label, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${label}${detail === undefined ? '' : ` -- ${detail}`}`)
}

// The PATH a person has when they never added the CLI folders themselves:
// Windows, and whatever interpreters their installers put there. Node stays,
// and that is the whole correction -- the first version of this stripped it
// too, npm's `copilot.cmd` shim calls `node`, and the probe failed with
// `'"node"' is not recognized`. I nearly reported that as a Copilot bug. It
// reproduces only on a PATH no real user has: a machine that npm-installed
// the CLI necessarily has node on PATH, and with the real environment the
// same probe returns "GitHub Copilot CLI 1.0.82." and exit 0.
//
// So this removes the RUNTIME directories and nothing else, which is the case
// worth guarding: discovery must find a CLI its installer never put on PATH.
const RUNTIME_DIRECTORY = /codex|claude|cursor|opencode|copilot|antigravity/i
const NARROWED_PATH = (process.env.PATH ?? '')
  .split(';')
  .filter((entry) => entry.length > 0 && !RUNTIME_DIRECTORY.test(entry))
  .join(';')

const profile = await mkdtemp(join(tmpdir(), 'locust-firstrun-'))
await mkdir(profile, { recursive: true })
const child = spawn(ELECTRON, ['.', '--remote-debugging-port=9290', `--user-data-dir=${profile}`], {
  cwd: APP_DIR,
  env: { ...process.env, PATH: NARROWED_PATH },
  stdio: ['ignore', 'pipe', 'pipe']
})
const output = []
child.stdout.on('data', (d) => output.push(String(d)))
child.stderr.on('data', (d) => output.push(String(d)))

try {
  let page
  for (let i = 0; i < 80 && page === undefined; i += 1) {
    await sleep(500)
    if (child.exitCode !== null) throw new Error(`app exited ${child.exitCode}: ${output.join('').slice(-400)}`)
    try {
      const list = await (await fetch('http://127.0.0.1:9290/json/list')).json()
      page = list.find((t) => t.type === 'page' && t.webSocketDebuggerUrl && !t.url.includes('#splash'))
    } catch {
      // not up yet
    }
  }
  say(`       PATH entries removed: ${String((process.env.PATH ?? '').split(';').filter((e) => e.length > 0).length - NARROWED_PATH.split(';').length)}`)
  check('the app starts with no runtime CLI on PATH', page !== undefined)
  if (page === undefined) throw new Error('renderer never came up')

  const socket = new WebSocket(page.webSocketDebuggerUrl)
  await new Promise((resolve, reject) => {
    socket.addEventListener('open', resolve, { once: true })
    socket.addEventListener('error', reject, { once: true })
  })
  let id = 0
  const pending = new Map()
  socket.addEventListener('message', (event) => {
    const message = JSON.parse(event.data)
    const waiter = pending.get(message.id)
    if (waiter) { pending.delete(message.id); waiter(message) }
  })
  const evaluate = (expression) =>
    new Promise((resolve) => {
      const next = ++id
      pending.set(next, (message) => resolve(message.result?.result?.value))
      socket.send(JSON.stringify({
        id: next,
        method: 'Runtime.evaluate',
        params: { expression, awaitPromise: true, returnByValue: true }
      }))
    })

  // Wait out discovery rather than reading mid-flight, so "loading" is not
  // mistaken for "found nothing".
  const settled = await evaluate(`(async () => {
    for (let i = 0; i < 120; i += 1) {
      const field = document.querySelector('form.command-dock textarea')
      if (field && !/Checking local runtimes/.test(field.placeholder)) return field.placeholder
      await new Promise(r => setTimeout(r, 500))
    }
    return 'still checking'
  })()`)
  check('discovery finishes rather than hanging', settled !== 'still checking', JSON.stringify(settled))

  const seen = JSON.parse(await evaluate(`JSON.stringify({
    placeholder: (document.querySelector('form.command-dock textarea') || {}).placeholder || '',
    sendDisabled: (document.querySelector('form.command-dock .send-button') || {}).disabled,
    connected: (document.querySelector('.lc-connected') || { innerText: '' }).innerText.trim(),
    body: document.body.innerText.replace(/[ \\t]+/g, ' ')
  })`))

  say(`       placeholder: ${JSON.stringify(seen.placeholder)}`)
  say(`       connected line: ${JSON.stringify(seen.connected)}`)

  // The count on screen must equal the number of runtimes actually reported
  // READY. The first version of this check was written as a regex with `\\b`
  // inside a normal literal, which matches a literal backslash and therefore
  // never matched anything -- it passed while the page said "4 runtimes
  // connected", which is the check-that-cannot-go-red this repo keeps finding
  // in its own tools.
  // Counted by the tags that mean "this can run a mission", not by the word
  // READY alone: an EXPERIMENTAL adapter completes missions today and counts
  // correctly while printing a different tag. Counting only READY made the app
  // look wrong when it was right, which is the harness measuring itself.
  const claimed = Number(/([0-9]+) runtimes? connected/.exec(seen.connected)?.[1] ?? '-1')
  const runnable = (seen.body.match(/READY|ACTIVE|EXPERIMENTAL/g) ?? []).length
  say(`       tags: ${JSON.stringify(seen.body.match(/READY|ACTIVE|EXPERIMENTAL|NOT INSTALLED|PLANNED|UNAVAILABLE|SIGN IN|PREVIEW/g) ?? [])}`)
  check(
    'the connected count equals the runtimes that can actually run one',
    claimed === runnable,
    `says ${String(claimed)}, runnable ${String(runnable)}`
  )
  check(
    'the composer refuses to start a mission there is nothing to run',
    seen.sendDisabled !== false,
    `send disabled: ${String(seen.sendDisabled)}`
  )
  // Every runtime shown carries a state and a reason for it -- READY with a
  // version, PLANNED for one not built, UNAVAILABLE with what went wrong. The
  // rule being guarded is that no runtime is listed with no explanation.
  const states = (seen.body.match(/READY|PLANNED|UNAVAILABLE|NOT INSTALLED/g) ?? []).length
  check('every runtime shown carries a state', states >= 4, `${String(states)} states listed`)
  check(
    'a runtime it could not probe says so rather than reading as ready',
    !/did not answer its version probe[\s\S]{0,40}READY/.test(seen.body)
  )

  // The count is about runtimes, so read the RUNTIMES, not the word READY --
  // an experimental adapter runs missions today and correctly counts while
  // printing a different tag, so counting the word overstates the problem.
  const rows = await evaluate(`(() => {
    const seen = []
    for (const row of document.querySelectorAll('.lc-runtimerow')) {
      seen.push((row.innerText || '').replace(/[\\r\\n]+/g, ' | ').slice(0, 90))
    }
    return JSON.stringify(seen)
  })()`)
  say(`       runtime rows: ${rows}`)

  say('')
  say('--- what a first-time user actually sees ---')
  say(seen.body.slice(0, 900))
} catch (error) {
  say(String(error))
  failures += 1
} finally {
  child.kill()
  await sleep(800)
  await rm(profile, { recursive: true, force: true }).catch(() => undefined)
}

say(failures === 0 ? '\nFIRST RUN OK' : `\n${failures} FIRST-RUN PROBLEM(S)`)
process.exit(failures === 0 ? 0 : 1)
