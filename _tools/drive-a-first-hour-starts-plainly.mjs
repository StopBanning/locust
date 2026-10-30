// A fresh profile's first screens say how to start, plainly (0.514).
//
//   node _tools/drive-a-first-hour-starts-plainly.mjs [--packaged <exe>] [--tag <name>]
//
// A first-hour pass on 0.512 found the free start was a sentence with nothing
// to press, the new-teammate form showed Create before the model, and a copy's
// Settings said it would update the machine's agents on its own. A fresh
// profile, no teammates, an empty folder. Sends nothing.

import { join } from 'node:path'
import { recordRoot, say, scratchRepository, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'
const workspace = await scratchRepository('locust-drive-first-hour-ws-')
const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `first-hour-starts-plainly-${tag}`,
  port: 9819,
  workspace,
  sendsNothing: true,
  outPath: join(recordRoot('a-first-hour-starts-plainly-2026-10-01'), tag),
  seed: { schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 260)}`}`)
}
const chip = `[...document.querySelectorAll('.lc-control')].find((b) => b.getAttribute('aria-haspopup') === 'listbox')?.innerText.replace(/\\s+/g, ' ').trim() ?? ''`
// The chip names the model and effort; the runtime is its mark and its title ("OpenCode / opencode/<model>"), and a
// free model wears a Free badge (since after 0.709, when the chip read "OpenCode / Exo Free" in words).
const route = `(() => { const c = [...document.querySelectorAll('.lc-control')].find((b) => b.getAttribute('aria-haspopup') === 'listbox'); return { title: c?.title ?? '', runtime: c?.querySelector('.lc-runtimemark')?.getAttribute('data-runtime') ?? '', free: c?.querySelector('.lc-control__free')?.textContent.trim() ?? '' } })()`

try {
  await drive.capture('launch', () => drive.ready())
  const home = JSON.parse(String(await drive.capture('Home, fresh', () => drive.evaluate(`(async () => {
    for (let i = 0; i < 40 && ![...document.querySelectorAll('button')].some((b) => b.innerText.trim() === 'Use a free model'); i += 1) await new Promise((r) => setTimeout(r, 250))
    return JSON.stringify({ chip: ${chip}, button: [...document.querySelectorAll('button')].some((b) => b.innerText.trim() === 'Use a free model') })
  })()`))))
  check('Home offers "Use a free model" beside the sentence about it', home.button === true, JSON.stringify(home))
  const freed = JSON.parse(String(await drive.capture('Use a free model pressed', () => drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Use a free model')?.click()
    await new Promise((r) => setTimeout(r, 900))
    return JSON.stringify({ chip: ${chip}, route: ${route}, button: [...document.querySelectorAll('button')].some((b) => b.innerText.trim() === 'Use a free model') })
  })()`))))
  check('pressed, the chat box is on OpenCode, on a free model',
    freed.route?.runtime === 'opencode' && /^OpenCode \/ opencode\/[\w.-]+-free$/.test(freed.route?.title ?? '') && freed.route?.free === 'Free', JSON.stringify(freed))
  check('and the offer goes, having done its job', freed.button === false, JSON.stringify(freed))

  const form = JSON.parse(String(await drive.capture('New teammate, as it opens', () => drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('button')].find((b) => /New teammate/.test(b.innerText))?.click()
    await new Promise((r) => setTimeout(r, 900))
    const model = document.querySelector('.lc-teammatemodel')?.getBoundingClientRect()
    const look = [...document.querySelectorAll('.lc-fieldlabel')].find((el) => /^look$/i.test(el.innerText.trim()))?.getBoundingClientRect()
    return JSON.stringify({ height: window.innerHeight, model: model === undefined ? null : Math.round(model.bottom), look: look === undefined ? null : Math.round(look.top) })
  })()`))))
  check('the model is on screen without scrolling', form.model !== null && form.model < form.height, JSON.stringify(form))
  check('and comes before how the teammate looks', form.model !== null && form.look !== null && form.model < form.look, JSON.stringify(form))
  await drive.evaluate(`(async () => {
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    ;[...document.querySelectorAll('button')].find((b) => /^Cancel$/.test(b.innerText.trim()))?.click()
    await new Promise((r) => setTimeout(r, 600))
  })()`)

  const runtimes = String(await drive.capture('Settings > Runtimes, in a copy', () => drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('button')].find((b) => b.innerText.replace(/\\s+/g, ' ').trim() === 'Settings')?.click()
    await new Promise((r) => setTimeout(r, 700))
    ;[...document.querySelectorAll('.lc-settings__navitem')].find((b) => /AI agents|Runtimes/.test(b.innerText))?.click()
    for (let i = 0; i < 40 && !/never updates the agents|Update Codex CLI and Copilot CLI/.test(document.body.innerText + [...document.querySelectorAll('[aria-label]')].map((el) => el.getAttribute('aria-label')).join(' ')); i += 1) await new Promise((r) => setTimeout(r, 250))
    const note = [...document.querySelectorAll('.lc-settings__note')].map((el) => el.innerText.trim()).find((text) => /agents on this machine|Updating/.test(text)) ?? ''
    const sw = [...document.querySelectorAll('button[role="switch"]')].some((b) => b.getAttribute('aria-label') === 'Update Codex CLI and Copilot CLI on their own')
    return JSON.stringify({ note, sw })
  })()`)))
  const said = JSON.parse(runtimes)
  check('a copy says it never updates the machine\'s agents, and shows no switch that would', /never updates the agents on this machine/.test(said.note) && said.sw === false, runtimes)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. A fresh profile, nobody on the team.`, extra: `Checks failed: ${String(failures)}` })
}
if (failures > 0) process.exitCode = 1
