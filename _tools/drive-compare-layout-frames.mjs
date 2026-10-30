// Frames of a finished compare in the app, at three window sizes (2026-10-05).
//
//   node _tools/drive-compare-layout-frames.mjs --columns 2|3 [--open] [--tag <name>] [--packaged <exe>]
//
// Colin, 2026-10-05, of a three-column Blind compare: "issues with text
// clipping through ... the whole layout might need polishing at some point."
// Seeds one comparison (blind unless --open) shaped like his -- one short
// answer, one long, one between (_tools/compare-layout-seed.mjs) -- opens it,
// and at 1200x780, 1000x680 and 1600x900 takes its top, its bottom, and
// measures what reads badly: text past its column's edge, a foot's numbers
// cut short, the composer's row overflowing, how far each column's last words
// sit above the tallest's. One comparison per launch: the compare screen hides
// the conversation list. Running columns need a live run; for those see
// _tools/look-compare-layout.mjs. Sends nothing.

import { createHash } from 'node:crypto'
import { mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { COLUMNS, PROMPT, columnEvents } from './compare-layout-seed.mjs'
import { localPath, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'
const count = Number(arg('--columns') ?? '3')
const blind = !process.argv.includes('--open')
const columns = count === 2 ? [COLUMNS[0], COLUMNS[1]] : COLUMNS

const root = localPath(new URL('..', import.meta.url))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-compare-layout-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-compare-layout-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const startedAt = new Date(Date.now() - 2 * 3_600_000).toISOString()
const slots = []
let n = 0
for (const column of columns) {
  n += 1
  const missionId = `mission_5e200000-0000-4000-8000-0007${String(n).padStart(8, '0')}`
  const runId = `run_5e2007${String(n)}`
  await ledger.createMission({
    missionId, runId, prompt: PROMPT,
    runtime: column.runtime, model: column.model, requestedRouteId: 'opencode', resolvedRouteId: 'opencode-account:default', cliVersion: null,
    workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: startedAt
  })
  await ledger.appendEvents(missionId, columnEvents(column, { missionId, runId, startedAt, minutes: 60 + n * 9 }))
  slots.push({ slot: column.slot, route: { runtime: column.runtime, model: column.model, label: column.label, mode: 'auto' }, missionIds: [missionId] })
}
await ledger.flush?.()
await writeFile(join(profilePath, 'compares.json'), JSON.stringify({ schemaVersion: 1, compares: [{ compareId: 'cmp_layout', prompt: PROMPT, createdAt: startedAt, slots, ...(blind ? { blind: true } : {}) }] }), 'utf8')

const name = `${String(count)}col-${blind ? 'blind' : 'open'}`
const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `compare-layout-${name}-${tag}`,
  port: 9893,
  workspace,
  profilePath,
  sendsNothing: true,
  outPath: join(recordRoot('compare-layout-frames-2026-10-05'), tag, name),
  seed: { schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})

/** What reads badly on the screen now, measured. */
const MEASURE = `(() => {
  const r = (el) => el.getBoundingClientRect()
  const compare = document.querySelector('.lc-compare')
  const scroll = document.querySelector('.lc-compare__scroll')
  const heads = [...document.querySelectorAll('.lc-compare__head')]
  const cells = [...document.querySelectorAll('.lc-compare__cell')]
  // Text past its own column: any leaf in a cell whose right edge passes the cell's.
  const spills = cells.flatMap((cell, index) => {
    const edge = r(cell).right + 1
    return [...cell.querySelectorAll('*')].filter((el) => el.children.length === 0 && el.textContent.trim().length > 0 && r(el).width > 0 && r(el).right > edge)
      .map((el) => ({ column: index, text: el.textContent.trim().slice(0, 50), over: Math.round(r(el).right - edge) }))
  })
  // How far above the cell's bottom each column's words end.
  const ends = cells.map((cell) => {
    const last = [...cell.children].filter((child) => r(child).height > 0).at(-1)
    return last === undefined ? null : Math.round(r(cell).bottom - r(last).bottom)
  })
  const numbers = [...document.querySelectorAll('.lc-compare__numbers')].map((el) => ({ text: el.textContent, cut: el.scrollWidth > el.clientWidth + 1 }))
  const names = [...document.querySelectorAll('.lc-compare__name')].map((el) => ({ text: el.textContent, cut: el.scrollWidth > el.clientWidth + 1 }))
  const controls = document.querySelector('.lc-composer__controls')
  const composerCut = controls ? [...controls.querySelectorAll('*')].filter((el) => el.children.length === 0 && el.textContent.trim().length > 0 && (r(el).right > r(controls).right + 1 || el.scrollWidth > el.clientWidth + 1)).map((el) => el.textContent.trim().slice(0, 40)) : []
  const prose = cells.map((cell) => { const p = cell.querySelector('.lc-agentline p, p'); return p ? Math.round(r(p).width) : null })
  return JSON.stringify({
    window: [innerWidth, innerHeight],
    compare: compare ? [Math.round(r(compare).width), Math.round(r(compare).height)] : null,
    scroll: scroll ? Math.round(r(scroll).height) : null,
    composer: controls ? Math.round(r(controls.closest('.lc-composer') ?? controls).height) : null,
    controlsRows: controls ? Math.round(r(controls).height) : null,
    heads: heads.map((h) => Math.round(r(h).width)),
    proseWidth: prose, ends, numbers, names, spills: spills.slice(0, 12), composerCut
  })
})()`

let failures = 0
try {
  await drive.capture('launch', () => drive.ready())
  await drive.resize(1200, 780)
  await drive.capture('the comparison, opened', () => drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('.lc-convrow button.lc-conv')].find((row) => /browser RPG/i.test(row.innerText))?.click()
    await new Promise((r) => setTimeout(r, 1500))
    ;[...document.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Open the comparison')?.click()
    for (let i = 0; i < 20 && !document.querySelector('.lc-compare__cell'); i += 1) await new Promise((r) => setTimeout(r, 250))
    await new Promise((r) => setTimeout(r, 1500))
    return String(document.querySelectorAll('.lc-compare__cell').length) + ' cells'
  })()`))
  for (const [width, height] of [[1200, 780], [1000, 680], [1600, 900]]) {
    await drive.resize(width, height)
    await sleep(500)
    await drive.capture(`${String(width)}x${String(height)} top`, () => drive.evaluate(`(() => { document.querySelector('.lc-compare__scroll')?.scrollTo(0, 0); return ${MEASURE} })()`))
    await drive.capture(`${String(width)}x${String(height)} bottom`, () => drive.evaluate(`(async () => {
      const s = document.querySelector('.lc-compare__scroll'); s?.scrollTo(0, s.scrollHeight)
      await new Promise((r) => setTimeout(r, 300))
      return ${MEASURE}
    })()`))
  }
  /*
   * A COLUMN THAT GROWS IS FOLLOWED (0.632). A running column grows with each
   * event; the view hears that as the body resizing and moves where it holds.
   * Grown here by hand (a tall block in the shortest body), at 1200x780: its
   * offset must go from under the heads to its end at the bottom edge, and
   * back when the block goes. The frames above are all at rest and cannot see
   * this; it is what the one observer is for.
   */
  await drive.resize(1200, 780)
  await sleep(400)
  const grown = JSON.parse(String(await drive.capture('a column that grows is followed', () => drive.evaluate(`(async () => {
    const frame = () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)))
    const port = document.querySelector('.lc-compare__scroll')
    const bodies = [...document.querySelectorAll('.lc-compare__cellbody')]
    if (port === null || bodies.length === 0) return JSON.stringify({ error: 'no compare bodies' })
    const body = bodies.reduce((a, b) => (b.offsetHeight < a.offsetHeight ? b : a))
    const read = () => Number.parseFloat(body.style.getPropertyValue('--lc-compare-stick'))
    port.scrollTo(0, 0)
    await frame()
    const before = read()
    const block = document.createElement('div')
    block.style.height = '2400px'
    body.appendChild(block)
    await frame()
    const grownAt = read()
    const expected = Math.round(port.clientHeight - body.offsetHeight - 8)
    block.remove()
    await frame()
    const after = read()
    return JSON.stringify({ before, grownAt, expected, after })
  })()`))))
  say(`  grows: ${JSON.stringify(grown)}`)
  if (grown.error !== undefined || grown.grownAt !== grown.expected || grown.after !== grown.before || !(grown.grownAt < 0)) failures += 1
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. A finished ${String(count)}-column ${blind ? 'blind ' : ''}comparison, seeded; nothing sent.`, extra: `Failures: ${String(failures)}` })
}
process.exit(failures === 0 ? 0 : 1)
