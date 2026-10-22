// A free model OpenCode dropped is said before the send (0.819).
//
//   node _tools/drive-a-dropped-model-is-said-before-the-send.mjs [--packaged <exe>] [--tag <name>]
//
// Pip is seeded on a free model OpenCode does not list: a made-up id, so the drive never depends on which free models
// OpenCode dropped this week (the 0.817 sweep met Ling, Space Bunny and LongCat). Ada is on the drives' free model.
// Nothing is sent. Then:
//   1. Home: Pip's model reads amber, and its hover says what to do; Ada's does not.
//   2. Pip's conversation: over the chat box, "OpenCode no longer offers ..." and "Switch Pip to <a free model OpenCode
//      lists>", at three window sizes.
//   3. The press moves the chat box and Pip's own saved model to it; the notice goes, and Home's card is plain again.
//   4. Ada's conversation says nothing of the kind.

import { readFile } from 'node:fs/promises'
import { join } from 'node:path'

import { FREE_ROUTE, openTeammateScript, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'
const DROPPED = 'opencode/retired-preview-free'

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 500)}`}`)
}

const workspace = await scratchRepository('locust-drive-dropped-model-ws-')
const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `dropped-model-${tag}`,
  port: 9943,
  workspace,
  sendsNothing: true,
  outPath: join(recordRoot('a-dropped-model-is-said-before-the-send-2026-10-22'), tag),
  seed: {
    schemaVersion: 1,
    teammates: [
      { teammateId: 'tm_pip', name: 'Pip', hue: 'violet', role: 'Docs & QA', createdAt: '2026-10-22T05:00:00.000Z', route: { runtime: 'opencode', model: DROPPED, mode: 'accept-edits' } },
      { teammateId: 'tm_ada', name: 'Ada', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-10-22T05:00:00.000Z', route: FREE_ROUTE }
    ],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false }
  }
})

const CARDS = `JSON.stringify([...document.querySelectorAll('.lc-hometeam__card')].map((card) => {
  const route = card.querySelector('.lc-hometeam__route')
  const name = card.querySelector('.lc-hometeam__name')?.textContent?.trim()
  return route === null || name === undefined ? undefined : { name, dropped: route.classList.contains('is-dropped'), title: route.getAttribute('title'), color: getComputedStyle(route).color }
}).filter(Boolean))`
const NOTICE = `(() => {
  const notice = document.querySelector('.lc-notice.is-offer')
  if (notice === null) return JSON.stringify(null)
  notice.scrollIntoView({ block: 'center' })
  const button = notice.querySelector('button')
  const box = button?.getBoundingClientRect()
  return JSON.stringify({ text: notice.querySelector('.lc-notice__text')?.textContent ?? '', label: button?.textContent ?? '',
    button: box === undefined ? null : { height: Math.round(box.height), inView: box.top >= 0 && box.bottom <= innerHeight && box.left >= 0 && box.right <= innerWidth } })
})()`
const home = () => drive.evaluate(`(async () => {
  document.querySelector('.lc-brand__lockup')?.click()
  for (let i = 0; i < 40 && !document.querySelector('.lc-hometeam'); i += 1) await new Promise((r) => setTimeout(r, 150))
})()`)

try {
  await drive.ready()
  await drive.resize(1440, 900)
  // OpenCode's catalogue is read at launch; until then nothing is said.
  for (let i = 0; i < 120 && !(await drive.evaluate(`document.querySelector('.lc-hometeam__route.is-dropped') !== null`)); i += 1) await sleep(500)

  // 1. Home.
  const cards = JSON.parse(String(await drive.capture('Home: Pip on a model OpenCode no longer lists', async () => { await sleep(500); return drive.evaluate(CARDS) })))
  const pip = cards.find((card) => card.name === 'Pip')
  const ada = cards.find((card) => card.name === 'Ada')
  check('Pip’s model reads amber on Home, and its hover says what to do', pip?.dropped === true && /^OpenCode no longer offers .+\. Open Pip to switch\.$/.test(pip.title ?? ''), JSON.stringify(pip))
  check('Ada’s does not', ada !== undefined && ada.dropped === false, JSON.stringify(ada))

  // 2. Pip's conversation, at three sizes.
  await drive.evaluate(openTeammateScript('Pip'))
  for (let i = 0; i < 40 && !(await drive.evaluate(`document.querySelector('.lc-notice.is-offer') !== null`)); i += 1) await sleep(250)
  const notice = JSON.parse(String(await drive.capture('Pip: said over the chat box', () => drive.evaluate(NOTICE))))
  check('over the chat box: OpenCode no longer offers it', notice !== null && /^OpenCode no longer offers Retired Preview Free\.$/.test(notice.text), JSON.stringify(notice))
  check('and the press names Pip and a free model OpenCode lists', notice !== null && /^Switch Pip to .+ Free$/.test(notice.label) && notice.button?.inView === true, JSON.stringify(notice))
  for (const [width, height] of [[1120, 720], [860, 640]]) {
    await drive.resize(width, height)
    await sleep(600)
    const at = JSON.parse(String(await drive.capture(`Pip at ${String(width)} by ${String(height)}`, () => drive.evaluate(NOTICE))))
    check(`at ${String(width)} by ${String(height)}, the sentence and its button are whole`, at !== null && at.button?.inView === true && at.button.height < 40, JSON.stringify(at))
  }
  await drive.resize(1440, 900)

  // 3. The press.
  const next = notice === null ? '' : notice.label.replace(/^Switch Pip to /, '')
  await drive.evaluate(`document.querySelector('.lc-notice.is-offer button')?.click()`)
  for (let i = 0; i < 40 && (await drive.evaluate(`document.querySelector('.lc-notice.is-offer') !== null`)); i += 1) await sleep(250)
  const after = String(await drive.capture('Pip moved: the chat box and Pip’s own model', async () => {
    await sleep(500)
    return drive.evaluate(`JSON.stringify({ notice: document.querySelector('.lc-notice.is-offer') !== null, chip: (document.querySelector('.lc-control__model')?.textContent ?? '').trim() })`)
  }))
  const moved = JSON.parse(after)
  const stored = JSON.parse(await readFile(join(drive.profile, 'teammates.json'), 'utf8')).teammates.find((mate) => mate.teammateId === 'tm_pip')
  check('the notice went, and the chat box is on the model it named', !moved.notice && next.replace(/ Free$/, '').length > 0 && moved.chip.length > 0 && next.toLowerCase().startsWith(moved.chip.toLowerCase().split(' ')[0] ?? '?'), `${after} || named ${next}`)
  check('Pip’s own saved model moved too, and nothing else of Pip did', stored?.route?.model !== DROPPED && /^opencode\/.+-free$/.test(stored?.route?.model ?? '') && stored.route.mode === 'accept-edits' && stored.name === 'Pip' && stored.role === 'Docs & QA', JSON.stringify(stored))
  await home()
  const again = JSON.parse(String(await drive.capture('Home: Pip plain again', async () => { await sleep(500); return drive.evaluate(CARDS) })))
  check('Home’s card is plain again', again.find((card) => card.name === 'Pip')?.dropped === false, JSON.stringify(again))

  // 4. Ada.
  await drive.evaluate(openTeammateScript('Ada'))
  await sleep(1200)
  check('Ada’s conversation says nothing of the kind', (await drive.evaluate(`document.querySelector('.lc-notice.is-offer') === null`)) === true)
} catch (error) {
  failures += 1
  say(`  [FAIL] the drive stopped: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. Pip seeded on ${DROPPED}, a free model OpenCode does not list; Ada on ${FREE_ROUTE.model}. Nothing is sent.` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} FAILURE(S)`)
process.exit(failures === 0 ? 0 : 1)
