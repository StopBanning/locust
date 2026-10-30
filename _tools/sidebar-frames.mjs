// The sidebar, loaded the way a real person's would be, at three widths.
//
//   node _tools/sidebar-frames.mjs
//
// For the design agent. Colin, 2026-09-14: "outside of teammates our sidebar
// is kind of a disaster", and 2026-09-15, on my own attempt at redrawing it:
// "honestly your design is cooked brother, maybe have a prompt for the ui
// design agent".
//
// Every sidebar frame this repo has captured so far was taken against a
// profile with four teammates and almost no work in it, which is the one
// state where the current structure looks fine. The complaint is about what
// happens when it fills up, so this seeds a ledger first: fourteen
// conversations across four teammates and one nobody owns, two rooms, over
// four days. That is a light week, not a stress test.
//
// Nothing here spends. No runtime is ever started -- the missions are written
// straight into the ledger as finished work, which is what the sidebar reads
// at boot anyway.

import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import { localPath, FREE_ROUTE, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const OUT = localPath(new URL('../docs/design-frames-2026-09-15/', import.meta.url))
const PS = localPath(new URL('./window-frame.ps1', import.meta.url))

/** Mirrors `workspaceIdFor` in the main process; a mission is scoped by it. */
const workspaceIdFor = (path) => `ws_${createHash('sha256').update(path, 'utf8').digest('hex').slice(0, 32)}`

const workspace = await scratchRepository('locust-sidebar-ws-')
const workspaceId = workspaceIdFor(workspace)

const at = '2026-09-11T05:00:00.000Z'
const team = [
  { teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: at, route: FREE_ROUTE },
  { teammateId: 'tm_atlas', name: 'Atlas', hue: 'blue', role: 'Research & Briefs', createdAt: at, route: FREE_ROUTE },
  { teammateId: 'tm_juno', name: 'Juno', hue: 'violet', role: 'Docs & QA', createdAt: at, route: FREE_ROUTE },
  { teammateId: 'tm_sable', name: 'Sable', hue: 'clay', role: 'Data & Reporting', createdAt: at, route: FREE_ROUTE }
]

/*
 * Real-shaped prompts, deliberately varied in length.
 *
 * A sidebar's hardest problem is the long title, and a fixture of tidy
 * four-word prompts is a fixture that cannot show it. `owner: null` is a
 * conversation nobody owns, which today lands in a different section from
 * every other one -- the double-listing the brief is about.
 */
const WORK = [
  ['tm_wren', 'Fix the rail menu so both items can be clicked', '2026-09-15T02:10:00.000Z'],
  ['tm_wren', 'Move the composer cost token out of the control row', '2026-09-15T01:05:00.000Z'],
  ['tm_juno', 'Reorganise Settings into five named areas', '2026-09-14T23:40:00.000Z'],
  [null, 'why is the boot screen showing up in the middle of a session', '2026-09-14T22:15:00.000Z'],
  ['tm_atlas', 'Read the Grok audit and tell me which findings are real', '2026-09-14T19:30:00.000Z'],
  ['tm_sable', 'Pull the last 30 days of runtime discovery timings into a table', '2026-09-14T16:00:00.000Z'],
  ['tm_wren', 'Elapsed counter restarts when the step changes', '2026-09-14T14:20:00.000Z'],
  ['tm_juno', 'Draft the changelog entry for 0.134.0', '2026-09-14T11:45:00.000Z'],
  ['tm_atlas', 'Summarise what changed in the three placements ruling', '2026-09-13T20:05:00.000Z'],
  ['tm_wren', 'Stagger runtime discovery so a slow probe does not block the rest', '2026-09-13T17:30:00.000Z'],
  [null, 'quick one — what does LOCUST.md actually get used for', '2026-09-13T15:10:00.000Z'],
  ['tm_sable', 'Chart the release cadence since 0.100.0', '2026-09-12T22:00:00.000Z'],
  ['tm_juno', 'Check every empty state says what to do next', '2026-09-12T18:25:00.000Z'],
  ['tm_atlas', 'Compare how Cursor and Claude Code report token spend', '2026-09-12T09:15:00.000Z']
]

const SCHEMA = 15
const line = (value) => JSON.stringify(value) + '\n'

/** One finished mission as the ledger writes it: a header, then its events. */
const ledgerFile = (missionId, prompt, createdAt) => {
  const runId = `run_${missionId.slice(2)}`
  const metadata = {
    missionId,
    runId,
    prompt,
    runtime: 'opencode',
    model: 'account-default',
    requestedRouteId: 'opencode',
    resolvedRouteId: 'opencode-account:default',
    cliVersion: '1.18.27',
    workspaceId,
    sandbox: 'read-only',
    executionPolicyVersion: 1,
    createdAt
  }
  const header = { schemaVersion: SCHEMA, recordType: 'mission.created', ledgerSequence: 1, occurredAt: createdAt, metadata }
  const events = [
    { type: 'run.started', payload: {} },
    { type: 'message.delta', payload: { itemId: 'answer_1', operation: 'append', text: 'Done.', final: true } },
    { type: 'run.completed', payload: { status: 'completed' } }
  ].map((event, index) => ({
    schemaVersion: SCHEMA,
    recordType: 'mission.event',
    ledgerSequence: index + 2,
    occurredAt: createdAt,
    event: { id: `event_${missionId}_${String(index)}`, runId, missionId, sequence: index + 1, occurredAt: createdAt, sourceAdapter: 'opencode', ...event }
  }))
  return line(header) + events.map(line).join('')
}

const files = {}
const missionOwners = {}
WORK.forEach(([owner, prompt, createdAt], index) => {
  const missionId = `m_frame${String(index).padStart(2, '0')}`
  files[`mission-ledger/${missionId}.jsonl`] = ledgerFile(missionId, prompt, createdAt)
  if (owner !== null) missionOwners[missionId] = owner
})

files['rooms.json'] = {
  schemaVersion: 1,
  rooms: [
    { roomId: 'room_ship', name: 'Ship review', teammateIds: ['tm_wren', 'tm_juno'], posts: [], createdAt: at },
    { roomId: 'room_beta', name: 'Beta readiness', teammateIds: ['tm_wren', 'tm_atlas', 'tm_sable'], posts: [], createdAt: at }
  ]
}

const drive = await startDrive({
  name: 'sidebar-frames',
  port: 9288,
  workspace,
  files,
  seed: { schemaVersion: 1, teammates: team, missionOwners, settings: { swarm: false, relay: true, memoryMode: 'off' } }
})

await mkdir(OUT, { recursive: true })
const notes = []

/** Resize the REAL window and report what Windows actually gave back. */
const sizeTo = (width, height) => {
  const args = ['-NoProfile', '-File', PS, '-ProcessId', String(drive.pid), '-Probe']
  if (width === 'max') args.push('-Maximize')
  else args.push('-Width', String(width), '-Height', String(height))
  const ran = spawnSync('powershell', args, { encoding: 'utf8' })
  const reported = /(\d+)x(\d+)/.exec(ran.stdout ?? '')
  return reported === null ? 'unreported' : `${reported[1]}x${reported[2]}`
}

const shoot = async (file, sized, note) => {
  await sleep(900)
  const shot = await drive.send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false })
  const data = shot.result?.data
  if (typeof data !== 'string') throw new Error(`no screenshot for ${file}`)
  await writeFile(join(OUT, file), Buffer.from(data, 'base64'))
  say(`${file} at ${sized}`)
  notes.push({ file, window: sized, note })
}

try {
  await drive.ready()

  /*
   * The premise, checked before anything is photographed.
   *
   * A frame of an EMPTY sidebar captioned "the sidebar, full" would be worse
   * than no frame at all -- it is the exact failure the drive library's own
   * seed check was written for, one width down. If the ledger did not land
   * (a workspaceId that does not match the folder would do it), say so and
   * stop rather than hand the design agent a picture of the wrong thing.
   */
  await drive.evaluate(`(() => { for (const fold of document.querySelectorAll('.lc-sectionlabel--fold:not(.is-open)')) fold.click(); return true })()`)
  await sleep(600)
  say('sections: ' + await drive.evaluate(`JSON.stringify([...document.querySelectorAll('.lc-sectionlabel')].map((el) => (el.textContent || '').trim()))`))
  /*
   * Counted by TITLE, not by row.
   *
   * The first version of this check counted `.lc-row` inside the sidebar and
   * got 10 for 14 conversations, then stopped -- reporting a seeding failure
   * that had not happened. All fourteen were there: twelve as
   * `.lc-teammate__missionrow` nested under their owner and two in the
   * Missions section, which is the very double-listing this brief is about.
   * A premise check that does not know the structure it is checking will
   * measure the structure instead of the premise.
   */
  const seen = JSON.parse(await drive.evaluate(`(() => {
    const text = document.querySelector('.lc-sidebar__scroll')?.textContent ?? ''
    return JSON.stringify(${JSON.stringify(WORK.map(([, title]) => title))}.filter((title) => text.includes(title.slice(0, 16))))
  })()`))
  say('titles: ' + await drive.evaluate(`JSON.stringify([...document.querySelectorAll('.lc-conv__title, .lc-teammate__title')].map((el) => (el.textContent || '').trim()).slice(0, 6))`))
  say(`workspaceId ${workspaceId}`)
  say(`conversations on screen: ${String(seen.length)} of ${String(WORK.length)}`)
  if (seen.length < WORK.length) {
    const missing = WORK.map(([, title]) => title).filter((title) => !seen.includes(title))
    say(`the seeded work did not reach the sidebar; missing: ${missing.join(' | ')}`)
    process.exit(1)
  }

  let sized = sizeTo(1477, 900)
  await shoot('01-sidebar-1477-loaded.png', sized, 'the default width, a light week of work in it')

  // Everything open, so the design agent can see the full inventory at once.
  await drive.evaluate(`(() => {
    for (const fold of document.querySelectorAll('.lc-sectionlabel--fold:not(.is-open)')) fold.click()
    return true
  })()`)
  await shoot('02-sidebar-1477-all-sections-open.png', sized, 'every section expanded')

  sized = sizeTo(1120, 720)
  await shoot('03-sidebar-1120-rail.png', sized, 'the minimum width: the sidebar is a 64px rail')

  sized = sizeTo('max', 0)
  await shoot('04-sidebar-maximised.png', sized, 'maximised')

  sized = sizeTo(1477, 900)
  await sleep(700)

  const measured = await drive.evaluate(`(() => {
    const box = (sel) => { const el = document.querySelector(sel); if (!el) return null; const r = el.getBoundingClientRect(); return { w: Math.round(r.width), h: Math.round(r.height) } }
    const sections = [...document.querySelectorAll('.lc-sectionlabel')].map((el) => (el.textContent || '').trim())
    return JSON.stringify({
      sidebar: box('.lc-sidebar'),
      scroll: box('.lc-sidebar__scroll'),
      sections,
      rowsDrawn: document.querySelectorAll('.lc-conv, .lc-sidebar__scroll .lc-row').length,
      // How much of the column is spent BEFORE the first conversation, and
      // how much of a conversation's title survives the indent.
      firstConversationTop: (() => { const el = document.querySelector('.lc-conv, .lc-teammate__missionrow'); return el === null ? null : Math.round(el.getBoundingClientRect().top) })(),
      scrollTop: Math.round(document.querySelector('.lc-sidebar__scroll')?.getBoundingClientRect().top ?? 0),
      titleWidth: (() => { const el = document.querySelector('.lc-conv__title, .lc-teammate__title'); return el === null ? null : Math.round(el.getBoundingClientRect().width) })(),
      titlesShown: [...document.querySelectorAll('.lc-conv__title, .lc-teammate__title')].map((el) => (el.textContent || '').trim()),
      // Conversations actually on screen without scrolling, at this width.
      visible: (() => {
        const scroll = document.querySelector('.lc-sidebar__scroll')
        if (scroll === null) return null
        const box = scroll.getBoundingClientRect()
        return [...document.querySelectorAll('.lc-conv, .lc-teammate__missionrow')]
          .filter((el) => { const r = el.getBoundingClientRect(); return r.top >= box.top && r.bottom <= box.bottom }).length
      })(),
      // How far past the viewport the list runs: the scrolling cost of the
      // current structure, in pixels, at this width.
      scrollHeight: document.querySelector('.lc-sidebar__scroll')?.scrollHeight ?? null
    })
  })()`)
  say(`measured: ${measured}`)

  await writeFile(join(OUT, 'frames.json'), JSON.stringify({ capturedAt: new Date().toISOString(), workspaceId, conversations: WORK.length, frames: notes, measured: JSON.parse(measured) }, null, 2), 'utf8')
  say('captured')
} finally {
  await drive.finish({ intro: 'sidebar-frames', last: true }).catch(() => undefined)
}
