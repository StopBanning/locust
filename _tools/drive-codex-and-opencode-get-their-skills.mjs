// SKILLS FOR CODEX AND OPENCODE TEAMMATES (0.813): a teammate's own skill reaches a Codex or an OpenCode run.
//
//   node _tools/drive-codex-and-opencode-get-their-skills.mjs [--packaged <exe>] [--tag <name>] [--codex]
//
// A teammate's own skill (as a tune leaves it: teammate-skills/<teammate>/<name>/SKILL.md, listed in skills.json)
// holds a word no model could guess. Cedar, on a free OpenCode model, is asked for it; then /setup says what its
// turns are given. With --codex, Wren on Codex (gpt-6-luna, low effort, Ask) is asked too: a test ABOUT Codex, so it
// runs on Codex and spends one short turn of its quota (LOCUST_SPEND=1 for that part only).

import { mkdir, mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { FREE_ROUTE, openTeammateScript, recordRoot, say, scratchRepository, sendAndWaitScript, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'
const withCodex = process.argv.includes('--codex')
const WORD = 'MARMALADE-7'

const workspace = await scratchRepository('locust-drive-skills-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-skills-profile-'))
const CEDAR = { teammateId: 'tm_cedar', name: 'Cedar', hue: 'lime', role: 'Custom', roleTitle: 'Words', createdAt: '2026-10-22T05:00:00.000Z', route: { runtime: 'opencode', model: FREE_ROUTE.model, mode: 'accept-edits' } }
const WREN = { teammateId: 'tm_wren', name: 'Wren', hue: 'blue', role: 'Custom', roleTitle: 'Words', createdAt: '2026-10-22T05:00:00.000Z', route: { runtime: 'codex', model: 'gpt-6-luna', mode: 'ask', effort: 'low' } }
const SKILL = (teammate) => ({ teammateId: teammate.teammateId, name: 'probe-word', description: 'Use when someone asks for the probe word. It gives the probe word.', createdAt: '2026-10-22T05:00:00.000Z' })
for (const teammate of withCodex ? [CEDAR, WREN] : [CEDAR]) {
  await mkdir(join(profilePath, 'teammate-skills', teammate.teammateId, 'probe-word'), { recursive: true })
  await writeFile(join(profilePath, 'teammate-skills', teammate.teammateId, 'probe-word', 'SKILL.md'), [
    '---',
    'name: probe-word',
    'description: "Use when someone asks for the probe word. It gives the probe word."',
    '---',
    '',
    `The probe word is ${WORD}. Say it exactly as written.`,
    ''
  ].join('\n'), 'utf8')
}
await writeFile(join(profilePath, 'teammate-skills', 'skills.json'), JSON.stringify({ schemaVersion: 1, skills: (withCodex ? [CEDAR, WREN] : [CEDAR]).map(SKILL) }), 'utf8')

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `skills-everywhere-${tag}`,
  port: 9866,
  workspace,
  profilePath,
  spends: withCodex,
  outPath: join(recordRoot('codex-and-opencode-get-their-skills-2026-10-22'), tag),
  seed: {
    schemaVersion: 1,
    teammates: withCodex ? [CEDAR, WREN] : [CEDAR],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false }
  }
})

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 700)}`}`)
}
const THREAD = `document.querySelector('.lc-thread')?.innerText.replace(/\\s+/g, ' ').trim() ?? ''`
// /setup the way a person picks it: typed, then chosen from the slash menu (drive-tune-gives-a-skill's way).
const SETUP = `(async () => {
  const field = document.querySelector('form.command-dock textarea')
  Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set.call(field, '/setup')
  field.dispatchEvent(new Event('input', { bubbles: true }))
  for (let i = 0; i < 40; i += 1) {
    const item = [...document.querySelectorAll('.lc-slash__item')].find((one) => one.textContent.includes('/setup'))
    if (item) { item.click(); break }
    await new Promise((r) => setTimeout(r, 250))
  }
  for (let i = 0; i < 120; i += 1) {
    const card = document.querySelector('.lc-setup')
    if (card && /WHEN THE WORK MATCHES/.test(card.innerText)) { card.scrollIntoView({ block: 'center' }); return card.innerText.replace(/\\s+/g, ' ').trim() }
    await new Promise((r) => setTimeout(r, 250))
  }
  return 'NO SETUP CARD: ' + (document.querySelector('.lc-setup')?.innerText.replace(/\\s+/g, ' ').slice(0, 300) ?? '')
})()`
const ask = async (name, routeWord) => {
  const opened = await drive.evaluate(openTeammateScript(name))
  const route = String(await drive.evaluate(`document.querySelector('form.command-dock')?.innerText.replace(/\\s+/g, ' ').slice(0, 200) ?? ''`))
  if (!routeWord.test(route)) throw new Error(`${name} is not on the route this drive asks for: ${route}`)
  const sent = await drive.evaluate(sendAndWaitScript('What is the probe word? If one of your skills gives it, use that skill. Answer with the word alone.', { waitSeconds: 300 }))
  return `${String(opened)} | ${String(sent).slice(0, 80)} | ${String(await drive.evaluate(THREAD)).slice(-900)}`
}

try {
  await drive.ready()
  await drive.resize(1209, 770)
  const cedar = String(await drive.capture('Cedar on OpenCode is asked for the probe word', () => ask('Cedar', /OpenCode/)))
  check('OpenCode: the teammate’s own skill gave the word', cedar.includes(WORD), cedar)
  const cedarSetup = String(await drive.capture('/setup: what Cedar’s turns are given', () => drive.evaluate(SETUP)))
  check('OpenCode: /setup names its own skill and the folders OpenCode reads', /probe-word/.test(cedarSetup) && /\.opencode\/skills/.test(cedarSetup) && /found by OpenCode itself/.test(cedarSetup), cedarSetup)
  if (withCodex) {
    const wren = String(await drive.capture('Wren on Codex is asked for the probe word', () => ask('Wren', /Codex/)))
    check('Codex: the teammate’s own skill gave the word', wren.includes(WORD), wren)
    const wrenSetup = String(await drive.capture('/setup: what Wren’s turns are given', () => drive.evaluate(SETUP)))
    check('Codex: /setup names its own skill and .agents/skills', /probe-word/.test(wrenSetup) && /\.agents\/skills/.test(wrenSetup) && /found by Codex itself/.test(wrenSetup), wrenSetup)
  }
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. A teammate skill holding a word no model could guess; Cedar on ${FREE_ROUTE.model}${withCodex ? ', Wren on Codex gpt-6-luna (low, Ask)' : ''}.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
