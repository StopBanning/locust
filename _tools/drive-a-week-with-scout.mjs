// A week with Scout is one card (0.817): the weekly tune-up.
//
//   node _tools/drive-a-week-with-scout.mjs [--packaged <exe>] [--tag <name>]
//
// A week cannot pass in a drive, so Scout's file is seeded, before launch, with a tally that began eight days ago:
// 410 looks, 6 of them read by its model, four GitHub cards all dismissed, two about teammates' turns sent on. Free
// model only: nothing is spent. Then:
//   1. Turning Scout on looks at once, and the look brings the week as one card on Home: "Your week", its counts in
//      Locust's words, and its one suggestion, Stop watching GitHub, as its button -- at three window sizes.
//   2. The press turns the GitHub watch off, as Settings then shows, and the card goes.
//   3. The next week began with that look: Scout's file says so.

import { readFile } from 'node:fs/promises'
import { join } from 'node:path'

import { FREE_ROUTE, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 500)}`}`)
}
const dayOf = (at) => `${String(at.getFullYear())}-${String(at.getMonth() + 1).padStart(2, '0')}-${String(at.getDate()).padStart(2, '0')}`
const began = dayOf(new Date(Date.now() - 8 * 86_400_000))
const NONE = { raised: 0, used: 0, dismissed: 0 }

const workspace = await scratchRepository('locust-drive-scout-week-ws-')
const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `scout-week-${tag}`,
  port: 9942,
  workspace,
  outPath: join(recordRoot('a-week-with-scout-2026-10-22'), tag),
  seed: {
    schemaVersion: 1,
    teammates: [{ teammateId: 'tm_ada', name: 'Ada', hue: 'violet', role: 'Docs & QA', createdAt: '2026-10-22T05:00:00.000Z', route: FREE_ROUTE }],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false }
  },
  files: {
    // Off, with no teammate yet: turning it on makes Scout and looks, reading this week from the file.
    'scout.json': {
      settings: { on: false, everyMinutes: 10, watches: { github: true, teammates: true, check: false }, notify: false, waitWhileWorking: true },
      cards: [],
      seen: [],
      lookIds: [],
      prs: [],
      week: { from: began, looks: 410, asked: 6, watches: { github: { raised: 4, used: 0, dismissed: 4 }, teammates: { raised: 2, used: 2, dismissed: 0 }, check: NONE }, sent: 2, changed: 0, dismissed: 4 }
    }
  }
})

const state = async () => JSON.parse(String(await drive.evaluate('window.desktop.scout.state().then((s) => JSON.stringify(s))')))
const WEEK_CARD = `(() => {
  const card = [...document.querySelectorAll('[data-scout-card]')].find((one) => one.innerText.includes('Your week'))
  if (!card) return 'no week card'
  card.scrollIntoView({ block: 'center' })
  const button = [...card.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Stop watching GitHub')
  const box = button?.getBoundingClientRect()
  return JSON.stringify({ text: card.innerText.replace(/\\s+/g, ' '), button: box === undefined ? null : { top: Math.round(box.top), bottom: Math.round(box.bottom), height: Math.round(box.height), inView: box.bottom <= innerHeight && box.top >= 0 } })
})()`

try {
  await drive.ready()
  await drive.resize(1440, 900)
  const turned = JSON.parse(String(await drive.evaluate(`window.desktop.scout.turnOn(${JSON.stringify({ runtime: FREE_ROUTE.runtime, model: FREE_ROUTE.model })}).then((s) => JSON.stringify(s))`)))
  check('Scout is on, on the free model', turned?.settings?.on === true, JSON.stringify(turned).slice(0, 200))
  for (let i = 0; i < 240 && (await state()).cards.some((card) => card.reading === true); i += 1) await sleep(500)
  const looked = await state()
  check('its first look brought the week as one card, keyed by the day it began', looked.cards.filter((card) => card.key.startsWith('week:')).map((card) => card.key).join() === `week:${began}`, JSON.stringify(looked.cards.map((card) => card.key)))

  // 1. The card, at three sizes.
  const shown = JSON.parse(String(await drive.capture('Home: the week as one card', async () => { await sleep(800); return drive.evaluate(WEEK_CARD) })))
  check('it says the week in Locust’s words', shown.text.includes('Your week') && shown.text.includes('This week: 410 looks, 6 of them read by its model.') && shown.text.includes('It raised 6 cards. You sent 2 on and dismissed 4.') && shown.text.includes('You dismissed all 4 GitHub cards and sent none on.'), shown.text)
  check('its one suggestion is its button, in view', shown.button !== null && shown.button.inView, JSON.stringify(shown.button))
  for (const [width, height] of [[1120, 720], [860, 640]]) {
    await drive.resize(width, height)
    await sleep(600)
    const at = JSON.parse(String(await drive.capture(`the week at ${String(width)} by ${String(height)}`, () => drive.evaluate(WEEK_CARD))))
    check(`at ${String(width)} by ${String(height)}, the card and its button are whole`, at.button !== null && at.button.inView && at.button.height < 48, JSON.stringify(at.button))
  }
  await drive.resize(1440, 900)

  // 2. The press.
  const pressed = String(await drive.evaluate(`(() => {
    const card = [...document.querySelectorAll('[data-scout-card]')].find((one) => one.innerText.includes('Your week'))
    const button = card && [...card.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Stop watching GitHub')
    if (!button) return 'no button'
    button.click()
    return 'pressed'
  })()`))
  for (let i = 0; i < 40 && (await state()).settings.watches.github !== false; i += 1) await sleep(250)
  const after = await state()
  check('the press turned the GitHub watch off, and the card went', pressed === 'pressed' && after.settings.watches.github === false && !after.cards.some((card) => card.key.startsWith('week:')), `${pressed}; ${JSON.stringify(after.settings.watches)}; ${JSON.stringify(after.cards.map((card) => card.key))}`)
  const switched = String(await drive.evaluate(`(async () => {
    if (!document.querySelector('.lc-settings')) document.querySelector('button[title="Settings (Ctrl 3)"]')?.click()
    for (let i = 0; i < 40 && !document.querySelector('.lc-settings'); i += 1) await new Promise((r) => setTimeout(r, 100))
    ;[...document.querySelectorAll('.lc-settings__navitem, button')].find((b) => b.innerText.trim() === 'General')?.click()
    for (let i = 0; i < 40 && !document.querySelector('[data-setting="scout"]'); i += 1) await new Promise((r) => setTimeout(r, 100))
    const row = [...document.querySelectorAll('[data-setting="scout"] [role="switch"]')].find((one) => /pull requests/i.test(one.getAttribute('aria-label') ?? ''))
    row?.scrollIntoView({ block: 'center' })
    return row ? row.getAttribute('aria-label') + ': ' + row.getAttribute('aria-checked') : 'no GitHub switch'
  })()`))
  await drive.capture('Settings: the GitHub watch is off', async () => { await sleep(400); return switched })
  check('Settings shows the GitHub watch off', /: false$/.test(switched), switched)

  // 3. The next week began with that look.
  const kept = JSON.parse(await readFile(join(drive.profile, 'scout.json'), 'utf8'))
  check('the next week began with that look', kept.week?.from === dayOf(new Date()) && kept.week.looks === 1, JSON.stringify(kept.week))
} catch (error) {
  failures += 1
  say(`  [FAIL] the drive stopped: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. Scout on ${FREE_ROUTE.model}; its file seeded with a week that began ${began}.` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} FAILURE(S)`)
process.exit(failures === 0 ? 0 : 1)
