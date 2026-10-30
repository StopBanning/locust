// A conversation with no teammate wears its model's logo in the sidebar (0.547).
//
//   node _tools/drive-a-conversation-with-nobody.mjs [--packaged <exe>] [--tag <name>]
//
// Colin, 2026-10-02, of a row with an empty dashed box where the face goes:
// "if a teammate isnt assigned we can just use the logo for whatever model is
// chosen". A profile whose ledger holds three finished conversations nobody
// owns -- on Claude, Codex and a free OpenCode model -- and one teammate's.
// Sends nothing.

import { createHash } from 'node:crypto'
import { mkdtemp, readFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, FREE_ROUTE, recordRoot, say, scratchRepository, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'

const root = localPath(new URL('..', import.meta.url))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-nobody-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-nobody-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const RUNS = [
  { n: 1, prompt: 'yo', runtime: 'claude', model: 'claude-sonnet-5' },
  { n: 2, prompt: 'Summarize the README', runtime: 'codex', model: 'gpt-6-luna' },
  { n: 3, prompt: 'List the TODOs', runtime: 'opencode', model: FREE_ROUTE.model },
  { n: 4, prompt: 'Wren checks the build', runtime: 'opencode', model: FREE_ROUTE.model }
]
for (const run of RUNS) {
  const missionId = `mission_5e000000-0000-4000-8000-00010000000${String(run.n)}`
  const runId = `run_5e000${String(run.n)}`
  const at = new Date(Date.now() - 3_600_000 + run.n * 60_000).toISOString()
  await ledger.createMission({
    missionId, runId, prompt: run.prompt, runtime: run.runtime, model: run.model, requestedRouteId: run.runtime,
    resolvedRouteId: `${run.runtime}-account:default`, cliVersion: null, workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at
  })
  const event = (sequence, type, payload) => ({ id: `event_${String(run.n)}_${String(sequence)}`, runId, missionId, sequence, occurredAt: at, sourceAdapter: run.runtime, type, payload: { ...payload, evidence: { redacted: true } } })
  await ledger.appendEvents(missionId, [
    event(1, 'message.delta', { itemId: 'answer', operation: 'append', text: 'Done.', final: true }),
    event(2, 'run.completed', { usage: { inputTokens: 90, outputTokens: 4 }, process: { exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 2, inputDeliveryFailed: false, outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: at, finishedAt: at } })
  ])
}
await ledger.flush()

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `a-conversation-with-nobody-${tag}`,
  port: 9864,
  workspace,
  profilePath,
  sendsNothing: true,
  outPath: join(recordRoot('a-conversation-with-nobody-2026-10-02'), tag),
  seed: {
    schemaVersion: 1,
    teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-10-02T05:00:00.000Z', route: FREE_ROUTE }],
    missionOwners: { 'mission_5e000000-0000-4000-8000-000100000004': 'tm_wren' },
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' }
  }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 400)}`}`)
}
try {
  await drive.ready()
  await drive.resize(1209, 770)
  const rows = JSON.parse(String(await drive.capture('the sidebar', () => drive.evaluate(`(async () => {
    for (let i = 0; i < 40 && document.querySelectorAll('.lc-conv').length < 4; i += 1) await new Promise((r) => setTimeout(r, 250))
    return JSON.stringify([...document.querySelectorAll('.lc-conv')].map((row) => {
      const mark = row.querySelector('.lc-conv__runtime')
      const box = (mark ?? row.querySelector('.lc-conv__nobody, .lc-teammatebot, svg'))?.getBoundingClientRect()
      const title = row.querySelector('.lc-conv__title') ?? row
      return {
        title: row.title,
        runtime: mark?.querySelector('svg')?.getAttribute('data-runtime') ?? null,
        empty: row.querySelector('.lc-conv__nobody') !== null,
        face: row.querySelector('.lc-conv__runtime') === null && row.querySelector('.lc-conv__nobody') === null,
        width: box ? Math.round(box.width) : null,
        titleX: Math.round(title.getBoundingClientRect().left)
      }
    }))
  })()`))))
  say(JSON.stringify(rows))
  const by = (word) => rows.find((row) => row.title.startsWith(word))
  check('Claude, nobody: the Claude mark', by('yo')?.runtime === 'claude', JSON.stringify(by('yo')))
  check('Codex, nobody: the Codex mark', by('Summarize')?.runtime === 'codex', JSON.stringify(by('Summarize')))
  check('OpenCode, nobody: the OpenCode mark', by('List')?.runtime === 'opencode', JSON.stringify(by('List')))
  check("Wren's: Wren's face, no mark", by('Wren')?.face === true && by('Wren')?.runtime === null, JSON.stringify(by('Wren')))
  check('no empty box left', rows.every((row) => !row.empty), JSON.stringify(rows.map((row) => row.empty)))
  // The face's place beside a conversation's name, from the app's own sizes (16 when this was written; 20 since 0.561).
  const ownerSize = Number(/conversationOwner: (\d+)/.exec(await readFile(join(root, 'apps', 'desktop', 'src', 'renderer', 'src', 'botSizes.ts'), 'utf8'))?.[1])
  check(`every mark sits in the face's ${String(ownerSize)}px place`, Number.isFinite(ownerSize) && rows.filter((row) => row.runtime !== null).every((row) => row.width === ownerSize), JSON.stringify(rows.map((row) => row.width)))
  // 0.550, Colin on 0.549: opening the conversation put the empty box back.
  const opened = JSON.parse(String(await drive.capture('the opened row', () => drive.evaluate(`(async () => {
    const row = [...document.querySelectorAll('.lc-conv')].find((one) => one.title.startsWith('yo'))
    if (!row) return JSON.stringify({ missing: true })
    row.click()
    await new Promise((r) => setTimeout(r, 1500))
    const now = [...document.querySelectorAll('.lc-conv')].find((one) => one.title.startsWith('yo'))
    return JSON.stringify({
      runtime: now?.querySelector('.lc-conv__runtime svg')?.getAttribute('data-runtime') ?? null,
      empty: now?.querySelector('.lc-conv__nobody') !== null
    })
  })()`))))
  check('opened, it keeps the Claude mark', opened.runtime === 'claude' && opened.empty === false, JSON.stringify(opened))
  // 0.621, Colin: "lets make the default teammate for now for a basic chat the ghost dude with terminal face".
  const faces = JSON.parse(String(await drive.capture('a plain chat wears the ghost', () => drive.evaluate(`JSON.stringify([...document.querySelectorAll('.lc-thread [data-bot]')].map((face) => face.getAttribute('data-bot')))`))))
  check('a plain chat\'s reply wears the ghost, never the swarm', faces.length > 0 && faces.every((shape) => shape === 'ghost'), JSON.stringify(faces))
  // 0.551, Colin: "my message is staying in the chatbox for all chats if unsent".
  const drafts = JSON.parse(String(await drive.capture('a draft stays with its conversation', () => drive.evaluate(`(async () => {
    const field = () => document.querySelector('form.command-dock textarea')
    const type = (text) => {
      Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set.call(field(), text)
      field().dispatchEvent(new Event('input', { bubbles: true }))
    }
    const open = async (word) => {
      [...document.querySelectorAll('.lc-conv')].find((one) => one.title.startsWith(word))?.click()
      await new Promise((r) => setTimeout(r, 1200))
    }
    type('half a thought for yo')
    await new Promise((r) => setTimeout(r, 300))
    await open('Summarize')
    const elsewhere = field()?.value ?? null
    await open('yo')
    const back = field()?.value ?? null
    return JSON.stringify({ elsewhere, back })
  })()`))))
  check('another conversation does not show the draft', drafts.elsewhere === '', JSON.stringify(drafts))
  check('its own conversation gets it back', drafts.back === 'half a thought for yo', JSON.stringify(drafts))
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. Four seeded conversations, three with nobody.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
