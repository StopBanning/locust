// Save the record, round trip (0.575): a conversation's right-click menu ->
// "Save the record…" -> the dialog says what the file is -> tick the raw record
// -> Save -> the Markdown and the JSON on disk say what the ledger holds.
//
//   node _tools/drive-save-the-record.mjs [--packaged <exe>]
//
// The native save dialog takes the dev-only LOCUST_RECORD_PATH, so `--packaged`
// drives everything up to the dialog's Save and stops there (a native dialog
// cannot be answered over CDP). Sends nothing: the conversation is seeded into
// the profile's ledger, with a command that ran and one the person declined.

import { createHash } from 'node:crypto'
import { mkdtemp, readFile, stat } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, FREE_ROUTE, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = packaged === undefined ? 'dev' : 'packaged'

const root = localPath(new URL('..', import.meta.url))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-record-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-record-profile-'))
const out = await mkdtemp(join(tmpdir(), 'locust-drive-record-file-'))
const recordPath = join(out, 'Locust record - Wren.md')
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const missionId = 'mission_5e000000-0000-4000-8000-0005ecd00001'
const runId = 'run_5ecd01'
const at = new Date(Date.now() - 3_600_000).toISOString()
const PROMPT = 'Tidy the build folder and fix the typo in the README.'
const ANSWER = 'Fixed the typo. I left build/ alone, as you said.'
const DECLINED = 'The person declined this, and said: use the build script instead'
await ledger.createMission({
  missionId, runId, prompt: PROMPT,
  runtime: 'opencode', model: 'opencode/muse-spark-1.3-contributor-free', requestedRouteId: 'opencode', resolvedRouteId: 'opencode-account:default', cliVersion: null,
  workspaceId, sandbox: 'workspace-write', executionPolicyVersion: 1, createdAt: at
})
let sequence = 0
const event = (type, payload) => {
  sequence += 1
  return { id: `event_${String(sequence)}`, runId, missionId, sequence, occurredAt: at, sourceAdapter: 'opencode', type, payload: { ...payload, evidence: { redacted: true } } }
}
const tool = (type, itemId, fields) => event(type, { itemId, toolKind: 'command_execution', name: 'shell', phase: type === 'tool.started' ? 'started' : 'completed', ...fields })
await ledger.appendEvents(missionId, [
  event('message.delta', { itemId: 'answer', operation: 'append', text: ANSWER, final: true }),
  tool('tool.started', 'c1', { command: 'npm test' }),
  tool('tool.completed', 'c1', { command: 'npm test', output: 'Tests 12 passed', exitCode: 0, status: 'completed', durationMs: 4200 }),
  tool('tool.started', 'c2', { command: 'rm -rf build' }),
  tool('tool.failed', 'c2', { command: 'rm -rf build', status: 'declined', output: DECLINED }),
  event('run.completed', { usage: { inputTokens: 900, outputTokens: 40 }, process: { exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 6, inputDeliveryFailed: false, outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: at, finishedAt: at } })
])
await ledger.flush()

const route = { ...FREE_ROUTE, mode: 'accept-edits' }
const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `save-the-record-${tag}`,
  port: 9861,
  workspace,
  profilePath,
  sendsNothing: true,
  outPath: join(recordRoot('save-the-record-2026-10-04'), tag),
  env: { LOCUST_RECORD_PATH: recordPath },
  seed: {
    schemaVersion: 1,
    teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Custom', roleTitle: 'Office', createdAt: '2026-10-03T00:00:00Z', route }],
    missionOwners: { [missionId]: 'tm_wren' },
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' }
  }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 300)}`}`)
}

try {
  await drive.capture('launch', () => drive.ready())
  await drive.resize(1200, 780)
  const menu = JSON.parse(String(await drive.capture('the conversation\'s menu', () => drive.evaluate(`(async () => {
    let row
    for (let i = 0; i < 60 && row === undefined; i += 1) {
      row = [...document.querySelectorAll('button.lc-conv')].find((b) => /Tidy the build folder/.test(b.innerText))
      if (row === undefined) await new Promise((r) => setTimeout(r, 250))
    }
    if (row === undefined) return JSON.stringify({ row: false, rows: [...document.querySelectorAll('button.lc-conv')].map((b) => b.innerText.slice(0, 60)) })
    const box = row.getBoundingClientRect()
    row.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, clientX: box.left + 10, clientY: box.top + 10 }))
    await new Promise((r) => setTimeout(r, 500))
    return JSON.stringify({ row: true, items: [...document.querySelectorAll('.lc-context__item')].map((one) => one.textContent.trim()) })
  })()`))))
  check('the conversation is in the sidebar', menu.row === true, JSON.stringify(menu))
  check('its menu has "Save the record…" beside Rename and Delete', (menu.items ?? []).some((item) => /^Save the record…/.test(item)), JSON.stringify(menu.items))
  const dialog = JSON.parse(String(await drive.capture('Save the record, the dialog', () => drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('.lc-context__item')].find((one) => /Save the record/.test(one.textContent))?.click()
    await new Promise((r) => setTimeout(r, 500))
    const box = document.querySelector('[role="dialog"][aria-label="Save the record"]')
    return JSON.stringify({ open: box !== null, text: box?.innerText.replace(/\\s+/g, ' ').trim() ?? '' })
  })()`))))
  check('the dialog says once what the file is', dialog.open === true && /This file contains the conversation\. Read it before you send it\./.test(dialog.text), dialog.text)
  // 0.601: the tick says the raw record is not scrubbed.
  check('it offers the raw record as a tick, saying it is not scrubbed', /Include the raw record \(JSON, not scrubbed\)/.test(dialog.text), dialog.text)
  if (packaged !== undefined) {
    say('  (packaged: the native save dialog cannot be answered over CDP, so the drive stops before Save)')
  } else {
    const saved = JSON.parse(String(await drive.capture('saved', () => drive.evaluate(`(async () => {
      const box = document.querySelector('[role="dialog"][aria-label="Save the record"]')
      box.querySelector('input[type="checkbox"]').click()
      await new Promise((r) => setTimeout(r, 200))
      ;[...box.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Save the record…')?.click()
      let said = ''
      for (let i = 0; i < 40 && !/Saved to|could not/.test(said); i += 1) {
        await new Promise((r) => setTimeout(r, 250))
        said = box.querySelector('[role="status"]')?.innerText ?? ''
      }
      return JSON.stringify({ said })
    })()`))))
    check('it says where it saved the file and the raw record', /^Saved to .+\.md, and the raw record beside it at .+\.json\.$/.test(saved.said), saved.said)
    const markdown = await readFile(recordPath, 'utf8').catch(() => '')
    check('the file names the teammate', markdown.startsWith('# Record of a conversation with Wren'), markdown.slice(0, 80))
    check('the file has the person\'s words and the answer', markdown.includes(PROMPT) && markdown.includes(ANSWER))
    check('the file lists the declined call with its recorded words', markdown.includes('Asked: `rm -rf build`') && markdown.includes(DECLINED))
    check('the file has the command that ran, with its output', markdown.includes('npm test') && markdown.includes('Tests 12 passed'))
    // 0.599: the record names the third kind of call it cannot show -- one covered by an earlier Always on the run.
    check('the file names a call covered by an earlier Always among what it cannot show', markdown.includes('covered by an earlier "Always" on this run, which raises no card'), (markdown.match(/Not recorded:[^\n]*/) ?? ['no Not recorded sentence'])[0].slice(0, 220))
    // 0.601: the header says what secret-shaped text it replaced; this seeded conversation carries none.
    check('the header reports the secret-shaped text it replaced (none in this conversation)', /- \*\*Secret-shaped text:\*\* none found\./.test(markdown), (markdown.match(/Secret-shaped text:[^\n]*/) ?? ['no Secret-shaped text line'])[0].slice(0, 160))
    const rawPath = recordPath.replace(/\.md$/, '.json')
    const raw = JSON.parse(await readFile(rawPath, 'utf8').catch(() => '{}'))
    const rawText = JSON.stringify(raw)
    check('the raw record beside it holds the ledger\'s events', rawText.includes(missionId) && rawText.includes('"tool.failed"') && rawText.includes(DECLINED), rawText.slice(0, 200))
    check('nothing else was written', (await stat(join(out, 'Locust record - Wren (2).json')).then(() => true, () => false)) === false)
  }
} catch (error) {
  failures += 1
  say(`  [FAIL] ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. One seeded conversation (a command that ran, one declined); nothing sent.`, extra: `Checks failed: ${String(failures)}` })
}
await sleep(100)
say(failures === 0 ? 'SAVE THE RECORD PASSED' : `SAVE THE RECORD FAILED (${String(failures)})`)
process.exit(failures === 0 ? 0 : 1)
