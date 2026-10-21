// One free teammate: /tune after a real turn, review the three setups, run them blind,
// keep a drafted setup and explicitly save its instructions. No paid route or global config.
// node _tools/drive-tune-a-teammate.mjs [--packaged <exe>] [--tag <name>]
import { join } from 'node:path'
import { FREE_ROUTE, openTeammateScript, recordRoot, say, scratchRepository, sendAndWaitScript, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined
const baseline = 'Separate what you observed from what you assumed.'
const how = 'Check your reasoning before concluding. Keep the final answer to two short sentences.'
const knows = 'Distinguish observed facts from assumptions. Name a material uncertainty when one remains.'
const ask = 'Explain in two sentences why checking assumptions matters. Do not use tools or change files.'
const workspace = await scratchRepository('locust-tune-ws-')
const drive = await startDrive({
  name: 'tune-a-teammate', port: 9817, workspace, holdCursor: false,
  outPath: join(recordRoot('tune-a-teammate-2026-10-20'), arg('--tag') ?? 'live'),
  ...(arg('--packaged') === undefined ? {} : { packaged: arg('--packaged') }),
  seed: { schemaVersion: 1, teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', instructions: baseline, route: FREE_ROUTE, createdAt: '2026-10-20T12:00:00.000Z' }], missionOwners: {}, settings: { memoryMode: 'off', swarm: false, relay: false, autoMode: false, relayHopCap: 2 } }
})
let checks = 0, failures = 0
const check = (what, ok, detail) => {
  checks += 1
  if (!ok) failures += 1
  say(`[${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ' -- ' + JSON.stringify(detail)}`)
}
const roster = async () => JSON.parse(String(await drive.evaluate(`(async () => JSON.stringify(await window.desktop.listTeammates()))()`)))
const comparisons = async () => JSON.parse(String(await drive.evaluate(`(async () => JSON.stringify(await window.desktop.listCompares()))()`)))
const lessons = async () => JSON.parse(String(await drive.evaluate(`(async () => JSON.stringify(await window.desktop.listTuneLessons('tm_wren')))()`)))
const cardState = () => drive.evaluate(`(() => {
  const card = document.querySelector('.lc-tune')
  const areas = [...card.querySelectorAll('textarea')]
  const tops = areas.slice(1).map((area) => area.getBoundingClientRect().top)
  const box = card.getBoundingClientRect()
  return JSON.stringify({ fields: areas.map((area) => ({ label: area.getAttribute('aria-label'), value: area.value, cap: area.maxLength })), text: card.innerText,
    fits: box.left >= 0 && box.right <= innerWidth + 1 && box.top >= 0 && box.bottom <= innerHeight + 1, aligned: Math.max(...tops) - Math.min(...tops) < 2,
    overflow: card.scrollWidth > card.clientWidth, theme: document.documentElement.dataset.theme })
})()`)
const setArea = (label, value) => drive.evaluate(`(() => {
  const area = [...document.querySelectorAll('textarea')].find((el) => el.getAttribute('aria-label') === ${JSON.stringify(label)})
  if (!area || area.disabled) throw new Error('editable field not found')
  Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set.call(area, ${JSON.stringify(value)})
  area.dispatchEvent(new Event('input', { bubbles: true }))
})()`)
try {
  await drive.ready()
  await drive.resize(1440, 900)
  await drive.evaluate(openTeammateScript('Wren'))
  say(String(await drive.evaluate(sendAndWaitScript(ask, { settle: false }))))
  await drive.waitFor(`(async () => { const h = await window.desktop.getMissionHistory(); return h.ok && h.data.missions.some((m) => m.phase === 'completed') })()`, { timeoutMs: 180_000, what: 'the free teammate’s first completed turn' })
  check('the initial free turn finishes', true)
  await drive.capture('one finished turn before tune', () => 'The teammate has a real completed turn; no comparison yet.')
  await drive.evaluate(`(() => {
    const field = document.querySelector('form.command-dock textarea')
    Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set.call(field, '/tune')
    field.dispatchEvent(new Event('input', { bubbles: true }))
  })()`)
  await drive.waitFor(`[...document.querySelectorAll('.lc-slash__item')].some((item) => item.textContent.includes('/tune'))`, { what: '/tune in the teammate’s slash menu' })
  await drive.evaluate(`[...document.querySelectorAll('.lc-slash__item')].find((item) => item.textContent.includes('/tune')).click()`)
  await drive.waitForSelector('.lc-tune')
  await drive.waitFor(`document.querySelectorAll('.lc-tune__instructions:not(:disabled)').length === 3`, { timeoutMs: 180_000, what: 'drafts or editable fallback copies of Current' })
  const initial = JSON.parse(String(await cardState()))
  check('the card names all three instruction setups and discloses the side turn', initial.fields.length === 4 && initial.text.includes('Current') && initial.text.includes('How it works') && initial.text.includes('What it knows') && initial.text.includes('one side turn'), initial)
  check('no comparison starts before Run all three', (await comparisons()).data?.compares?.length === 0)
  check('drafting does not save teammate instructions', (await roster()).data?.teammates?.[0]?.instructions === baseline)
  // Real JSON drafts are model-dependent, and OpenCode 2 cannot fork yet. Edit both boxes as a person may, so
  // Keep exercises a known replacement even when the honest fallback began with three copies of Current.
  await setArea('How it works instructions', how)
  await setArea('What it knows instructions', knows)
  for (const theme of ['dark', 'light']) {
    await drive.evaluate(`window.desktop.setTheme(${JSON.stringify(theme)})`)
    for (const [width, height] of [[1440, 900], [1205, 803]]) {
      await drive.resize(width, height); await sleep(250)
      const state = JSON.parse(String(await cardState()))
      check(`${theme} ${width}: the card fits, with three aligned, capped editable instruction fields`, state.fits && state.aligned && !state.overflow && state.fields.filter((field) => field.cap === 4000).length === 3, { fits: state.fits, aligned: state.aligned })
      await drive.capture(`${theme} tune card ${width} by ${height}`, () => JSON.stringify(state))
    }
  }
  await drive.evaluate(`[...document.querySelectorAll('.lc-tune button')].find((button) => button.textContent.trim() === 'Run all three').click()`)
  await drive.waitForSelector('.lc-compare', { timeoutMs: 180_000 })
  await drive.waitFor(`document.querySelectorAll('.lc-compare__foot .lc-primarybutton:not(:disabled)').length === 3`, { timeoutMs: 240_000, what: 'all three free comparison answers' })
  const trial = (await comparisons()).data?.compares?.at(-1)
  check('one blind, file-changing comparison starts on the teammate’s same model', trial?.blind === true && trial?.changes === true && trial.slots.length === 3 && trial.slots.every((slot) => slot.route.runtime === FREE_ROUTE.runtime && slot.route.model === FREE_ROUTE.model), trial?.slots.map((slot) => slot.route))
  check('the exact three instruction texts reach the saved columns', JSON.stringify(trial?.slots.map((slot) => slot.route.instructions).sort()) === JSON.stringify([baseline, how, knows].sort()))
  // A tune's blind columns are Setup A, B and C (0.809): one model, three sets of instructions.
  check('the column heads show only blind letters before Keep', await drive.evaluate(`[...document.querySelectorAll('.lc-compare__head')].every((head) => /^Setup [ABC]/.test(head.innerText.trim()))`))
  check('the teammate still has Current before Keep', (await roster()).data?.teammates?.[0]?.instructions === baseline)
  for (const theme of ['dark', 'light']) {
    await drive.evaluate(`window.desktop.setTheme(${JSON.stringify(theme)})`)
    for (const [width, height] of [[1440, 900], [1205, 803]]) {
      await drive.resize(width, height); await sleep(250)
      await drive.capture(`${theme} blind comparison ${width} by ${height}`, () => drive.evaluate(`document.querySelector('.lc-compare').innerText`))
    }
  }
  const index = trial.slots.findIndex((slot) => slot.route.instructions === how)
  if (index < 0) throw new Error('the edited draft is missing')
  await drive.evaluate(`document.querySelectorAll('.lc-compare__foot')[${index}].querySelector('button.lc-primarybutton').click()`)
  await drive.waitForSelector('.lc-tuneoffer', { timeoutMs: 60_000 })
  check('Keep alone does not save the instructions', (await roster()).data?.teammates?.[0]?.instructions === baseline)
  await drive.evaluate(`document.querySelector('.lc-tuneoffer summary').click()`)
  for (const theme of ['dark', 'light']) {
    await drive.evaluate(`window.desktop.setTheme(${JSON.stringify(theme)})`)
    for (const [width, height] of [[1440, 900], [1205, 803]]) {
      await drive.resize(width, height); await sleep(250)
      check(`${theme} ${width}: the kept instructions can be reviewed before saving`, await drive.evaluate(`document.querySelector('.lc-tuneoffer pre')?.textContent === ${JSON.stringify(how)}`))
      await drive.capture(`${theme} kept instructions ${width} by ${height}`, () => drive.evaluate(`document.querySelector('.lc-tuneoffer').innerText`))
    }
  }
  await drive.evaluate(`[...document.querySelectorAll('.lc-tuneoffer button')].find((button) => button.textContent.trim() === 'Use these instructions for Wren').click()`)
  await drive.waitFor(`document.querySelector('.lc-tuneoffer')?.textContent.includes('next turn')`, { what: 'the explicit save confirmation' })
  check('the explicit button saves only the chosen draft', (await roster()).data?.teammates?.[0]?.instructions === how)
  await drive.capture('chosen instructions take effect next turn', () => drive.evaluate(`document.querySelector('.lc-tuneoffer').innerText`))
  check('nothing notes a lesson before Note what this tune showed is pressed', (await lessons()).data?.lessons?.length === 0)
  check('the kept strip offers Note what this tune showed and says its cost', await drive.evaluate(`(() => {
    const strip = document.querySelector('.lc-tuneoffer')?.innerText ?? ''
    return strip.includes('Note what this tune showed') && strip.includes('One side turn on') && strip.includes('free')
  })()`))
  await drive.evaluate(`[...document.querySelectorAll('.lc-tuneoffer button')].find((button) => button.textContent.trim() === 'Note what this tune showed').click()`)
  await drive.waitFor(`Boolean(document.querySelector('.lc-tuneoffer__lesson') || document.querySelector('.lc-tuneoffer__note'))`, { timeoutMs: 180_000, what: 'the tune lesson side turn to finish' })
  const savedLessons = (await lessons()).data?.lessons ?? []
  const savedLesson = savedLessons[0]
  check('one lesson is saved for the teammate and shown on the kept strip with Remove', savedLessons.length === 1 && typeof savedLesson?.when === 'string' && typeof savedLesson?.do === 'string' && await drive.evaluate(`(() => {
    const text = document.querySelector('.lc-tuneoffer__lesson')?.innerText ?? ''
    return text.includes('When ') && text.includes(' do ') && text.includes('Remove')
  })()`), savedLesson)
  for (const theme of ['dark', 'light']) {
    await drive.evaluate(`window.desktop.setTheme(${JSON.stringify(theme)})`)
    for (const [width, height] of [[1440, 900], [1205, 803]]) {
      await drive.resize(width, height); await sleep(250)
      await drive.capture(`${theme} kept lesson ${width} by ${height}`, () => drive.evaluate(`document.querySelector('.lc-tuneoffer').innerText`))
    }
  }
  await drive.evaluate(`(() => {
    const face = [...document.querySelectorAll('.lc-faces__one')].find((el) => (el.getAttribute('aria-label') ?? '').startsWith('Wren — '))
    face.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, clientX: 80, clientY: 160 }))
  })()`)
  await drive.waitFor(`[...document.querySelectorAll('.lc-context__item')].some((item) => item.querySelector('.lc-context__label')?.textContent.trim() === 'Edit')`, { what: 'Edit in the teammate row menu' })
  await drive.evaluate(`[...document.querySelectorAll('.lc-context__item')].find((item) => item.querySelector('.lc-context__label')?.textContent.trim() === 'Edit').click()`)
  await drive.waitForSelector('.lc-field--lessons', { what: 'the teammate’s lessons in Edit teammate' })
  await drive.evaluate(`document.querySelector('.lc-field--lessons')?.scrollIntoView({ block: 'center' })`)
  check('Edit teammate lists the lesson with Remove', await drive.evaluate(`(() => {
    const text = document.querySelector('.lc-field--lessons')?.innerText ?? ''
    return text.includes('When ') && text.includes(' do ') && text.includes('Remove')
  })()`))
  for (const theme of ['dark', 'light']) {
    await drive.evaluate(`window.desktop.setTheme(${JSON.stringify(theme)})`)
    for (const [width, height] of [[1440, 900], [1205, 803]]) {
      await drive.resize(width, height); await sleep(250)
      await drive.evaluate(`document.querySelector('.lc-field--lessons')?.scrollIntoView({ block: 'center' })`)
      await sleep(100)
      await drive.capture(`${theme} edit teammate lesson ${width} by ${height}`, () => drive.evaluate(`document.querySelector('.lc-field--lessons').innerText`))
    }
  }
  await drive.evaluate(`document.querySelector('.lc-dialog__close').click()`)
  await drive.waitFor(`!document.querySelector('.lc-field--lessons')`, { what: 'Edit teammate closed' })
  await drive.evaluate(`(() => {
    const field = document.querySelector('form.command-dock textarea')
    Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set.call(field, '/tune')
    field.dispatchEvent(new Event('input', { bubbles: true }))
  })()`)
  await drive.waitFor(`[...document.querySelectorAll('.lc-slash__item')].some((item) => item.textContent.includes('/tune'))`, { what: '/tune in the teammate’s slash menu for a second tune' })
  await drive.evaluate(`[...document.querySelectorAll('.lc-slash__item')].find((item) => item.textContent.includes('/tune')).click()`)
  await drive.waitForSelector('.lc-tune')
  await drive.waitFor(`document.querySelectorAll('.lc-tune__instructions:not(:disabled)').length === 3`, { timeoutMs: 180_000, what: 'the second tune’s draft side turn to finish' })
  const draftCheck = JSON.parse(String(await drive.evaluate(`(async () => {
    const h = await window.desktop.getMissionHistory()
    const missions = h.ok ? h.data.missions : []
    const sideDrafts = missions.filter((m) => m.startedBy?.kind === 'side' && m.prompt.includes('earlier tunes of this teammate found:'))
    const ordinary = missions.filter((m) => m.startedBy === undefined)
    return JSON.stringify({
      sideDraftCount: sideDrafts.length,
      carriesLesson: sideDrafts.some((m) => m.prompt.includes(${JSON.stringify(savedLesson?.when ?? '')}) && m.prompt.includes(${JSON.stringify(savedLesson?.do ?? '')})),
      ordinaryClean: ordinary.length > 0 && ordinary.every((m) => !m.prompt.includes('earlier tunes of this teammate found:'))
    })
  })()`)))
  check('the second /tune draft prompt carries the saved lesson while ordinary turns never do', draftCheck.sideDraftCount >= 1 && draftCheck.carriesLesson && draftCheck.ordinaryClean, draftCheck)
  await drive.evaluate(`[...document.querySelectorAll('.lc-tune button')].find((button) => button.textContent.trim() === 'Cancel').click()`)
  await drive.waitFor(`!document.querySelector('.lc-tune')`, { what: 'the second tune dialog closed' })
  await drive.evaluate(`document.querySelector('.lc-tuneoffer__lesson button').click()`)
  await drive.waitFor(`!document.querySelector('.lc-tuneoffer__lesson')`, { what: 'the lesson removed from the kept strip' })
  check('Remove deletes the lesson from the teammate’s store', (await lessons()).data?.lessons?.length === 0)
  check('the frames have no renderer errors', drive.record.every((frame) => frame.errors.length === 0))
} catch (error) {
  check('the drive completes', false, error instanceof Error ? error.message : String(error))
  await drive.capture('drive failure', () => drive.evaluate(`document.body.innerText`))
} finally {
  say(`TUNE A TEAMMATE: ${checks - failures}/${checks} passed`)
  await drive.finish({ intro: `One real free turn on ${FREE_ROUTE.model}; /tune drafts or falls back honestly, then three real free trials and an explicit instruction save. No paid turn. The edited boxes are fixture setups, not a claim that either is a better prompt.` })
  if (failures > 0) process.exitCode = 1
}
