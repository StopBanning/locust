// Twelve frames for the design review, at real window sizes.
//
//   node _tools/design-frames.mjs
//
// Asked for on 2026-09-14: the whole window including chrome, PNG, no
// cropping and no scaling, at 1120x720 (the minimum, where things break),
// 1477 (the default) and maximised. Display scaling here is 100% on a
// 1920x1080 screen, so a pixel in these files is a pixel.
//
// The window is moved by Win32 -- a real OS resize, NOT by
// `Emulation.setDeviceMetricsOverride`, which changes the PAGE and leaves the
// OS window where it was -- what Astra rightly called "narrower evidence, not
// a native-window sizing pass". Every frame reports the size Windows ACTUALLY
// gave the window, because a request is not a measurement and a minimum size
// refuses silently.
//
// CAPTURE is CDP, not PrintWindow, and that was measured rather than assumed.
// PrintWindow draws the composited swarm-mark image at the wrong offset: in a
// 1477 frame it landed in the middle of the `effort - fixed` label and looked
// exactly like a layout collision. The DOM says otherwise -- the label ends
// at x=1198 and the mark starts at x=1215, a 17px gap -- and a CDP capture of
// the same state shows them cleanly apart. A frame that invents a defect is
// worse than no frame, so the window is sized natively and photographed by
// the renderer. The cost is the 8px Windows border, which carries no design.
//
// Free OpenCode only. Nothing here spends, so anything needing an approval
// is skipped and said so rather than substituted.

import { spawnSync } from 'node:child_process'
import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import { localPath, FREE_ROUTE, pickRouteScript, say, scratchRepository, sendAndWaitScript, sleep, startDrive, teammateRows } from './drive-lib.mjs'

const OUT = localPath(new URL('../docs/design-frames-2026-09-14/', import.meta.url))
const PS = localPath(new URL('./window-frame.ps1', import.meta.url))

const at = '2026-09-05T05:00:00.000Z'
const workspace = await scratchRepository('locust-frames-ws-')

const drive = await startDrive({
  name: 'design-frames',
  port: 9251,
  workspace,
  seed: {
    schemaVersion: 1,
    teammates: [
      { teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: at, route: FREE_ROUTE },
      { teammateId: 'tm_atlas', name: 'Atlas', hue: 'blue', role: 'Research & Briefs', createdAt: at, route: FREE_ROUTE },
      { teammateId: 'tm_juno', name: 'Juno', hue: 'violet', role: 'Docs & QA', createdAt: at, route: FREE_ROUTE },
      // A Claude route with nothing signed in: frame 6 asks for a signed-out
      // teammate, and that state has to be real to be worth photographing.
      { teammateId: 'tm_sable', name: 'Sable', hue: 'clay', role: 'Data & Reporting', createdAt: at, route: { runtime: 'claude', model: 'sonnet', mode: 'read-only' } }
    ],
    missionOwners: {},
    settings: { swarm: false, relay: true, relayHopCap: 4, memoryMode: 'off' }
  }
})

await mkdir(OUT, { recursive: true })
const notes = []
const skipped = []

const shoot = async (number, title, sized) => {
  await sleep(900)
  const file = `${String(number).padStart(2, '0')}-${title}.png`
  const shot = await drive.send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false })
  const data = shot.result?.data
  if (typeof data !== 'string') throw new Error(`no screenshot for frame ${String(number)}`)
  await writeFile(join(OUT, file), Buffer.from(data, 'base64'))
  say(`frame ${String(number)} ${title}: ${file} at ${sized}`)
  notes.push({ frame: number, title, file, window: sized })
  return file
}

/**
 * Resize the REAL window and report what Windows gave back.
 *
 * Returned verbatim so every frame carries the size it was actually taken
 * at, rather than the size that was asked for.
 */
let currentSize = 'unsized'
const size = async (spec) => {
  const args = ['-NoProfile', '-File', PS, '-ProcessId', String(drive.pid)]
  if (spec === 'max') args.push('-Maximize')
  else args.push('-Width', String(spec[0]), '-Height', String(spec[1]))
  args.push('-Probe')
  const done = spawnSync('powershell', args, { encoding: 'utf8' })
  currentSize = (done.stdout || done.stderr || '').trim()
  say(`sized: ${currentSize}`)
  await sleep(1200)
  return currentSize
}

const evaluate = (expression) => drive.evaluate(expression)
const openTeammate = (name) =>
  evaluate(`(() => {
    const row = ${teammateRows()}.find(r => new RegExp(${JSON.stringify(name)}).test(r.innerText))
    if (!row) return 'no row for ' + ${JSON.stringify(name)}
    row.click()
    return 'opened ' + ${JSON.stringify(name)}
  })()`)
/**
 * The composer's route, as the CHIP reads it.
 *
 * Seeding `route` on a teammate is not enough and this drive proved it: the
 * first run of this script seeded the free OpenCode route on all four, and
 * the header came back "Codex CLI - Account Default" with two turns already
 * spent against a paid account. A seed is a request; the chip is the fact.
 */
const routeChip = () =>
  evaluate(`(() => {
    const control = [...document.querySelectorAll('.lc-control')].find(b => b.getAttribute('aria-haspopup') === 'listbox')
    return control ? control.innerText.replace(/\\s+/g, ' ').trim() : 'no route control'
  })()`)

/** Put the composer on the free route and refuse to continue until it is. */
const forceFreeRoute = async () => {
  say(await evaluate(pickRouteScript({ group: '/opencode/i', search: 'free', row: '/free/i' })))
  await sleep(900)
  const chip = await routeChip()
  say(`route chip: ${chip}`)
  if (!/opencode/i.test(chip)) {
    throw new Error(`refusing to send: the composer is on "${chip}", not the free OpenCode route`)
  }
  return chip
}

/*
 * Never send without checking the chip first. Frames are not worth somebody
 * else's quota, and "I seeded it" is not evidence of where a run went.
 */
const ask = async (text, waitSeconds = 300) => {
  const chip = await routeChip()
  if (!/opencode/i.test(chip)) await forceFreeRoute()
  return evaluate(sendAndWaitScript(text, { waitSeconds }))
}
const activity = () =>
  evaluate(`(() => {
    const b = [...document.querySelectorAll('button')].find(x => /Activity/i.test(x.innerText))
    if (!b) return 'no Activity button'
    b.click()
    return 'toggled Activity'
  })()`)

const SMALL = [1120, 720]
const DEFAULT = [1477, 920]

try {
  await drive.ready()

  // ============================================================ 1120x720
  await size(SMALL)
  await openTeammate('Wren')
  await sleep(1200)
  await forceFreeRoute()

  say('two completed turns, so the thread is mid-conversation rather than new')
  say(await ask('In two short sentences, say what a task board is for. Do not use tools.'))
  say(await ask('Now say, in one sentence, when you would NOT use one. Do not use tools.'))

  await shoot(1, 'workroom-1120-inspector-closed', currentSize)
  say(await activity())
  await shoot(2, 'workroom-1120-inspector-open', currentSize)
  say(await activity())

  // Frame 3 -- a room with a live exchange. Rooms live on their own screen.
  say('staging a room')
  const room = await evaluate(`(async () => {
    const nav = [...document.querySelectorAll('button, a')].find(b => /^\\s*Rooms/i.test(b.innerText) || b.getAttribute('aria-label') === 'Rooms')
    if (nav) { nav.click(); await new Promise(r => setTimeout(r, 900)) }
    const make = [...document.querySelectorAll('button')].find(b => /New room|Create room|\\+ *Room/i.test(b.innerText))
    if (!make) return 'no room control; buttons: ' + [...document.querySelectorAll('button')].map(b => b.innerText.replace(/\\s+/g,' ').trim()).filter(Boolean).slice(0, 14).join(' | ')
    make.click()
    await new Promise(r => setTimeout(r, 1400))
    return 'room screen: ' + (document.body.innerText.replace(/\\s+/g, ' ').slice(0, 220))
  })()`)
  say(room)
  notes.push({ note: 'room staging', result: room })
  if (/no room control/.test(room)) {
    skipped.push({ frame: 3, why: 'could not reach a room from the harness; not substituted with a lookalike' })
  } else {
    await shoot(3, 'room-1120-exchange', currentSize)
  }

  // ============================================================ 1477 wide
  await size(DEFAULT)
  await openTeammate('Wren')
  await sleep(1200)

  say('more turns, so the thread is long enough to read as a timeline')
  say(await ask('Name three things a small team tracks on a board. One line each. Do not use tools.'))
  say(await ask('Which of those changes most often, and why? Two sentences. Do not use tools.'))

  await evaluate(`(() => { const t = document.querySelector('.lc-thread'); if (t) t.scrollTop = Math.floor(t.scrollHeight / 2); return 'scrolled to middle' })()`)
  await shoot(4, 'conversation-1477-scrolled-middle', currentSize)

  say('one live run, for the running state -- frames 5 and 6 both need it LIVE')
  await evaluate(sendAndWaitScript('Count slowly from 1 to 60, one number per line. Do not use tools.', { settle: false }))
  // Wait for the run to be visibly running rather than for a fixed delay: a
  // sleep long enough to be safe is also long enough to miss the end of a
  // fast reply, and a frame captioned "running" that is not is the exact
  // thing the design pass asked me not to send.
  say(await evaluate(`(async () => {
    for (let i = 0; i < 120; i += 1) {
      await new Promise(r => setTimeout(r, 250))
      if (document.querySelector('button[aria-label^="Stop the running"]')) return 'stop button is up: running'
    }
    return 'never saw the stop button'
  })()`))
  await sleep(2500)
  await evaluate(`(() => { const t = document.querySelector('.lc-thread'); if (t) t.scrollTop = t.scrollHeight; return 'bottom' })()`)
  const liveAt5 = await evaluate(`document.querySelector('button[aria-label^="Stop the running"]') ? 'live' : 'already settled'`)
  say(`frame 5 state: ${liveAt5}`)
  await shoot(5, 'conversation-1477-running', currentSize)
  notes.push({ note: 'frame 5 was captured while', result: liveAt5 })

  /*
   * Frame 6 -- the sidebar, four teammates. Called the single most useful
   * frame in the list, so what it does and does not show is stated exactly.
   *
   * Staged: Wren SELECTED and RUNNING (the turn above is still going), Atlas
   * and Juno IDLE. NOT staged: "waiting on you" needs an approval, which is
   * Codex-only and would spend; and "signed out" is not reachable here
   * because Claude Code IS signed in on this machine, so Sable's route is
   * genuinely available. Both are recorded as gaps rather than implied.
   */
  const liveAt6 = await evaluate(`document.querySelector('button[aria-label^="Stop the running"]') ? 'live' : 'already settled'`)
  say(`frame 6 state: ${liveAt6}`)
  await shoot(6, 'sidebar-1477-four-states', currentSize)
  notes.push({
    note: 'frame 6 coverage',
    result: `selected+running: Wren (${liveAt6}); idle: Atlas, Juno, Sable. NOT shown: "waiting on you" (needs an approval, Codex-only, would spend) and "signed out" (Claude Code is signed in on this machine, so no teammate here is honestly signed out).`
  })
  skipped.push({ frame: 6, why: 'two of the four asked-for states are absent: waiting-on-you and signed-out. The frame is real but it is not the full lime question.' })

  say(await evaluate(`(async () => {
    for (let i = 0; i < 240; i += 1) {
      await new Promise(r => setTimeout(r, 500))
      if (!document.querySelector('button[aria-label^="Stop the running"]')) return 'settled'
    }
    return 'still running'
  })()`))

  // Frame 7 -- a large diff, folded open.
  say(await ask('Create a file called inventory.md with a markdown table of 60 rows: an id, a name and a count. Use your tools to write it.', 420))
  // No fold to open: the diff card says "All 1 hunk shown" for a file this
  // size, so the frame just needs the diff in view. `.lc-fold` was invented
  // and the repo's own harness guard caught it.
  say(await evaluate(`(() => {
    const diff = [...document.querySelectorAll('.lc-diff')].pop()
    if (!diff) return 'no diff on screen'
    diff.scrollIntoView({ block: 'center' })
    return 'diff in view: ' + diff.innerText.replace(/\\s+/g, ' ').slice(0, 60)
  })()`))
  await shoot(7, 'diff-1477-fold-open', currentSize)

  // Frame 8 -- an approval card beside a receipt. NOT STAGED: approve-each
  // is Codex-only and this drive is free-route only, so there is no honest
  // way to produce one. Saying so rather than photographing something close.
  skipped.push({ frame: 8, why: 'an approval card needs approve-each, which is Codex-only and would spend; no lookalike substituted' })

  // ============================================================ maximised
  await size('max')
  await shoot(9, 'workroom-maximised', currentSize)

  say(await evaluate(`(async () => {
    const nav = [...document.querySelectorAll('button, a')].find(b => /Settings/i.test(b.innerText))
    if (!nav) return 'no Settings nav'
    nav.click()
    await new Promise(r => setTimeout(r, 1500))
    const runtimes = [...document.querySelectorAll('.lc-runtimerow, .lc-runtimecell')]
    if (runtimes.length > 0) runtimes[Math.min(2, runtimes.length - 1)].scrollIntoView({ block: 'center' })
    return 'settings: ' + document.body.innerText.replace(/\\s+/g, ' ').slice(0, 200)
  })()`))
  await shoot(10, 'settings-maximised-runtimes', currentSize)

  // Frame 11 -- a teammate with no missions.
  say(await openTeammate('Juno'))
  await sleep(1400)
  await shoot(11, 'teammate-no-missions-maximised', currentSize)

  // Frame 12 -- the command palette, empty query.
  // A REAL key event through the browser, not a synthetic one: Astra
  // measured that `.click()` does not reliably open newer menus and pointer
  // input does, and the same is true of keys.
  for (const type of ['keyDown', 'keyUp']) {
    await drive.send('Input.dispatchKeyEvent', {
      type, key: 'k', code: 'KeyK', windowsVirtualKeyCode: 75, modifiers: 2, text: type === 'keyDown' ? 'k' : undefined
    })
  }
  await sleep(1400)
  const palette = await evaluate(`document.querySelector('.lc-palette') ? 'palette open' : 'palette did not open'`)
  say(palette)
  if (palette === 'palette open') await shoot(12, 'palette-maximised-empty', currentSize)
  else skipped.push({ frame: 12, why: 'the palette did not open from a synthetic Ctrl+K; not substituted' })

  await writeFile(join(OUT, 'frames.json'), JSON.stringify({ notes, skipped }, null, 2), 'utf8')
  say('part one captured; see frames.json')
} finally {
  await writeFile(join(OUT, 'frames.json'), JSON.stringify({ notes, skipped }, null, 2), 'utf8').catch(() => undefined)
  await drive.finish({ intro: 'Design frames', last: true }).catch(() => undefined)
}
