// A Claude Code teammate on Haiku (0.811): /tune drafts "What it knows" as a skill, that column's run alone is handed
// it, Keep offers Use this skill for Ada, the teammate's next turn is handed the kept skill, and Edit teammate lists it
// with Remove. Spends Claude Haiku turns on the person's own plan: one turn, the draft, three trials, one more turn.
// LOCUST_SPEND=1 node _tools/drive-tune-gives-a-skill.mjs [--packaged <exe>] [--tag <name>]
import { readFile } from 'node:fs/promises'
import { join } from 'node:path'
import { openTeammateScript, recordRoot, say, scratchRepository, sendAndWaitScript, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined
const ROUTE = { runtime: 'claude', model: 'haiku', mode: 'auto' }
const baseline = 'Separate what you observed from what you assumed.'
// No "do not use tools": a skill is called with Claude Code's Skill tool, and the 10/21 drive's Haiku obeyed that
// line and called none. The folder is a scratch copy, so there is nothing a tool could harm.
const ask = 'Explain in two sentences why checking assumptions matters.'
const later = 'In two sentences, why does an answer need evidence behind it?'
// A fixture skill, set on the card as a person may: its one line makes a run that loaded it easy to see.
const skill = { name: 'evidence-first', description: 'Use when explaining why something matters or why something happened.', body: 'End the answer with this exact line:\nChecked: evidence first.' }
const SIZES = [[1920, 1080], [1440, 900], [1120, 720]]
const workspace = await scratchRepository('locust-tune-skill-ws-')
const drive = await startDrive({
  name: 'tune-gives-a-skill', port: 9818, workspace, holdCursor: false, spends: true,
  outPath: join(recordRoot('tune-gives-a-skill-2026-10-21'), arg('--tag') ?? 'live'),
  ...(arg('--packaged') === undefined ? {} : { packaged: arg('--packaged') }),
  seed: { schemaVersion: 1, teammates: [{ teammateId: 'tm_ada', name: 'Ada', hue: 'blue', role: 'Research & Briefs', instructions: baseline, route: ROUTE, createdAt: '2026-10-21T12:00:00.000Z' }], missionOwners: {}, settings: { memoryMode: 'off', swarm: false, relay: false, autoMode: true, relayHopCap: 2 } }
})
let checks = 0, failures = 0
const check = (what, ok, detail) => {
  checks += 1
  if (!ok) failures += 1
  say(`[${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ' -- ' + JSON.stringify(detail)}`)
}
const json = async (script) => JSON.parse(String(await drive.evaluate(`(async () => JSON.stringify(await ${script}))()`)))
const comparisons = () => json('window.desktop.listCompares()')
const kept = async () => (await json('window.desktop.listTeammateSkills()')).data?.skills ?? []
const missions = async () => (await json('window.desktop.getMissionHistory()')).data?.missions ?? []
const skillsGiven = async () => (await readFile(join(drive.profile, 'locust-errors.log'), 'utf8').catch(() => '')).split('\n').filter((line) => line.includes(' skills-given: '))
const givenTo = (lines, missionId) => lines.filter((line) => line.includes(` skills-given: ${missionId} `)).join(' ')
// How many skills Claude Code listed as the run started (claude-events.ts summarizeInit keeps the count, never names).
const skillCountOf = async (missionId) => {
  const read = await json(`window.desktop.readMission(${JSON.stringify(missionId)})`)
  const started = (read.data?.mission?.events ?? []).find((event) => event.type === 'run.started')
  const count = /"skillCount":(\d+)/.exec(JSON.stringify(started ?? {}))
  return count === null ? undefined : Number(count[1])
}
const answerOf = async (missionId) => JSON.stringify((await json(`window.desktop.readMission(${JSON.stringify(missionId)})`)).data?.mission?.events ?? [])
const setField = (label, value) => drive.evaluate(`(() => {
  const field = [...document.querySelectorAll('.lc-tune input, .lc-tune textarea')].find((el) => el.getAttribute('aria-label') === ${JSON.stringify(label)})
  if (!field || field.disabled) throw new Error('editable field not found: ' + ${JSON.stringify(label)})
  const proto = field instanceof HTMLInputElement ? HTMLInputElement.prototype : HTMLTextAreaElement.prototype
  Object.getOwnPropertyDescriptor(proto, 'value').set.call(field, ${JSON.stringify(value)})
  field.dispatchEvent(new Event('input', { bubbles: true }))
})()`)
const press = (scope, name) => drive.evaluate(`(() => {
  const button = [...document.querySelectorAll(${JSON.stringify(scope + ' button')})].find((el) => el.textContent.trim() === ${JSON.stringify(name)})
  if (!button) throw new Error('no button ' + ${JSON.stringify(name)})
  button.click()
})()`)
const cardState = () => json(`Promise.resolve((() => {
  const card = document.querySelector('.lc-tune')
  const box = card.getBoundingClientRect()
  const column = (index) => card.querySelectorAll('.lc-tune__column')[index]
  const top = (el) => el?.getBoundingClientRect().top ?? NaN
  const foots = [0, 1, 2].map((index) => top(column(index)?.querySelector('.lc-tune__foot')))
  const thirds = [0, 1, 2].map((index) => top(column(index)?.children[2]))
  // Labels built from a column's title are found by their words (as drive-tune-a-teammate.mjs does).
  const labelled = (label) => [...card.querySelectorAll('[aria-label]')].find((el) => el.getAttribute('aria-label') === label)
  const group = card.querySelector('[aria-label="What it knows skill"]')
  // The boxes beside a skill fill the row (0.811 drive), and When to use it shows its sentence whole.
  const bottom = (el) => el?.getBoundingClientRect().bottom ?? NaN
  const when = card.querySelector('[aria-label="What it knows skill: when to use it"]')
  const named = (label) => card.querySelector('[aria-label=' + JSON.stringify(label) + ']')
  const run = [...card.querySelectorAll('button')].find((el) => el.textContent.trim() === 'Run all three')
  return {
    skill: group === null ? undefined : { name: named('What it knows skill name')?.value, description: named('What it knows skill: when to use it')?.value, body: named('What it knows skill: what it says')?.value },
    text: card.innerText, runEnabled: run !== undefined && !run.disabled,
    fits: box.left >= 0 && box.right <= innerWidth + 1 && box.top >= 0 && box.bottom <= innerHeight + 1,
    aligned: Math.max(...foots) - Math.min(...foots) < 2 && Math.max(...thirds) - Math.min(...thirds) < 2,
    filled: group === null || Math.abs(bottom(labelled('Current instructions')) - bottom(card.querySelector('.lc-tune__skill'))) < 2,
    whenWhole: when === null || when.scrollHeight <= when.clientHeight + 1,
    overflow: card.scrollWidth > card.clientWidth
  }
})())`)
try {
  await drive.ready()
  await drive.resize(1440, 900)
  await drive.evaluate(openTeammateScript('Ada'))
  say(String(await drive.evaluate(sendAndWaitScript(ask, { settle: false }))))
  await drive.waitFor(`(async () => { const h = await window.desktop.getMissionHistory(); return h.ok && h.data.missions.some((m) => m.phase === 'completed') })()`, { timeoutMs: 240_000, what: 'Ada’s first completed turn on Haiku' })
  check('the first Haiku turn finishes', true)
  check('a teammate with no skills of its own has none', (await kept()).length === 0)
  await drive.evaluate(`(() => {
    const field = document.querySelector('form.command-dock textarea')
    Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set.call(field, '/tune')
    field.dispatchEvent(new Event('input', { bubbles: true }))
  })()`)
  await drive.waitFor(`[...document.querySelectorAll('.lc-slash__item')].some((item) => item.textContent.includes('/tune'))`, { what: '/tune in the slash menu' })
  await drive.evaluate(`[...document.querySelectorAll('.lc-slash__item')].find((item) => item.textContent.includes('/tune')).click()`)
  await drive.waitForSelector('.lc-tune')
  await drive.waitFor(`document.querySelectorAll('.lc-tune__instructions:not(:disabled)').length === 3`, { timeoutMs: 240_000, what: 'the drafts, or editable copies of Current' })
  const draftPrompt = (await missions()).filter((m) => m.startedBy?.kind === 'side').map((m) => m.prompt).find((prompt) => prompt.includes('Suggest two short alternatives'))
  check('the draft side turn asked a Claude Code teammate for "What it knows" as a skill', draftPrompt?.includes('This teammate runs on Claude Code') === true && draftPrompt.includes('"knows":{"skill":{"name":'), draftPrompt?.slice(-900))
  const drafted = await cardState()
  if (drafted.skill === undefined) {
    say('[NOTE] Haiku drafted no skill for What it knows (instructions, or the fallback); the card is switched to a skill by hand, as a person may.')
    // What it wrote, so a reply Locust could not read says why.
    const side = (await missions()).find((m) => m.startedBy?.kind === 'side' && m.prompt.includes('Suggest two short alternatives'))
    const reply = side === undefined ? '' : (await answerOf(side.missionId)).match(/"text":"(?:[^"\\]|\\.)*"/g)?.at(-1) ?? ''
    say(`[NOTE] The draft reply, last text: ${reply.slice(0, 1200)}`)
  }
  else {
    say(`[NOTE] Haiku drafted a skill: ${JSON.stringify(drafted.skill)}`)
    check('a drafted skill reads whole, so Run all three is open', drafted.runEnabled, drafted.skill)
    // Either way round: set aside, then back, unchanged.
    await press('.lc-tune', 'Use instructions instead')
    await drive.waitFor(`[...document.querySelectorAll('.lc-tune textarea')].some((el) => el.getAttribute('aria-label') === 'What it knows instructions')`, { what: 'What it knows as instructions' })
    await press('.lc-tune', 'Write a skill instead')
    await drive.waitFor(`Boolean(document.querySelector('[aria-label="What it knows skill"]'))`, { what: 'the skill back' })
    check('Use instructions instead sets the drafted skill aside, and Write a skill instead brings it back', JSON.stringify((await cardState()).skill) === JSON.stringify(drafted.skill))
    await press('.lc-tune', 'Use instructions instead')
    await drive.waitFor(`[...document.querySelectorAll('.lc-tune textarea')].some((el) => el.getAttribute('aria-label') === 'What it knows instructions')`, { what: 'What it knows as instructions' })
  }
  check('What it knows offers Write a skill instead', (await cardState()).text.includes('Write a skill instead'))
  // The fixture skill, written on the card: the name is typed as words and taken as Claude Code names one.
  await press('.lc-tune', 'Write a skill instead')
  await drive.waitFor(`Boolean(document.querySelector('[aria-label="What it knows skill"]'))`, { what: 'the skill fields' })
  await setField('What it knows skill name', '')
  await setField('What it knows skill: when to use it', '')
  await setField('What it knows skill: what it says', '')
  await sleep(150)
  const empty = await cardState()
  check('an empty skill holds Run all three and says what it needs', !empty.runEnabled && empty.text.includes('A skill needs a name of lower-case words joined by hyphens, one sentence on when to use it, and what it says.'), empty.text.slice(-400))
  await setField('What it knows skill name', 'Evidence First')
  await setField('What it knows skill: when to use it', skill.description)
  await setField('What it knows skill: what it says', skill.body)
  await sleep(150)
  const written = await cardState()
  check('the name typed as words is taken as Claude Code names a skill', written.skill?.name === skill.name, written.skill)
  check('the written skill reads whole, and Run all three opens', written.runEnabled && !written.text.includes('A skill needs a name'), written.skill)
  for (const theme of ['dark', 'light']) {
    await drive.evaluate(`window.desktop.setTheme(${JSON.stringify(theme)})`)
    for (const [width, height] of SIZES) {
      await drive.resize(width, height); await sleep(300)
      const state = await cardState()
      check(`${theme} ${width}: the card fits, and the skill column's rows line up with the others`, state.fits && state.aligned && !state.overflow, { fits: state.fits, aligned: state.aligned, overflow: state.overflow })
      check(`${theme} ${width}: the boxes beside the skill fill its row, and When to use it shows its sentence whole`, state.filled && state.whenWhole, { filled: state.filled, whenWhole: state.whenWhole })
      await drive.capture(`${theme} tune card with a skill ${width} by ${height}`, () => JSON.stringify(state))
    }
  }
  await drive.resize(1440, 900)
  await press('.lc-tune', 'Run all three')
  await drive.waitForSelector('.lc-compare', { timeoutMs: 180_000 })
  await drive.waitFor(`document.querySelectorAll('.lc-compare__foot .lc-primarybutton:not(:disabled)').length === 3`, { timeoutMs: 600_000, what: 'all three Haiku trials' })
  const trial = (await comparisons()).data?.compares?.at(-1)
  const skilled = trial?.slots.filter((slot) => slot.route.skill !== undefined) ?? []
  check('the tune runs on Ada’s own model, blind, with the skill in one column alone', trial?.blind === true && trial.slots.length === 3 && trial.slots.every((slot) => slot.route.runtime === 'claude' && slot.route.model === ROUTE.model) && skilled.length === 1 && JSON.stringify(skilled[0].route.skill) === JSON.stringify(skill), trial?.slots.map((slot) => ({ label: slot.route.label, skill: slot.route.skill?.name })))
  check('the skill’s setup keeps Current’s instructions', skilled[0]?.route.instructions === baseline, skilled[0]?.route.instructions)
  const given = await skillsGiven()
  const skilledMission = skilled[0]?.missionIds.at(-1)
  check('that column’s run alone is handed the skill, as the teammate plugin', skilledMission !== undefined && givenTo(given, skilledMission).includes('teammate:evidence-first') && trial.slots.filter((slot) => slot !== skilled[0]).every((slot) => !givenTo(given, slot.missionIds.at(-1)).includes('teammate:')), given)
  // Claude Code itself listed one skill more in that column's run than in the others: the skill reached it.
  const counts = await Promise.all(trial.slots.map((slot) => skillCountOf(slot.missionIds.at(-1))))
  const skilledCount = counts[trial.slots.indexOf(skilled[0])]
  const otherCounts = counts.filter((_, at) => at !== trial.slots.indexOf(skilled[0]))
  check('Claude Code lists one skill more in that column’s run than in the others', skilledCount !== undefined && otherCounts.every((count) => count !== undefined && skilledCount === count + 1), counts)
  const recorded = skilledMission === undefined ? '' : await answerOf(skilledMission)
  const called = /"name":"Skill"/.test(recorded) && recorded.includes('evidence-first')
  say(`[NOTE] The skill's column ${called ? 'called' : 'did not call'} the skill, and its answer ${recorded.includes('Checked: evidence first.') ? 'carries' : 'does not carry'} the skill's line. Whether a run calls a skill is the model's call.`)
  check('the column heads stay Setup A, B and C before Keep', await drive.evaluate(`[...document.querySelectorAll('.lc-compare__head')].every((head) => /^Setup [ABC]/.test(head.innerText.trim()))`))
  check('nothing is kept for Ada before Keep', (await kept()).length === 0)
  await drive.capture('the blind tune with a skill column', () => drive.evaluate(`document.querySelector('.lc-compare').innerText`))
  const index = trial.slots.indexOf(skilled[0])
  await drive.evaluate(`document.querySelectorAll('.lc-compare__foot')[${index}].querySelector('button.lc-primarybutton').click()`)
  await drive.waitFor(`(document.querySelector('.lc-tuneoffer')?.innerText ?? '').includes('Use this skill for Ada')`, { timeoutMs: 60_000, what: 'the skill offered on the kept strip' })
  const strip = () => drive.evaluate(`document.querySelector('.lc-tuneoffer')?.innerText ?? ''`)
  const offered = String(await strip())
  check('the kept strip offers the kept setup’s skill, and neither instructions nor a combine', offered.includes('The setup you kept adds a skill, evidence-first.') && !offered.includes('Use these instructions') && !offered.includes('Combine both edits'), offered)
  check('Keep alone keeps no skill', (await kept()).length === 0)
  await drive.evaluate(`[...document.querySelectorAll('.lc-tuneoffer summary')].find((el) => el.textContent.trim() === 'Review the skill').click()`)
  check('the skill can be read before it is used', await drive.evaluate(`[...document.querySelectorAll('.lc-tuneoffer pre')].some((pre) => pre.textContent === ${JSON.stringify(`When to use it: ${skill.description}\n\n${skill.body}`)})`))
  for (const theme of ['dark', 'light']) {
    await drive.evaluate(`window.desktop.setTheme(${JSON.stringify(theme)})`)
    for (const [width, height] of SIZES) {
      await drive.resize(width, height); await sleep(300)
      await drive.capture(`${theme} kept skill offered ${width} by ${height}`, () => strip())
    }
  }
  await drive.resize(1440, 900)
  await press('.lc-tuneoffer', 'Use this skill for Ada')
  await drive.waitFor(`(document.querySelector('.lc-tuneoffer')?.innerText ?? '').includes('Ada has this skill from the next turn.')`, { what: 'the skill saved' })
  const saved = await kept()
  check('Use this skill keeps it for Ada, from this tune', saved.length === 1 && saved[0].teammateId === 'tm_ada' && saved[0].name === skill.name && saved[0].compareId === trial.compareId, saved)
  const file = await readFile(join(drive.profile, 'teammate-skills', 'tm_ada', skill.name, 'SKILL.md'), 'utf8').catch(() => '')
  check('it is kept as Claude Code reads a skill', file === `---\nname: ${skill.name}\ndescription: ${JSON.stringify(skill.description)}\n---\n\n${skill.body}\n`, file)
  await drive.capture('the kept skill takes effect next turn', () => strip())
  // Keep goes back to the conversation itself; from the comparison, its own Back does.
  if (await drive.evaluate(`Boolean(document.querySelector('.lc-compare'))`)) await press('.lc-compare', 'Back to the conversation')
  await drive.waitFor(`!document.querySelector('.lc-compare')`, { what: 'the conversation' })
  // Ada's own first turn, not one of the trials: the same words, outside the comparison.
  const trialRuns = new Set(trial.slots.flatMap((slot) => slot.missionIds))
  const firstTurn = (await missions()).find((m) => m.prompt === ask && m.startedBy === undefined && !trialRuns.has(m.missionId))
  say(String(await drive.evaluate(sendAndWaitScript(later, { settle: false }))))
  await drive.waitFor(`(async () => { const h = await window.desktop.getMissionHistory(); return h.ok && h.data.missions.some((m) => m.prompt === ${JSON.stringify(later)} && m.phase === 'completed') })()`, { timeoutMs: 240_000, what: 'Ada’s next turn' })
  const next = (await missions()).find((m) => m.prompt === later)
  check('Ada’s next turn is handed the kept skill', next !== undefined && givenTo(await skillsGiven(), next.missionId).includes('teammate:evidence-first'), next === undefined ? undefined : givenTo(await skillsGiven(), next.missionId))
  const before = firstTurn === undefined ? undefined : await skillCountOf(firstTurn.missionId)
  const after = next === undefined ? undefined : await skillCountOf(next.missionId)
  check('Claude Code lists one skill more in that turn than in Ada’s first, before the tune', before !== undefined && after === before + 1, { before, after })
  const nextRecord = next === undefined ? '' : await answerOf(next.missionId)
  say(`[NOTE] The next turn ${/"name":"Skill"/.test(nextRecord) && nextRecord.includes('evidence-first') ? 'called' : 'did not call'} the skill, and ${nextRecord.includes('Checked: evidence first.') ? 'carries' : 'does not carry'} its line.`)
  // A new turn draws the strip afresh: a skill used from this tune is not offered again.
  check('once used, the kept strip offers the skill no more', !String(await strip()).includes('Use this skill for Ada'), String(await strip()))
  /*
   * A TEAMMATE'S SETUP (0.811): /setup in Ada's conversation -- what shapes her turns, read fresh, and what this
   * conversation's last turn loaded, as Claude Code said when it started. It changes nothing.
   */
  await drive.evaluate(`(() => {
    const field = document.querySelector('form.command-dock textarea')
    Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set.call(field, '/setup')
    field.dispatchEvent(new Event('input', { bubbles: true }))
  })()`)
  await drive.waitFor(`[...document.querySelectorAll('.lc-slash__item')].some((item) => item.textContent.includes('/setup'))`, { what: '/setup in the slash menu' })
  await drive.evaluate(`[...document.querySelectorAll('.lc-slash__item')].find((item) => item.textContent.includes('/setup')).click()`)
  await drive.waitFor(`(document.querySelector('.lc-setup')?.innerText ?? '').includes('ITS LAST TURN HERE')`, { timeoutMs: 30_000, what: 'Ada’s setup, read' })
  const setupText = String(await drive.evaluate(`document.querySelector('.lc-setup').innerText`))
  check('/setup says the model, then every layer, with her kept skill and what her last turn loaded', ['Claude Code / Haiku', 'EVERY TURN', 'Its instructions', `${String(baseline.length)} characters`, 'WHEN THE WORK MATCHES', skill.name, 'ONLY WHEN YOU TUNE IT', 'ITS LAST TURN HERE', `${String(after)} skills`].every((part) => setupText.includes(part)), setupText)
  check('/setup changed nothing', (await kept()).length === 1)
  for (const theme of ['dark', 'light']) {
    await drive.evaluate(`window.desktop.setTheme(${JSON.stringify(theme)})`)
    for (const [width, height] of SIZES) {
      await drive.resize(width, height); await sleep(300)
      const fits = await drive.evaluate(`(() => { const box = document.querySelector('.lc-setup').getBoundingClientRect(); return box.top >= 0 && box.bottom <= innerHeight + 1 && box.left >= 0 && box.right <= innerWidth + 1 })()`)
      check(`${theme} ${width}: the setup card fits the window`, fits === true)
      await drive.capture(`${theme} setup ${width} by ${height}`, () => drive.evaluate(`document.querySelector('.lc-setup').innerText`))
    }
  }
  await drive.resize(1440, 900)
  await press('.lc-setup', 'Edit teammate')
  await drive.waitFor(`!document.querySelector('.lc-setup') && Boolean(document.querySelector('.lc-dialog'))`, { what: 'Edit teammate from /setup' })
  check('Edit teammate opens from /setup', String(await drive.evaluate(`document.querySelector('.lc-dialog')?.innerText ?? ''`)).includes('Edit teammate'))
  await drive.evaluate(`document.querySelector('.lc-dialog__close').click()`)
  await drive.waitFor(`!document.querySelector('.lc-dialog')`, { what: 'Edit teammate closed' })
  await drive.capture('the next turn, with the skill kept', () => drive.evaluate(`document.querySelector('.lc-thread')?.innerText.slice(-1200) ?? ''`))
  await drive.evaluate(`(() => {
    const face = [...document.querySelectorAll('.lc-faces__one')].find((el) => (el.getAttribute('aria-label') ?? '').startsWith('Ada — '))
    face.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, clientX: 80, clientY: 160 }))
  })()`)
  await drive.waitFor(`[...document.querySelectorAll('.lc-context__item')].some((item) => item.querySelector('.lc-context__label')?.textContent.trim() === 'Edit')`, { what: 'Edit in the teammate row menu' })
  await drive.evaluate(`[...document.querySelectorAll('.lc-context__item')].find((item) => item.querySelector('.lc-context__label')?.textContent.trim() === 'Edit').click()`)
  const skillsField = `[...document.querySelectorAll('.lc-field--lessons')].find((el) => el.querySelector('.lc-fieldlabel')?.textContent.trim() === 'Skills')`
  await drive.waitFor(`Boolean(${skillsField})`, { what: 'Ada’s skills in Edit teammate' })
  const listed = String(await drive.evaluate(`${skillsField}.innerText`))
  check('Edit teammate lists the kept skill, when it is used, and Remove', listed.includes(skill.name) && listed.includes(skill.description) && listed.includes('Remove') && listed.includes('A run loads one only when the work matches when to use it.'), listed)
  for (const theme of ['dark', 'light']) {
    await drive.evaluate(`window.desktop.setTheme(${JSON.stringify(theme)})`)
    for (const [width, height] of SIZES) {
      await drive.resize(width, height); await sleep(300)
      await drive.evaluate(`${skillsField}?.scrollIntoView({ block: 'center' })`)
      await sleep(100)
      await drive.capture(`${theme} edit teammate skills ${width} by ${height}`, () => drive.evaluate(`${skillsField}.innerText`))
    }
  }
  await drive.evaluate(`${skillsField}.querySelector('button').click()`)
  await drive.waitFor(`!(${skillsField})`, { what: 'the skill removed from Edit teammate' })
  check('Remove takes the skill from Ada, and its folder with it', (await kept()).length === 0 && (await readFile(join(drive.profile, 'teammate-skills', 'tm_ada', skill.name, 'SKILL.md'), 'utf8').then(() => false, () => true)))
  await drive.capture('edit teammate with no skills left', () => drive.evaluate(`document.querySelector('.lc-dialog')?.innerText.slice(0, 1500) ?? ''`))
  await drive.evaluate(`document.querySelector('.lc-dialog__close').click()`)
  check('the frames have no renderer errors', drive.record.every((frame) => frame.errors.length === 0))
} catch (error) {
  check('the drive completes', false, error instanceof Error ? error.message : String(error))
  await drive.capture('drive failure', () => drive.evaluate(`document.body.innerText`))
} finally {
  say(`TUNE GIVES A SKILL: ${checks - failures}/${checks} passed`)
  await drive.finish({ intro: `Claude Code on ${ROUTE.model}, in Auto: one turn, /tune with What it knows as a skill (the fixture skill is written on the card), three trials, Keep, Use this skill, Ada's next turn, and Remove in Edit teammate. The skill is a fixture, not a claim that it makes a better answer.` })
  if (failures > 0) process.exitCode = 1
}
