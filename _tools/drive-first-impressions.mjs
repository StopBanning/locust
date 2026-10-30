// First impressions: every main screen, photographed for a design review.
//
//   node _tools/drive-first-impressions.mjs [--packaged <exe>] [--tag <label>]
//
// Colin, 2026-09-26: "as a UI snob myself we have to win over these peoples
// hearts visually, people eat with their eyes." And Locust is for "quite
// literally ANY ai user" -- code, financial advice, design. So this seeds the
// profile a new person might have after a day: three teammates with three
// kinds of work on three routes, one finished conversation each, a few kept
// memories -- and photographs home, each conversation, the route picker, Team,
// Memory, Missions, Rooms and Settings at 1440x900, then home and a
// conversation at 1280x800 and 1920x1080.
//
// The conversations are ILLUSTRATIVE: written here in the exact record shapes
// Codex and OpenCode print (see packages/runtime-adapters/test/fixtures), run
// through Locust's own normalizers into a real ledger, so the screen draws
// them the way it draws real runs. No model wrote them; no real names. Sends
// nothing.

import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, git, say, scratchRepository, sleep, startDrive, teammateFace, recordRoot } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag')
const outPath = tag === undefined ? undefined : join(recordRoot('beta-fixes-2026-09-24'), `first-impressions-${tag}`)
if (outPath !== undefined) await mkdir(outPath, { recursive: true })

const root = join(localPath(new URL('..', import.meta.url)))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const adapters = await import(pathToFileURL(join(root, 'packages', 'runtime-adapters', 'dist', 'index.js')).href)

// A small real project, so file names in the conversations are real ones.
const workspace = await scratchRepository('locust-drive-firstlook-ws-', 'A small web app: a signup form, a pricing page and a style sheet.\n')
await mkdir(join(workspace, 'src'), { recursive: true })
await writeFile(join(workspace, 'src', 'signup.ts'), [
  'export function validEmail(value: string): boolean {',
  "  return value.includes('@')",
  '}',
  ''
].join('\n'), 'utf8')
await writeFile(join(workspace, 'src', 'theme.css'), ':root {\n  --accent: #3b82f6;\n}\n', 'utf8')
await git(['add', '-A'], workspace)
await git(['commit', '-q', '-m', 'app'], workspace)

const profilePath = await mkdtemp(join(tmpdir(), 'locust-drive-firstlook-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const completion = (at, count) => ({ exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: count, cancelled: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, startedAt: at, finishedAt: at })

// ---- Wren, on Codex: a code change with a test run. -------------------------
{
  const at = '2026-09-26T09:12:00.000Z'
  await ledger.createMission({
    missionId: 'mission_wren', runId: 'run_wren', prompt: 'The signup form accepts "a@" as an email. Tighten the check and make sure the tests still pass.',
    runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: '0.156.1',
    workspaceId, sandbox: 'workspace-write', mode: 'accept-edits', executionPolicyVersion: 1, createdAt: at
  })
  const codex = adapters.createCodexEventNormalizer({ runId: 'run_wren', missionId: 'mission_wren', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: '0.156.1', now: () => new Date(at) })
  const records = [
    { type: 'thread.started', thread_id: 'thread_wren' },
    { type: 'item.completed', item: { id: 'r1', type: 'reasoning', text: 'Checking how validEmail is used before changing it.' } },
    // Each action as Codex reports it: started, then completed.
    { type: 'item.started', item: { id: 'c1', type: 'command_execution', command: 'rg -n "validEmail" src', aggregated_output: '', status: 'in_progress' } },
    { type: 'item.completed', item: { id: 'c1', type: 'command_execution', command: 'rg -n "validEmail" src', aggregated_output: 'src/signup.ts:1:export function validEmail(value: string): boolean {\n', exit_code: 0, status: 'completed' } },
    { type: 'item.started', item: { id: 'f1', type: 'file_change', changes: [{ path: join(workspace, 'src', 'signup.ts'), kind: 'update' }], status: 'in_progress' } },
    { type: 'item.completed', item: { id: 'f1', type: 'file_change', changes: [{ path: join(workspace, 'src', 'signup.ts'), kind: 'update' }], status: 'completed' } },
    { type: 'item.started', item: { id: 'c2', type: 'command_execution', command: 'npm test', aggregated_output: '', status: 'in_progress' } },
    { type: 'item.completed', item: { id: 'c2', type: 'command_execution', command: 'npm test', aggregated_output: '> app@1.0.0 test\n> vitest run\n\n ✓ src/signup.test.ts (6 tests) 12ms\n\n Test Files  1 passed (1)\n      Tests  6 passed (6)\n', exit_code: 0, status: 'completed' } },
    { type: 'item.completed', item: { id: 'a1', type: 'agent_message', text: [
      'Done. `validEmail` now requires a name, one `@`, and a domain with a dot:',
      '',
      '```ts',
      'export function validEmail(value: string): boolean {',
      '  return /^[^\\s@]+@[^\\s@]+\\.[^\\s@]+$/.test(value.trim())',
      '}',
      '```',
      '',
      '- `a@` and `a@b` are now refused; `ana@example.com` still passes.',
      '- Leading and trailing spaces are ignored, since people paste addresses.',
      '- All 6 tests pass.'
    ].join('\n') } },
    { type: 'turn.completed', usage: { input_tokens: 18240, cached_input_tokens: 12800, output_tokens: 612 } }
  ]
  const events = records.flatMap((record, index) => codex.accept({ sequence: index + 1, raw: JSON.stringify(record) }))
  await ledger.appendEvents('mission_wren', [...events, ...codex.finish(completion(at, records.length))])
}

// OpenCode records, in the shape `opencode run --format json` prints them.
const openCodeRun = (session) => {
  let clock = Date.parse('2026-09-26T10:00:00.000Z')
  let n = 0
  const id = (prefix) => `${prefix}_${session}${String((n += 1)).padStart(4, '0')}`
  const records = []
  let message = id('msg')
  const push = (type, part) => records.push({ type, timestamp: (clock += 700), sessionID: `ses_${session}`, part: { id: id('prt'), messageID: message, sessionID: `ses_${session}`, ...part } })
  return {
    records,
    start: () => {
      message = id('msg')
      push('step_start', { type: 'step-start' })
    },
    text: (text) => push('text', { type: 'text', text, time: { start: clock, end: clock + 4 } }),
    tool: (tool, input, output, metadata = {}) => push('tool_use', { type: 'tool', tool, callID: id('call'), state: { status: 'completed', input, output, metadata, title: '', time: { start: clock, end: clock + 40 } } }),
    finish: (reason, output) => push('step_finish', { type: 'step-finish', reason, tokens: { total: 9000 + output, input: 9000, output, reasoning: 0, cache: { write: 0, read: 0 } }, cost: 0 })
  }
}

const seedOpenCode = async (missionId, prompt, at, build) => {
  await ledger.createMission({
    missionId, runId: `run_${missionId}`, prompt,
    runtime: 'opencode', model: 'opencode/ling-3.0-flash-fin-free', requestedRouteId: 'opencode', resolvedRouteId: 'opencode-account:default', cliVersion: '1.18.27',
    workspaceId, sandbox: 'workspace-write', mode: 'accept-edits', executionPolicyVersion: 1, createdAt: at
  })
  const run = openCodeRun(missionId.replace(/[^a-z]/g, ''))
  build(run)
  const normalizer = adapters.createOpenCodeEventNormalizer({ runId: `run_${missionId}`, missionId, cliVersion: '1.18.27', now: () => new Date(at) })
  const events = run.records.flatMap((record, index) => normalizer.accept({ sequence: index + 1, raw: JSON.stringify(record) }))
  await ledger.appendEvents(missionId, [...events, ...normalizer.finish(completion(at, run.records.length))])
}

// ---- Penny, on OpenCode: a money question, answered with a table. ----------
await seedOpenCode('mission_penny', 'I have $12,000 saved and want to use it within three years. How should I think about where to keep it?', '2026-09-26T10:30:00.000Z', (run) => {
  run.start()
  run.text([
    'For money you plan to use within three years, the first job is keeping it safe and easy to reach; growth comes second. A common way to think about it:',
    '',
    '| Bucket | Share | Why |',
    '| --- | --- | --- |',
    '| Emergency fund, high-yield savings | $4,000 | Three months of costs you can reach the same day |',
    '| Short-term Treasuries or CDs | $6,000 | Pays more than savings; matures before you need it |',
    '| Broad index fund | $2,000 | Only money you could leave alone if markets fall |',
    '',
    '**Before you pick:** check what each account pays today, and whether the three-year date is fixed. If it might come sooner, move more into the first two rows.',
    '',
    'This is a way to think about it, not personal financial advice.'
  ].join('\n'))
  run.finish('stop', 240)
})

// ---- Iris, on OpenCode: a design task that edits a file. --------------------
await seedOpenCode('mission_iris', 'Give the app a calmer palette -- it should feel like a quiet morning, not a dashboard. Update the theme.', '2026-09-26T11:05:00.000Z', (run) => {
  run.start()
  run.text('Reading the current theme first.')
  run.tool('read', { filePath: join(workspace, 'src', 'theme.css') }, '<content>\n1: :root {\n2:   --accent: #3b82f6;\n3: }\n</content>')
  run.finish('tool-calls', 40)
  run.start()
  run.tool('edit', { filePath: join(workspace, 'src', 'theme.css'), oldString: '  --accent: #3b82f6;', newString: '  --paper: #f6f3ee;\n  --ink: #2b2a28;\n  --accent: #7a9e8e;\n  --accent-soft: #dfe8e2;' }, 'Edit applied successfully.', {
    diff: [
      `Index: ${join(workspace, 'src', 'theme.css')}`,
      '===================================================================',
      `--- ${join(workspace, 'src', 'theme.css')}`,
      `+++ ${join(workspace, 'src', 'theme.css')}`,
      '@@ -1,3 +1,6 @@',
      ' :root {',
      '-  --accent: #3b82f6;',
      '+  --paper: #f6f3ee;',
      '+  --ink: #2b2a28;',
      '+  --accent: #7a9e8e;',
      '+  --accent-soft: #dfe8e2;',
      ' }',
      ''
    ].join('\n')
  })
  run.finish('tool-calls', 120)
  run.start()
  run.text([
    'Updated `src/theme.css` with a softer set:',
    '',
    '- **Paper** `#f6f3ee` -- a warm off-white instead of pure white',
    '- **Ink** `#2b2a28` -- near-black text, easier on the eyes than #000',
    '- **Sage** `#7a9e8e` -- the accent, for buttons and links',
    '- **Mist** `#dfe8e2` -- a pale sage for highlights and selected rows',
    '',
    'The blue accent is gone; sage on paper keeps a 4.6:1 contrast for text-sized links.'
  ].join('\n'))
  run.finish('stop', 180)
})

const T0 = '2026-09-20T05:00:00.000Z'
const kept = (memoryId, text) => ({ memoryId, text, scope: 'workspace', workspaceId, workspaceName: 'firstlook', by: { name: 'you' }, createdAt: T0, status: 'kept', enabled: true })

const drive = await startDrive({
  name: 'first-impressions',
  port: 9595,
  workspace,
  profilePath,
  sendsNothing: true,
  ...(packaged === undefined ? {} : { packaged }),
  ...(outPath === undefined ? {} : { outPath }),
  seed: {
    schemaVersion: 1,
    teammates: [
      { teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: T0, route: { runtime: 'codex', model: 'account-default', mode: 'accept-edits' } },
      { teammateId: 'tm_penny', name: 'Penny', hue: 'butter', role: 'Custom', roleTitle: 'Money', createdAt: T0, route: { runtime: 'opencode', model: 'opencode/ling-3.0-flash-fin-free', mode: 'ask' } },
      { teammateId: 'tm_iris', name: 'Iris', hue: 'rose', role: 'Custom', roleTitle: 'Design', createdAt: T0, route: { runtime: 'opencode', model: 'opencode/ling-3.0-flash-fin-free', mode: 'accept-edits' } }
    ],
    missionOwners: { mission_wren: 'tm_wren', mission_penny: 'tm_penny', mission_iris: 'tm_iris' },
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'on' }
  },
  files: {
    'memories.json': {
      schemaVersion: 1,
      memories: [
        kept('mem_tests', 'Run npm test before calling a code change done.'),
        kept('mem_voice', 'Keep explanations short; lead with the answer.'),
        kept('mem_brand', 'The brand accent is sage #7a9e8e on paper #f6f3ee.')
      ]
    }
  }
})

// A conversation's row names its owner (`data-teammate`), so the row is found
// by who it belongs to rather than by its title.
const openConversation = (name) => drive.evaluate(`(async () => {
  const id = ${JSON.stringify('tm_')} + ${JSON.stringify(name.toLowerCase())}
  const row = [...document.querySelectorAll('.lc-conv')].find((b) => b.querySelector('[data-teammate]')?.getAttribute('data-teammate') === id)
  if (!row) return 'no conversation row for ' + id + '; rows: ' + [...document.querySelectorAll('.lc-conv')].map((b) => b.querySelector('[data-teammate]')?.getAttribute('data-teammate')).join(',')
  row.click()
  await new Promise((r) => setTimeout(r, 1300))
  return 'opened: ' + (document.querySelector('.lc-thread')?.innerText.replace(/\\s+/g, ' ').slice(0, 100) ?? 'no thread')
})()`)
const screen = (key) => drive.evaluate(`(async () => {
  window.dispatchEvent(new KeyboardEvent('keydown', { key: '${key}', ctrlKey: true, bubbles: true }))
  await new Promise((r) => setTimeout(r, 1100))
  return (document.querySelector('main, .lc-screen')?.innerText ?? '').replace(/\\s+/g, ' ').slice(0, 160)
})()`)
const home = () => drive.evaluate(`(async () => {
  document.querySelector('.lc-brand__lockup')?.click()
  await new Promise((r) => setTimeout(r, 1100))
  return (document.querySelector('main')?.innerText ?? '').replace(/\\s+/g, ' ').slice(0, 160)
})()`)

try {
  await drive.capture('launch', () => drive.ready())
  await drive.resize(1440, 900)
  await sleep(1500)
  await drive.capture('1440 home', home)
  await drive.capture('1440 Wren: a code change on Codex', () => openConversation('Wren'))
  await drive.capture('1440 Penny: a money question on OpenCode', () => openConversation('Penny'))
  await drive.capture('1440 Iris: a design change on OpenCode', () => openConversation('Iris'))
  await drive.capture('1440 the route picker, open', () => drive.evaluate(`(async () => {
    // The route control is the one with a listbox behind it (drive-lib's pickRouteScript).
    const chip = [...document.querySelectorAll('.lc-control')].find((b) => b.getAttribute('aria-haspopup') === 'listbox')
    if (!chip) return 'no route chip found'
    chip.click()
    await new Promise((r) => setTimeout(r, 1200))
    return 'opened: ' + (document.querySelector('[role=dialog], [role=listbox], .lc-picker')?.innerText.replace(/\\s+/g, ' ').slice(0, 160) ?? 'no picker')
  })()`))
  await drive.evaluate(`document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))`)
  await sleep(500)
  await drive.capture('1440 Missions (Ctrl 1)', () => screen('1'))
  await drive.capture('1440 Team (Ctrl 2)', () => screen('2'))
  await drive.capture('1440 Settings (Ctrl 3)', () => screen('3'))
  // "What's new" in Settings (settingsPages.ts); it was Changelog.
  for (const page of ['AI agents', 'Teammates', 'Appearance', 'General', "What's new"]) {
    await drive.capture(`1440 Settings: ${page}`, () => drive.evaluate(`(async () => {
      const item = [...document.querySelectorAll('.lc-settings button, .lc-settings a, nav button')].find((b) => b.textContent.trim() === ${JSON.stringify(page)})
      if (!item) return 'no ' + ${JSON.stringify(page)} + ' in the Settings list'
      item.click()
      await new Promise((r) => setTimeout(r, 900))
      return [...document.querySelectorAll('.lc-settings__heading')].map((h) => h.textContent).join(' | ').slice(0, 160)
    })()`))
  }
  await drive.capture('1440 Rooms (Ctrl 4)', () => screen('4'))
  await drive.capture('1440 Memory (Ctrl 5)', () => screen('5'))
  // The surfaces the first pass never photographed (0.354): the Routines
  // screen, a conversation's Activity panel, the teammate dialog, the
  // command palette, and New teammate from Home.
  const press = (label) => drive.evaluate(`(async () => {
    const button = [...document.querySelectorAll('button')].find((b) => b.textContent.trim() === ${JSON.stringify(label)})
    if (!button) return 'no ' + ${JSON.stringify(label)} + ' button'
    button.click()
    await new Promise((r) => setTimeout(r, 1100))
    return (document.querySelector('.lc-screen, [role=dialog], main')?.innerText ?? '').replace(/\\s+/g, ' ').slice(0, 160)
  })()`)
  const escape = async () => {
    await drive.evaluate(`document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))`)
    await sleep(500)
  }
  await drive.capture('1440 Routines', () => press('Routines'))
  await drive.capture('1440 Wren, with Activity open', async () => {
    await openConversation('Wren')
    return press('Activity')
  })
  await drive.capture('1440 Edit teammate (Wren)', async () => {
    await screen('2')
    return drive.evaluate(`(async () => {
      ;[...document.querySelectorAll('.lc-rostercard')].find((card) => card.querySelector('.lc-rostercard__name')?.textContent.trim() === 'Wren')?.querySelector('.lc-rostercard__edit')?.click()
      await new Promise((r) => setTimeout(r, 1100))
      return document.querySelector('[role=dialog]')?.innerText.replace(/\\s+/g, ' ').slice(0, 160) ?? 'no dialog'
    })()`)
  })
  await escape()
  await drive.capture('1440 the command palette (Ctrl K)', () => drive.evaluate(`(async () => {
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', ctrlKey: true, bubbles: true }))
    await new Promise((r) => setTimeout(r, 900))
    return document.querySelector('[role=dialog]')?.innerText.replace(/\\s+/g, ' ').slice(0, 160) ?? 'no palette'
  })()`))
  await escape()
  await drive.capture('1440 New teammate, from Home', async () => {
    await home()
    return press('New teammate')
  })
  await escape()
  await drive.resize(1280, 800)
  await sleep(1200)
  await drive.capture('1280 home', home)
  await drive.capture('1280 Iris', () => openConversation('Iris'))
  await drive.resize(1920, 1080)
  await sleep(1200)
  await drive.capture('1920 home', home)
  await drive.capture('1920 Wren', () => openConversation('Wren'))
  // The two ends of the cover's growth (coverGrowFor): a big screen with room
  // to spare, and a compact window with none, where it must be as it was.
  await drive.resize(2560, 1440)
  await sleep(1200)
  await drive.capture('2560 home', home)
  await drive.resize(1120, 720)
  await sleep(1200)
  await drive.capture('1120 home', home)
} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'whatever pnpm build last wrote to out/'}. Three teammates (Wren on Codex: code; Penny on OpenCode: money; Iris on OpenCode: design), one finished conversation each, three memories; every main screen at 1440x900, then 1280x800 and 1920x1080. Illustrative conversations in the runtimes' own record shapes; nothing sent.` })
}
