// An automatic reply stays on the model picked in its conversation (0.854).
//
//   node _tools/drive-a-returned-answer-keeps-the-conversations-model.mjs [--packaged <exe>] [--tag <name>]
//
// The first-session drive, 2026-11-01: Sable's free model ran out, the person
// pressed Stop and switch to Nemotron, and the answer Penny brought back
// started Sable on the model saved on Sable's profile again -- the one that
// had run out. Here Wren is saved on one free model and the person picks
// another for this conversation; Wren asks Booty, Booty answers in plain text,
// and the host brings the answer back. Wren's turn to take it must run on the
// model picked in the conversation, not the saved one.

import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import { freeModels, pickRouteScript, recordRoot, say, scratchRepository, startDrive, teammateFace, teammateRows } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag')
const outPath = tag === undefined ? undefined : join(recordRoot('beta-fixes-2026-09-24'), `returned-answer-keeps-model-${tag}`)
if (outPath !== undefined) await mkdir(outPath, { recursive: true })

const [saved, picked, third] = freeModels()
// Booty only answers, so any model that answers in plain text will do; a third free model when there is one.
const bootyModel = third ?? picked
if (picked === undefined) {
  say('this drive needs two free models, and OpenCode lists one: nothing to tell apart.')
  process.exit(1)
}
// As drive-lib's FREE_ROW and FREE_SEARCH are made: the picker draws a model by the words of its id.
const words = (model) => model.replace(/^opencode\//, '').replace(/-free$/, '')
const rowOf = (model) => '/' + words(model).split('-').map((word) => word.replace(/[.*+?^$()|[\]\\{}]/g, (character) => '\\' + character)).join('.*') + '/i'
const searchOf = (model) => words(model).replace(/[-.]+/g, ' ')

const workspace = await scratchRepository('locust-drive-kept-model-ws-')
const T0 = '2026-11-01T18:00:00.000Z'
const drive = await startDrive({
  name: 'returned-answer-keeps-model',
  port: 9563,
  workspace,
  ...(packaged === undefined ? {} : { packaged }),
  ...(outPath === undefined ? {} : { outPath }),
  seed: {
    schemaVersion: 1,
    teammates: [
      { teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: T0, route: { runtime: 'opencode', model: saved, mode: 'accept-edits' } },
      { teammateId: 'tm_booty', name: 'Booty', hue: 'violet', role: 'Custom', roleTitle: 'Reviewer', createdAt: T0, route: { runtime: 'opencode', model: bootyModel, mode: 'accept-edits' } }
    ],
    missionOwners: {},
    settings: { swarm: false, relay: true, relayHopCap: 2, memoryMode: 'off' }
  }
})

const busy = `${teammateRows()}.some(r => /working|running|starting|replying|listening|thinking|waiting/i.test(r.innerText))`
const chip = `(() => {
  const control = [...document.querySelectorAll('.lc-control')].find(b => b.getAttribute('aria-haspopup') === 'listbox')
  return control === undefined ? '' : (control.innerText + ' ' + (control.title ?? '')).replace(/\\s+/g, ' ').trim()
})()`
const verdicts = []
const judge = (name, pass, seen) => {
  verdicts.push(`${name}: ${pass ? 'PASS' : 'FAIL'} (${seen})`)
  say(`${pass ? '[PASS]' : '[FAIL]'} ${name} -- ${seen}`)
}

try {
  await drive.capture('launch: Wren saved on one free model, Booty on another, replies on', () => drive.ready())
  say(`Wren's saved model: ${saved}; picked for this conversation: ${picked}`)
  await drive.capture("open Wren, and pick the other free model for this conversation", () => drive.evaluate(`(async () => {
    window.__updates = []
    window.desktop.onCodexMissionUpdate(u => { if (u.kind !== 'event') window.__updates.push({ kind: u.kind, message: u.message, teammateId: u.teammateId, startedBy: u.startedBy, model: u.data?.model, followUpOf: u.data?.followUpOf }) })
    const wren = ${teammateFace('Wren')}
    wren.click()
    await new Promise(r => setTimeout(r, 600))
    const said = await ${pickRouteScript({ group: '/opencode/i', search: searchOf(picked), row: rowOf(picked) })}
    await new Promise(r => setTimeout(r, 400))
    return String(said).slice(0, 200) + ' || chip: ' + ${chip}
  })()`))
  const before = String(await drive.evaluate(chip))
  judge('the conversation is on the picked model before sending', new RegExp(rowOf(picked).slice(1, -2), 'i').test(before), before)
  /*
   * The block itself, to copy: the first run (2026-11-01) asked Wren on Nemotron to "send Booty one message with the
   * share block form", and Wren worked out 17 times 3 itself and sent nothing.
   */
  await drive.capture('Wren is asked to put a question to Booty', () => drive.evaluate(`(async () => {
    const field = document.querySelector('form.command-dock textarea')
    const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
    setter.call(field, 'Do not answer this yourself, and do not read, edit or run anything. Your whole reply must be exactly the block below, copied character for character, and nothing else:\\n\\n<locust-share to="Booty" wants="answer">Answer in your own reply as plain text, with no share block back to me: what is 17 times 3?</locust-share>')
    field.dispatchEvent(new Event('input', { bubbles: true }))
    for (let i = 0; i < 120; i += 1) {
      await new Promise(r => setTimeout(r, 250))
      const button = document.querySelector('button[aria-label="Send"]')
      if (button && !button.disabled) { button.click(); return 'sent' }
    }
    return 'no send'
  })()`))
  // Polled from here, up to ten minutes: one evaluate may not run past drive-lib's 400 s. A model out of free usage
  // waits on its provider for minutes, so it is said and given up on (GLM, the third run, 2026-11-01).
  await drive.capture('wait for the whole exchange to settle', async () => {
    const t0 = Date.now()
    while (Date.now() - t0 < 600_000) {
      await new Promise((r) => setTimeout(r, 2000))
      const state = String(await drive.evaluate(`(() => {
        if (/usage exceeded|rate limit/i.test(document.querySelector('.lc-thread')?.innerText ?? '')) return 'stalled'
        return !document.querySelector('button[aria-label^="Stop the running"]') && !${busy} ? 'settled' : 'busy'
      })()`))
      const s = Math.round((Date.now() - t0) / 1000)
      if (state === 'stalled') return `a model is out of free usage after ${String(s)}s`
      if (state === 'settled' && s > 15) return `settled after ${String(s)}s`
    }
    return 'still going after 10 minutes'
  })
  const updates = JSON.parse(String(await drive.evaluate(`JSON.stringify(window.__updates ?? [])`)))
  await writeFile(join(drive.out, 'host-updates.json'), JSON.stringify(updates, null, 2), 'utf8')
  const starts = updates.filter((u) => u.kind === 'mission-started' && u.startedBy?.kind === 'relay')
  await drive.capture('who the host started, on which model', async () =>
    starts.map((u) => `${u.teammateId} hop ${String(u.startedBy?.hop)} on ${String(u.model)}`).join(' / ') || 'no automatic runs')
  const booty = starts.find((u) => u.teammateId === 'tm_booty')
  const wrenBack = starts.find((u) => u.teammateId === 'tm_wren')
  judge('Booty answered on their own saved model', booty?.model === bootyModel, String(booty?.model))
  judge("Wren's turn to take the answer ran on the model picked in the conversation, not the saved one", wrenBack?.model === picked, wrenBack === undefined ? 'no returned-answer turn was started' : String(wrenBack.model))
  await drive.capture("Wren's thread: Booty's answer, brought back, and the model it shows", () => drive.evaluate(`(async () => {
    const row = ${teammateRows()}.find(r => /Wren/.test(r.innerText))
    const conversation = row && row.conversation
    if (conversation) conversation.click()
    await new Promise(r => setTimeout(r, 900))
    const thread = document.querySelector('.lc-thread')?.innerText ?? ''
    return 'returned label seen: ' + /Returned by Locust/.test(thread) + ' || 51 seen: ' + /\\b51\\b/.test(thread) + ' || stalled: ' + /usage exceeded|rate limit/i.test(thread) + ' || chip: ' + ${chip}
  })()`))
  const after = String(await drive.evaluate(chip))
  judge("the conversation's model chip still shows the picked model", new RegExp(rowOf(picked).slice(1, -2), 'i').test(after), after)
} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({
    intro: `Build: ${packaged ?? 'whatever pnpm build last wrote to out/'}. Wren saved on ${saved}, this conversation on ${picked}; Booty on ${bootyModel}; replies on, budget 2.`,
    extra: verdicts.length === 0 ? '' : ['## Verdicts', '', ...verdicts.map((line) => `- ${line}`)].join('\n')
  })
}
