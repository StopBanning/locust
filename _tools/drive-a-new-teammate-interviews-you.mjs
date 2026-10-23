// Drive (0.831): A NEW TEAMMATE INTERVIEWS YOU, from where a person stands. A fresh profile with nobody on the team:
// the New teammate window, named Iris, then "Let Iris interview you instead". Iris's conversation opens with the
// person's own ask, in Ask for that turn; she asks, the person answers in the chat box, and she writes her
// instructions. The card under that answer is changed and saved, and Iris's own window then holds them. Every turn
// runs on the free OpenCode model, so nothing is spent.
//
//   node _tools/drive-a-new-teammate-interviews-you.mjs [--packaged <Locust.exe>] [--tag <name>]
import { join } from 'node:path'

import { recordRoot, say, scratchRepository, sendAndWaitScript, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `a-new-teammate-interviews-you-${tag}`,
  port: 9783,
  workspace: await scratchRepository('locust-drive-interview-ws-'),
  outPath: join(recordRoot('a-new-teammate-interviews-you-2026-10-23'), tag),
  seed: { schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 600)}`}`)
}
const THREAD = `(document.querySelector('.lc-thread')?.innerText ?? '').replace(/\\s+/g, ' ')`
const CARD = `document.querySelector('.lc-interview')`
const openDialog = `(async () => {
  document.querySelector('.lc-sidebar button[aria-label="Add"]')?.click()
  await new Promise((r) => setTimeout(r, 500))
  ;[...document.querySelectorAll('.lc-context__item')].find((b) => /^New teammate/.test(b.textContent.trim()))?.click()
  for (let i = 0; i < 20 && !document.querySelector('.lc-dialog'); i += 1) await new Promise((r) => setTimeout(r, 200))
  document.querySelector('.lc-field--interview')?.scrollIntoView({ block: 'center' })
  return document.querySelector('.lc-dialog') ? 'open' : 'no dialog'
})()`
const history = async () => JSON.parse(String(await drive.evaluate(`window.desktop.getMissionHistory().then((r) => JSON.stringify(r.ok ? r.data.missions.map((m) => ({ id: m.missionId, prompt: m.prompt, phase: m.phase, mode: m.mode ?? null, sandbox: m.sandbox ?? null })) : []))`)))
const cardThere = async () => String(await drive.evaluate(`String(${CARD} !== null)`)) === 'true'

try {
  await drive.capture('launch: nobody on the team', () => drive.ready())
  await drive.resize(1215, 860)
  await sleep(800)

  // The New teammate window, with the interview offered under Instructions, at three sizes.
  check('the New teammate window opens', (await drive.evaluate(openDialog)) === 'open')
  const offered = String(await drive.capture('New teammate: the interview offered under Instructions', () => drive.evaluate(`(document.querySelector('.lc-field--interview')?.innerText ?? 'none').replace(/\\s+/g, ' ')`)))
  check('the window offers an interview, and says it only asks', /interview you instead/.test(offered) && /nothing is changed in the folder/.test(offered), offered)
  for (const [width, height] of [[960, 720], [760, 640]]) {
    await drive.resize(width, height)
    await sleep(700)
    await drive.capture(`New teammate, the interview offered, at ${String(width)} by ${String(height)}`, () => drive.evaluate(`(() => { document.querySelector('.lc-field--interview')?.scrollIntoView({ block: 'center' }); const r = document.querySelector('.lc-field--interview button')?.getBoundingClientRect(); return r ? JSON.stringify({ top: Math.round(r.top), bottom: Math.round(r.bottom), right: Math.round(r.right), width: innerWidth }) : 'no button' })()`))
  }
  await drive.resize(1215, 860)
  await sleep(700)

  // Named Iris, then the interview instead of the box.
  const pressed = String(await drive.capture('Iris named; Let Iris interview you instead', () => drive.evaluate(`(async () => {
    const field = document.querySelector('#lc-teammate-name')
    if (!field) return 'no name field'
    field.focus()
    const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set
    set.call(field, 'Iris')
    field.dispatchEvent(new Event('input', { bubbles: true }))
    await new Promise((r) => setTimeout(r, 400))
    const button = document.querySelector('.lc-field--interview button')
    if (!button || button.disabled || !/Let Iris interview you instead/.test(button.textContent)) return 'no interview button: ' + (button?.textContent ?? '')
    button.click()
    for (let i = 0; i < 30 && document.querySelector('.lc-dialog'); i += 1) await new Promise((r) => setTimeout(r, 200))
    return document.querySelector('.lc-dialog') ? 'still open' : 'closed'
  })()`)))
  check('the window closes on the interview press', pressed === 'closed', pressed)

  // Iris's conversation: the person's ask, then her first question.
  let thread = ''
  for (let i = 0; i < 240; i += 1) {
    await sleep(1000)
    thread = String(await drive.evaluate(THREAD))
    const running = String(await drive.evaluate(`String(document.querySelector('button[aria-label^="Stop the running"]') !== null)`)) === 'true'
    if (/Interview me so you can write your own standing instructions/.test(thread) && !running && i > 3) break
  }
  await drive.capture('Iris’s first question', () => drive.evaluate('1'))
  check('Iris’s conversation opens with the person’s ask to be interviewed', /Interview me so you can write your own standing instructions/.test(thread) && /You are Iris/.test(thread), thread.slice(0, 300))
  const first = await history()
  check('the interview’s first turn ran in Ask, so it changed nothing', first.length === 1 && first[0].mode === 'ask', JSON.stringify(first))
  check('Iris asked something first', /\?/.test(thread.slice(thread.indexOf('no password, key or other secret in them.') + 40)), thread.slice(-300))

  // The person answers in the chat box until she writes her instructions.
  const ANSWERS = [
    'I want a short summary every Monday of what changed in this repository, for my manager.',
    'Plain words, five bullet points at most, and name the files that changed.',
    'Never change any file. You only read and report.',
    'That is everything I need. Write your instructions now, in the block I asked for.',
    'Please write your standing instructions now inside the locust-instructions block.'
  ]
  // A turn still going is waited for (up to seven minutes), as a person waits for the question before answering it.
  const settled = async () => {
    for (let i = 0; i < 420; i += 1) {
      if (String(await drive.evaluate(`String(document.querySelector('button[aria-label^="Stop the running"]') !== null)`)) !== 'true') return true
      await sleep(1000)
    }
    return false
  }
  for (const [index, words] of ANSWERS.entries()) {
    if (!(await settled())) { check(`Iris's turn before answer ${String(index + 1)} ended within seven minutes`, false); break }
    if (await cardThere()) break
    const said = String(await drive.capture(`answer ${String(index + 1)}`, () => drive.evaluate(sendAndWaitScript(words, { waitSeconds: 420 }))))
    check(`answer ${String(index + 1)} was sent and answered`, said.startsWith('finished'), said.slice(-200))
  }
  await settled()
  for (let i = 0; i < 20 && !(await cardThere()); i += 1) await sleep(500)
  const card = String(await drive.capture('Iris’s instructions, on the card under her answer', () => drive.evaluate(`(() => { const card = ${CARD}; card?.scrollIntoView({ block: 'center' }); return card ? JSON.stringify({ said: card.innerText.replace(/\\s+/g, ' '), text: card.querySelector('textarea')?.value ?? '' }) : 'no card' })()`)))
  check('the card appears under the answer that wrote them', card !== 'no card', thread.slice(-400))
  if (card !== 'no card') {
    const shown = JSON.parse(card)
    check('the card offers the words to change, and Save', /Iris wrote its own instructions from your answers/.test(shown.said) && /Save as Iris’s instructions/.test(shown.said) && shown.text.length > 20, shown.said.slice(0, 200))
    const tags = String(await drive.evaluate(THREAD))
    check('the answer shows the words, never the block’s tags', !/<\/?locust-instructions/.test(tags.slice(tags.lastIndexOf('Iris') - 20)), tags.slice(-300))
    // The person adds a line of their own, by typing at the end of the box, then saves.
    await drive.evaluate(`(() => { const box = ${CARD}?.querySelector('textarea'); box?.focus(); box?.setSelectionRange(box.value.length, box.value.length); return 1 })()`)
    await drive.send('Input.insertText', { text: '\nSign each summary with your name.' })
    const saved = String(await drive.capture('Save: Iris’s instructions kept', () => drive.evaluate(`(async () => {
      ;[...(${CARD}?.querySelectorAll('button') ?? [])].find((b) => /Save as Iris/.test(b.textContent))?.click()
      for (let i = 0; i < 30 && !/Saved as Iris/.test(${CARD}?.innerText ?? ''); i += 1) await new Promise((r) => setTimeout(r, 200))
      return (${CARD}?.innerText ?? 'no card').replace(/\\s+/g, ' ')
    })()`)))
    check('the card says they are saved', /Saved as Iris’s instructions/.test(saved), saved)
    const kept = JSON.parse(String(await drive.evaluate(`window.desktop.listTeammates().then((r) => JSON.stringify(r.ok ? r.data.teammates.map((m) => ({ name: m.name, instructions: m.instructions ?? '' })) : []))`)))
    const iris = kept.find((one) => one.name === 'Iris')
    check('Iris’s record holds the instructions, the person’s line included', iris !== undefined && iris.instructions.includes('Sign each summary with your name.') && iris.instructions.length > 40, JSON.stringify(kept))
    // From where the person stands: Iris's own window shows them.
    const window_ = String(await drive.capture('Iris’s own window: the instructions there', () => drive.evaluate(`(async () => {
      document.querySelector('.lc-faces__team')?.click()
      for (let i = 0; i < 30 && document.querySelectorAll('.lc-rostercard').length === 0; i += 1) await new Promise((r) => setTimeout(r, 150))
      const card = [...document.querySelectorAll('.lc-rostercard')].find((c) => /Iris/.test(c.innerText))
      const button = card && [...card.querySelectorAll('button')].find((b) => /Edit/.test(b.textContent ?? '') || /Edit/.test(b.getAttribute('aria-label') ?? ''))
      ;(button ?? card)?.click()
      for (let i = 0; i < 20 && !document.querySelector('.lc-dialog'); i += 1) await new Promise((r) => setTimeout(r, 200))
      const box = document.querySelector('.lc-dialog textarea[aria-label="Instructions for this teammate"]')
      box?.scrollIntoView({ block: 'center' })
      return box?.value ?? 'no box'
    })()`)))
    check('Iris’s edit window holds the saved instructions', window_.includes('Sign each summary with your name.'), window_.slice(0, 200))
    const again = String(await drive.evaluate(`(document.querySelector('.lc-field--interview button')?.textContent ?? 'none')`))
    check('an edit window offers the interview again', /Let Iris interview you again/.test(again), again)
  }
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. A fresh profile, nobody on the team; Iris runs on the free OpenCode model.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
