// The usage line on Home and the Usage dialog, to be looked at (0.714).
//
//   node _tools/look-usage.mjs [--packaged <exe>] [--sizes 1209x770,1440x900,1920x1080,1120x720]
//
// Twelve weeks of turns over every kind of receipt the app reads: Claude Code
// on a plan (cache-heavy, billed to the subscription) and on an API key
// (priced), Codex on its account (cached input counted inside its input),
// OpenCode on a free model, Copilot's premium requests, Antigravity with
// tokens and nothing else. The everyday team, so Home is the everyday Home,
// and the limit readings Claude Code and Codex leave. Written with the
// mission store's own writer, as the everyday profile is. Sends nothing.

import { mkdtemp, writeFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { TEAM } from './everyday-ledger.mjs'
import { localPath, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const sizes = (arg('--sizes') ?? '1209x770,1440x900,1920x1080,1120x720').split(',').map((size) => size.split('x').map(Number))

const root = localPath(new URL('..', import.meta.url))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-look-usage-ws-')
const profilePath = await mkdtemp(join(tmpdir(), 'locust-look-usage-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })

// A small fixed generator, so every run of this draws the same twelve weeks.
let seed = 20261009
const random = () => {
  seed = (seed * 1103515245 + 12345) % 2147483648
  return seed / 2147483648
}

const ROUTES = [
  { weight: 34, runtime: 'claude', model: 'opus', resolved: 'claude-opus-4-6', receipt: () => ({ inputTokens: 40 + Math.round(random() * 900), cacheReadTokens: 30_000 + Math.round(random() * 160_000), cacheWriteTokens: 2_000 + Math.round(random() * 9_000), outputTokens: 400 + Math.round(random() * 3_000), usd: 0.4 + random() * 2, billing: 'subscription' }) },
  { weight: 18, runtime: 'codex', model: 'gpt-6-luna', receipt: () => { const input = 20_000 + Math.round(random() * 160_000); return { inputTokens: input, cacheReadTokens: Math.round(input * (0.5 + random() * 0.4)), cacheWriteTokens: 0, outputTokens: 300 + Math.round(random() * 2_500) } } },
  { weight: 9, runtime: 'codex', model: 'gpt-6-sol', receipt: () => { const input = 40_000 + Math.round(random() * 220_000); return { inputTokens: input, cacheReadTokens: Math.round(input * (0.6 + random() * 0.3)), cacheWriteTokens: 0, outputTokens: 800 + Math.round(random() * 4_000) } } },
  { weight: 14, runtime: 'opencode', model: 'opencode/space-bunny-free', receipt: () => ({ inputTokens: 3_000 + Math.round(random() * 14_000), outputTokens: 200 + Math.round(random() * 1_500), usd: 0 }) },
  { weight: 5, runtime: 'claude', model: 'haiku', resolved: 'claude-haiku-4-5', receipt: () => ({ inputTokens: 1_200 + Math.round(random() * 4_000), cacheReadTokens: Math.round(random() * 9_000), cacheWriteTokens: Math.round(random() * 2_000), outputTokens: 150 + Math.round(random() * 600), usd: 0.004 + random() * 0.03 }) },
  { weight: 4, runtime: 'copilot', model: 'auto', receipt: () => ({ premiumRequests: 1 }) },
  { weight: 3, runtime: 'antigravity', model: 'gemini-3.8-flash', receipt: () => ({ inputTokens: 4_000 + Math.round(random() * 20_000), outputTokens: 300 + Math.round(random() * 2_000) }) }
]
const pick = () => {
  const total = ROUTES.reduce((sum, route) => sum + route.weight, 0)
  let at = random() * total
  for (const route of ROUTES) {
    at -= route.weight
    if (at <= 0) return route
  }
  return ROUTES[0]
}
const ASKS = ['Fix the flaky checkout test', 'Summarize the support tickets from this week', 'Draft the release notes', 'Why is the dashboard slow on Mondays?', 'Rename the billing module', 'Plan the onboarding emails', 'Reconcile the invoices against the bank export', 'Write tests for the export', 'Compare three CRMs for a small team', 'Tidy the README']

const now = Date.now()
const day = 86_400_000
const missionOwners = {}
let conversation = 0
let made = 0
for (let back = 83; back >= 0; back -= 1) {
  // Fewer weekend days, a quiet stretch three weeks ago, a busier last fortnight.
  const date = new Date(now - back * day)
  const weekend = date.getDay() === 0 || date.getDay() === 6
  if ((weekend && random() < 0.65) || (back >= 19 && back <= 23) || random() < 0.12) continue
  const conversations = 1 + Math.floor(random() * (back < 14 ? 4 : 2.4))
  for (let c = 0; c < conversations; c += 1) {
    const route = pick()
    const turns = 1 + Math.floor(random() * 3.2)
    const owner = TEAM[Math.floor(random() * TEAM.length)]
    let previous
    const startHour = 9 + Math.floor(random() * 9)
    for (let t = 0; t < turns; t += 1) {
      const missionId = `mission_5e000000-0000-4000-8000-${String(conversation).padStart(4, '0')}${String(t).padStart(8, '0')}`
      const runId = `run_5e${String(conversation).padStart(4, '0')}${String(t)}`
      const startedAt = new Date(new Date(date).setHours(startHour, 7 + t * 9 + Math.floor(random() * 6), 0, 0)).toISOString()
      if (Date.parse(startedAt) > now) continue
      const endedAt = new Date(Date.parse(startedAt) + (25 + Math.floor(random() * 140)) * 1000).toISOString()
      await ledger.createMission({
        missionId, runId, prompt: t === 0 ? ASKS[conversation % ASKS.length] : 'Keep going',
        runtime: route.runtime, model: route.model, requestedRouteId: route.runtime, resolvedRouteId: `${route.runtime}-account:default`, cliVersion: null,
        workspaceId, sandbox: 'workspace-write', mode: 'accept-edits', executionPolicyVersion: 1, createdAt: startedAt,
        ...(previous === undefined ? {} : { continuesFrom: { missionId: previous, checkpointEpoch: 1, reason: 'follow-up' } })
      })
      const base = { runId, missionId, sourceAdapter: route.runtime, requestedRouteId: route.runtime, resolvedRouteId: `${route.runtime}-account:default` }
      const thread = `thread_${missionId}`
      const process = { exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 2, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt, finishedAt: endedAt }
      await ledger.appendEvents(missionId, [
        { ...base, id: `${runId}:1`, sequence: 1, occurredAt: startedAt, type: 'run.started', payload: { runtimeThreadId: thread, evidence: { redacted: true } } },
        { ...base, id: `${runId}:2`, sequence: 2, occurredAt: endedAt, type: 'run.completed', payload: { runtimeThreadId: thread, usage: route.receipt(), ...(route.resolved === undefined ? {} : { resolvedModel: route.resolved }), process } }
      ])
      missionOwners[missionId] = owner.teammateId
      previous = missionId
      made += 1
    }
    conversation += 1
  }
}
say(`seeded ${String(made)} turns in ${String(conversation)} conversations over twelve weeks`)

const soon = (hours) => new Date(now + hours * 3_600_000).toISOString()
const readings = {
  claude: `5-hour window 64% used · resets ${soon(2.2)} · 7-day window 31% used · resets ${soon(70)} · from a run at ${new Date(now - 40 * 60_000).toISOString()}`,
  codex: `weekly window 21% used · resets ${soon(101)} · 5-hour window 84% used · resets ${soon(1.4)} · from a run at ${new Date(now - 3 * 3_600_000).toISOString()}`
}
await writeFile(join(profilePath, 'usage-readings.json'), JSON.stringify(readings), 'utf8')

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: 'look-usage',
  port: 9875,
  workspace,
  profilePath,
  sendsNothing: true,
  outPath: join(recordRoot('look-usage-2026-10-09'), packaged === undefined ? 'local' : 'packaged'),
  seed: { schemaVersion: 1, teammates: TEAM, missionOwners, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false } }
})
/** A close-up of one element, at three times its size: the small things are where a look goes wrong. */
const zoom = async (name, selector, pad = 10) => {
  const box = JSON.parse(String(await drive.evaluate(`(() => { const r = document.querySelector(${JSON.stringify(selector)})?.getBoundingClientRect(); return JSON.stringify(r ? { x: r.x, y: r.y, width: r.width, height: r.height } : null) })()`)))
  if (box === null) return say(`zoom: no ${selector}`)
  const shot = await drive.send('Page.captureScreenshot', { format: 'png', clip: { x: Math.max(0, box.x - pad), y: Math.max(0, box.y - pad), width: box.width + pad * 2, height: box.height + pad * 2, scale: 3 } })
  if (shot?.result?.data) await writeFile(join(drive.out, `zoom-${name}.png`), Buffer.from(shot.result.data, 'base64'))
}

try {
  await drive.ready()
  for (const [width, height] of sizes) {
    await drive.resize(width, height)
    await sleep(1800)
    await drive.capture(`Home at ${String(width)}x${String(height)}`, () => drive.evaluate(`(() => {
      const line = document.querySelector('.lc-usageline')
      const page = document.scrollingElement
      // Colin's rule for Home (first-screen-fits.mjs): no scrollbar that goes to nothing.
      const scroller = [...document.querySelectorAll('main *, .lc-empty')].find((node) => node.scrollHeight > node.clientHeight + 1 && /auto|scroll/.test(getComputedStyle(node).overflowY)) ?? document.querySelector('.lc-empty')
      const over = scroller ? scroller.scrollHeight - scroller.clientHeight : -1
      if (line) line.style.display = 'none'
      const without = scroller ? scroller.scrollHeight - scroller.clientHeight : -1
      if (line) line.style.display = ''
      return JSON.stringify({ line: line?.innerText.replace(/\\s+/g, ' ').trim() ?? 'NO USAGE LINE', lineBox: line ? Math.round(line.getBoundingClientRect().height) : 0, scroller: scroller?.className ?? 'none', overflow: over, overflowWithoutLine: without })
    })()`))
    if (width === 1440) await zoom('home-line', '.lc-usageline')
    await drive.capture(`the dialog at ${String(width)}x${String(height)}`, () => drive.evaluate(`(async () => {
      document.querySelector('.lc-usageline__open')?.click()
      await new Promise((r) => setTimeout(r, 900))
      const dialog = document.querySelector('.lc-usage')
      if (!dialog) return 'NO DIALOG'
      const box = dialog.getBoundingClientRect()
      const body = dialog.querySelector('.lc-dialog__body')
      return JSON.stringify({ box: [Math.round(box.width), Math.round(box.height)], scrolls: body ? body.scrollHeight > body.clientHeight : false, rows: dialog.querySelectorAll('.lc-usage__models tbody tr').length, limits: dialog.querySelectorAll('.lc-usage__window').length, cells: dialog.querySelectorAll('.lc-heat__grid .lc-heat__cell').length })
    })()`))
    if (width === 1440) {
      await zoom('limits', '.lc-usage__limits')
      await zoom('activity', '.lc-usage__activity')
      await drive.capture('the dialog, all time', () => drive.evaluate(`(async () => {
        ;[...document.querySelectorAll('.lc-usage__range button')].find((b) => b.innerText.trim() === 'All time')?.click()
        await new Promise((r) => setTimeout(r, 900))
        return document.querySelector('.lc-usage__stats')?.innerText.replace(/\\s+/g, ' ') ?? ''
      })()`))
      await drive.capture('the dialog, scrolled to the models', () => drive.evaluate(`(async () => {
        document.querySelector('.lc-usage__models')?.scrollIntoView({ block: 'end' })
        await new Promise((r) => setTimeout(r, 500))
        return [...document.querySelectorAll('.lc-usage__models tbody tr')].map((r) => r.innerText.replace(/\\s+/g, ' ').trim()).join(' | ')
      })()`))
      await zoom('models', '.lc-usage__models')
    }
    await drive.evaluate(`(async () => { document.querySelector('.lc-usage .lc-dialog__close')?.click(); await new Promise((r) => setTimeout(r, 400)); return 1 })()`)
  }
  say('captured')
} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. Twelve weeks of seeded turns; the usage line on Home and the Usage dialog at each size.`, extra: '' })
}
