// Are the small capitalised labels one label now (sweep D4)?
//
//   node _tools/probe-label-frames.mjs [--packaged <exe>] [--tag <name>]
//
// The stylesheet drew small capitalised labels 24 ways; D4 put every one on
// the design system's label -- Geist Mono at --lc-text-mono-label (10.5px),
// tracked --lc-tracking-label (0.14em) or -wide (0.16em) for a section head.
// This walks the screens those labels live on -- a thread with a code block,
// Team, Rooms, New teammate, Settings > Runtimes, the model picker, the
// command palette -- takes a frame of each, and reads each label's face,
// size and tracking as drawn. SPENDS NOTHING.

import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

import { localPath, say, scratchRepository, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const OUT = join(localPath(new URL('../docs/beta-fixes-2026-09-23/', import.meta.url)), `label-frames-${tag}`)
await mkdir(OUT, { recursive: true })

const workspace = resolve(await scratchRepository('locust-labels-ws-'))
const WORKSPACE_ID = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const profile = await mkdtemp(join(tmpdir(), 'locust-labels-'))
await mkdir(join(profile, 'mission-ledger'), { recursive: true })

const at = (second) => new Date(Date.UTC(2026, 8, 23, 5, 0, second)).toISOString()
const PROCESS = {
  exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 3, inputDeliveryFailed: false,
  outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: at(0), finishedAt: at(20)
}
const ANSWER = 'Here is the check:\n\n```typescript\nexport const ready = (value: number): boolean => value > 0\n```\n\nIt returns true for any positive number.'
const metadata = {
  missionId: 'mission_code', runId: 'run_code', prompt: 'Write a readiness check',
  runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default',
  cliVersion: '0.153.0', workspaceId: WORKSPACE_ID, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at(0)
}
const events = [
  { type: 'run.started', occurredAt: at(0), payload: { runtimeThreadId: 'thread-1', evidence: { redacted: true } } },
  { type: 'message.delta', occurredAt: at(10), payload: { itemId: 'answer', operation: 'append', text: ANSWER, final: true, evidence: { redacted: true } } },
  { type: 'run.completed', occurredAt: at(20), payload: { process: PROCESS, evidence: { redacted: true } } }
]
const lines = [JSON.stringify({ schemaVersion: 13, recordType: 'mission.created', ledgerSequence: 1, occurredAt: at(0), metadata })]
events.forEach((event, index) => {
  lines.push(JSON.stringify({
    schemaVersion: 13, recordType: 'mission.event', ledgerSequence: index + 2, occurredAt: event.occurredAt,
    event: { ...event, id: `mission_code:${String(index + 1)}`, runId: 'run_code', missionId: 'mission_code', sequence: index + 1, sourceAdapter: 'codex' }
  }))
})
await writeFile(join(profile, 'mission-ledger', 'mission_code.jsonl'), `${lines.join('\n')}\n`, 'utf8')
await writeFile(join(profile, 'teammates.json'), JSON.stringify({
  schemaVersion: 1,
  teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-09-05T05:00:00.000Z' }],
  missionOwners: { mission_code: 'tm_wren' },
  settings: { swarm: false, relay: false, relayHopCap: 2 }
}), 'utf8')

const drive = await startDrive({
  name: `label-frames-${tag}`,
  port: 9439,
  workspace,
  profilePath: profile,
  sendsNothing: true,
  outPath: OUT,
  ...(packaged === undefined ? {} : { packaged })
})

/** The first element matching each selector, as drawn: face, size, tracking. */
const measure = (selectors) => `(() => JSON.stringify(Object.fromEntries(${JSON.stringify(selectors)}.map((selector) => {
  const node = document.querySelector(selector)
  if (!node) return [selector, null]
  const style = getComputedStyle(node)
  const size = parseFloat(style.fontSize)
  return [selector, { mono: /Geist Mono/.test(style.fontFamily), size: style.fontSize, em: Math.round((parseFloat(style.letterSpacing) / size) * 100) / 100, text: node.textContent.trim().slice(0, 24) }]
}))))()`
const click = (script) => drive.evaluate(`(async () => { ${script}; await new Promise((r) => setTimeout(r, 900)) })()`)
const byText = (selector, text) => `[...document.querySelectorAll(${JSON.stringify(selector)})].find((node) => node.textContent.trim().startsWith(${JSON.stringify(text)}))`

const seen = {}
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}

try {
  await drive.ready()
  await drive.resize(1215, 800)
  const steps = [
    ['a thread with a code block', () => click(`document.querySelector('.lc-conv')?.click()`), ['.lc-code__lang', '.lc-thread__marker']],
    ['the Team screen', () => click(`${byText('button', 'Team')}?.click()`), ['.lc-rostercard__stat dt', '.lc-rostercard__recentlabel', '.lc-rostercard__facts dt']],
    ['the New teammate dialog', () => click(`${byText('button', 'New teammate')}?.click()`), ['.lc-fieldlabel']],
    ['the Rooms screen', () => click(`document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); ${byText('button', 'Rooms')}?.click()`), ['.lc-settings__heading--section', '.lc-roomcardform__label']],
    ['Settings > Runtimes', () => click(`${byText('button', 'Settings')}?.click(); await new Promise((r) => setTimeout(r, 700)); ${byText('button', 'AI agents')}?.click()`), ['.lc-cliartifacts__summary', '.lc-settings__more > summary']],
    ['the command palette', () => click(`document.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', ctrlKey: true, bubbles: true }))`), ['.lc-palette__group']],
    ['the model picker', () => click(`document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); document.querySelector('.lc-conv')?.click(); await new Promise((r) => setTimeout(r, 700)); [...document.querySelectorAll('.lc-control')].find((b) => b.getAttribute('aria-haspopup') === 'listbox')?.click()`), ['.lc-picker__group']]
  ]
  for (const [where, go, selectors] of steps) {
    const read = JSON.parse(await drive.capture(where, async () => {
      await go()
      return drive.evaluate(measure(selectors))
    }))
    for (const [selector, value] of Object.entries(read)) seen[selector] = value
    say(`${where}: ${JSON.stringify(read)}`)
  }
  const drawn = Object.entries(seen).filter(([, value]) => value !== null)
  say(`labels drawn: ${String(drawn.length)} of ${String(Object.keys(seen).length)}`)
  for (const [selector, value] of drawn) {
    check(`${selector} is the mono label`, value.mono && value.size === '10.5px' && (value.em === 0.14 || value.em === 0.16), JSON.stringify(value))
  }
  say(failures === 0 ? '\nLABEL FRAMES PASSED' : `\nLABEL FRAMES: ${String(failures)} FAILED`)
} catch (error) {
  say(`probe failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: 'The screens the small capitalised labels live on, each label read as drawn. Nothing was sent.' })
}
