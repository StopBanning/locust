// A DIAGRAM IN A REPLY, DRAWN (0.770), the way a person meets it: a finished conversation open whose reply holds
// Mermaid -- a flowchart, a sequence of calls, a pie chart, a state diagram, a wide pipeline, and one with a mistake
// in it. Each is drawn as a picture in its frame, named by its kind, its text a tab away; the wide one scrolls inside
// its frame and not the page; the one with a mistake stays text, with Mermaid's reason under it. The pictures carry
// their own typeface and nothing that runs.
//
//   node _tools/drive-a-diagram-is-drawn.mjs [--packaged <exe>]
//
// Spends nothing: the conversation is seeded into the drive's own ledger, as a finished Codex turn's events.

import { createHash } from 'node:crypto'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const root = localPath(new URL('..', import.meta.url))
const adapters = await import(pathToFileURL(join(root, 'packages', 'runtime-adapters', 'dist', 'index.js')).href)
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)

const workspace = await scratchRepository('locust-drive-diagram-ws-')
const profilePath = await mkdtemp(join(tmpdir(), 'locust-drive-diagram-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const WREN = { teammateId: 'tm_aaaaaaaaaaaaaaaaaaaaaaa1', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-08-10T09:00:00.000Z' }

const fence = (...rows) => ['```mermaid', ...rows, '```'].join('\n')
const ANSWER = [
  'Here is how a change reaches production:',
  '',
  fence(
    'flowchart TD',
    '  accTitle: How a change reaches production',
    '  push[Push to main] --> ci{CI passes?}',
    '  ci -- yes --> staging[Deploy to staging]',
    '  ci -- no --> fix[Fix and push again]',
    '  fix --> push',
    '  staging --> smoke[Smoke tests]',
    '  subgraph release [Release]',
    '    smoke --> prod[(Production)]',
    '  end'
  ),
  '',
  'And what happens when you sign in:',
  '',
  fence(
    'sequenceDiagram',
    '  participant You',
    '  participant Locust',
    '  participant GitHub',
    '  You->>Locust: Sign in',
    '  Locust->>GitHub: Ask for a code',
    '  GitHub-->>Locust: Code and link',
    '  Note over You,GitHub: You approve it in your browser',
    '  Locust-->>You: Signed in'
  ),
  '',
  'Where the time went last week:',
  '',
  fence('pie title Time last week', '  "Reviews" : 34', '  "New work" : 41', '  "Fixes" : 17', '  "Meetings" : 8'),
  '',
  'A task moves through these states:',
  '',
  fence('stateDiagram-v2', '  [*] --> Waiting', '  Waiting --> Working: picked up', '  Working --> Done: settled', '  Working --> Waiting: handed back', '  Done --> [*]'),
  '',
  'The whole pipeline, end to end:',
  '',
  fence('flowchart LR', '  a[Write the change] --> b[Run the unit tests] --> c[Build the installer] --> d[Sign nothing] --> e[Smoke the update] --> f[Publish the release] --> g[Attach the Mac build] --> h[Tell the people using it]'),
  '',
  'This one has a mistake in it:',
  '',
  fence('flowchart TD', '  A[Start] -->'),
  '',
  'That is all of it.'
].join('\n')
const MISSION = { missionId: 'mission_30000000-0000-4000-8000-000000000770', runId: 'run_300770', prompt: 'Draw me how our releases work' }

{
  const at = new Date(Date.now() - 300_000).toISOString()
  await ledger.createMission({
    missionId: MISSION.missionId, runId: MISSION.runId, prompt: MISSION.prompt,
    runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: null,
    workspaceId, sandbox: 'workspace-write', executionPolicyVersion: 1, createdAt: at
  })
  let tick = 0
  const normalizer = adapters.createCodexEventNormalizer({ runId: MISSION.runId, missionId: MISSION.missionId, requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: '0.162.1', now: () => new Date(Date.parse(at) + (tick++) * 1000) })
  const records = [
    { type: 'thread.started', thread_id: `thread_${MISSION.runId}` },
    { type: 'turn.started' },
    { type: 'item.completed', item: { id: 'answer', type: 'agent_message', text: ANSWER } },
    { type: 'turn.completed', usage: { input_tokens: 2000, cached_input_tokens: 0, output_tokens: 400 } }
  ]
  const events = [
    ...records.flatMap((record, i) => normalizer.accept({ sequence: i + 1, raw: JSON.stringify(record) })),
    ...normalizer.finish({ exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: records.length, cancelled: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, startedAt: at, finishedAt: new Date(Date.parse(at) + 12_000).toISOString() })
  ]
  await ledger.appendEvents(MISSION.missionId, events)
}
say('seeded one finished conversation whose reply holds six Mermaid diagrams, one with a mistake')

const drive = await startDrive({
  name: 'a-diagram-is-drawn', port: 9656, workspace, profilePath, sendsNothing: true,
  outPath: join(recordRoot('a-diagram-is-drawn-2026-10-13'), packaged === undefined ? 'local' : 'packaged'),
  ...(packaged === undefined ? {} : { packaged }),
  seed: { schemaVersion: 1, teammates: [WREN], missionOwners: { [MISSION.missionId]: WREN.teammateId }, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false } }
})

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 500)}`}`)
}
const json = async (expression) => JSON.parse(String(await drive.evaluate(`JSON.stringify(${expression})`)))
const pictures = () => json(`[...document.querySelectorAll('.lc-replydiagram:not(.lc-replydiagram--text)')].map((figure) => {
  const img = figure.querySelector('img.lc-replydiagram__picture')
  const stage = figure.querySelector('.lc-replydiagram__stage')
  const svg = img === null ? '' : decodeURIComponent(img.getAttribute('src').replace(/^data:image\\/svg\\+xml;charset=utf-8,/, ''))
  return {
    kind: (figure.querySelector('.lc-replydiagram__kind')?.textContent ?? '').trim(),
    alt: img?.getAttribute('alt') ?? '',
    data: (img?.getAttribute('src') ?? '').startsWith('data:image/svg+xml'),
    complete: img !== null && img.complete && img.naturalWidth > 0,
    width: img?.naturalWidth ?? 0,
    shown: img === null ? 0 : Math.round(img.getBoundingClientRect().width),
    scrolls: stage === null ? false : stage.scrollWidth > stage.clientWidth + 1,
    typeface: svg.includes('@font-face') && svg.includes("font-family:'Figtree'"),
    runs: /<script|\\son[a-z]+\\s*=|javascript:/i.test(svg)
  }
})`)

try {
  await drive.ready()
  await drive.resize(1215, 860)
  const opened = String(await drive.evaluate(`(() => {
    const row = [...document.querySelectorAll('button.lc-conv')].find((b) => b.innerText.includes('releases work'))
    if (!row) return 'no conversation row'
    row.click()
    return 'opened'
  })()`))
  say(`  conversation: ${opened}`)
  let drawn = []
  for (let second = 0; second < 40; second += 1) {
    await sleep(1000)
    drawn = await pictures()
    if (drawn.length === 5 && drawn.every((one) => one.complete)) break
  }
  say(`  pictures: ${JSON.stringify(drawn.map(({ kind, width, shown, scrolls }) => ({ kind, width, shown, scrolls })))}`)
  check('five diagrams are drawn, each a picture shown from data', drawn.length === 5 && drawn.every((one) => one.complete && one.data), JSON.stringify(drawn))
  check('each is named by its kind', JSON.stringify(drawn.map((one) => one.kind)) === JSON.stringify(['Flowchart', 'Sequence diagram', 'Pie chart', 'State diagram', 'Flowchart']), JSON.stringify(drawn.map((one) => one.kind)))
  check("the picture's words are the title the diagram gave itself", drawn[0]?.alt === 'How a change reaches production', drawn[0]?.alt)
  check('each picture carries its typeface and nothing that runs', drawn.length === 5 && drawn.every((one) => one.typeface && !one.runs), JSON.stringify(drawn.map(({ typeface, runs }) => ({ typeface, runs }))))
  check('each is shown at its own size; only the wide pipeline scrolls, inside its frame', drawn.length === 5 && drawn.every((one) => one.shown === one.width) && drawn[4]?.scrolls === true && drawn.slice(0, 4).every((one) => !one.scrolls), JSON.stringify(drawn.map(({ width, shown, scrolls }) => ({ width, shown, scrolls }))))
  const page = await json(`({ wide: document.scrollingElement.scrollWidth > document.scrollingElement.clientWidth, column: (() => { const c = document.querySelector('.lc-thread__column'); return c === null ? false : c.scrollWidth > c.clientWidth + 1 })() })`)
  check('the conversation itself never scrolls sideways', !page.wide && !page.column, JSON.stringify(page))

  const mistake = String(await drive.evaluate(`(document.querySelector('.lc-replydiagram--text')?.innerText ?? '').replace(/\\s+/g, ' ').trim()`))
  check("the one with a mistake stays text, with Mermaid's reason under it", /flowchart TD A\[Start\] -->/.test(mistake) && /Not drawn as a diagram \((Parse|Lexical) error on line \d+\)\. Its text is above, as written\./.test(mistake), mistake)

  await drive.evaluate(`document.querySelector('.lc-replydiagram')?.scrollIntoView({ block: 'start' })`)
  await sleep(400)
  await drive.capture('A flowchart in a reply, drawn', () => drive.evaluate(`(document.querySelector('.lc-replydiagram')?.innerText ?? '').replace(/\\s+/g, ' ')`))
  const code = String(await drive.evaluate(`(() => {
    const tab = [...document.querySelectorAll('.lc-replydiagram [role="tab"]')].find((one) => one.textContent.trim() === 'Code')
    if (!tab) return 'no code tab'
    tab.click()
    return 'clicked'
  })()`))
  await sleep(300)
  const text = String(await drive.evaluate(`(document.querySelector('.lc-replydiagram')?.innerText ?? '').replace(/\\s+/g, ' ').trim()`))
  check('the Code tab shows its Mermaid text', code === 'clicked' && /flowchart TD accTitle: How a change reaches production push\[Push to main\] --> ci\{CI passes\?\}/.test(text), text)
  await drive.capture('The same diagram, its text', () => text)
  await drive.evaluate(`[...document.querySelectorAll('.lc-replydiagram [role="tab"]')].find((one) => one.textContent.trim() === 'Diagram')?.click()`)
  await sleep(300)
  for (const [index, name] of [[1, 'A sequence of calls, drawn'], [2, 'A pie chart, drawn'], [3, 'A state diagram, drawn'], [4, 'A wide pipeline, scrolling in its frame']]) {
    await drive.evaluate(`document.querySelectorAll('.lc-replydiagram')[${String(index)}]?.scrollIntoView({ block: 'center' })`)
    await sleep(300)
    await drive.capture(name, () => drive.evaluate(`(document.querySelectorAll('.lc-replydiagram')[${String(index)}]?.innerText ?? '').replace(/\\s+/g, ' ')`))
  }
  await drive.evaluate(`document.querySelector('.lc-replydiagram--text')?.scrollIntoView({ block: 'center' })`)
  await sleep(300)
  await drive.capture('A diagram with a mistake, as text', () => mistake)

  // The smallest window Locust allows: every picture still at its own size, the page still not sideways.
  await drive.resize(1120, 720)
  await sleep(600)
  const small = await pictures()
  const smallPage = await json(`({ wide: document.scrollingElement.scrollWidth > document.scrollingElement.clientWidth })`)
  check('at the smallest window, the pictures keep their size and the page stays put', small.length === 5 && small.every((one) => one.shown === one.width) && !smallPage.wide, JSON.stringify({ small: small.map(({ shown, width, scrolls }) => ({ shown, width, scrolls })), smallPage }))
  await drive.evaluate(`document.querySelector('.lc-replydiagram')?.scrollIntoView({ block: 'start' })`)
  await sleep(300)
  await drive.capture('A flowchart, at the smallest window', () => '')
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. One seeded reply with six Mermaid blocks, one with a mistake.`, extra: `Checks failed: ${String(failures)}` })
  process.exit(failures === 0 ? 0 : 1)
}
