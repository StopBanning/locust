// Drive (0.833): A TEAMMATE'S KEY REACHES ITS RUNS, on a real agent. The person keeps a key for Kit in Kit's own
// settings -- a name, a masked box, Add -- and Kit's turn runs a command that prints whether the key is there, its
// length and the key itself, as a careless command would. The answer must say PRESENT with the right length, and the
// value must be nowhere: not in the window, not in the record Locust keeps -- its name in brackets instead. Then Kit is
// told it needs a key it does not have, asks for it in the way its brief says, and the card under its answer takes it.
//
//   LOCUST_SPEND=1 node _tools/drive-a-teammates-key-reaches-its-runs.mjs --runtime claude|cursor|gemini|copilot|opencode [--packaged <exe>]
//
// Claude spends one or two Haiku turns; Cursor keeps the person's own model (choosing one rewrites their default).
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { FREE_ROUTE, openTeammateScript, recordRoot, say, sendAndWaitScript, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const runtime = arg('--runtime') ?? 'claude'
const tag = arg('--tag') ?? `${runtime}-${packaged === undefined ? 'local' : 'packaged'}`
// Made up, and put together here so no file holds a whole key-shaped value.
const VALUE = ['probe', 'Locust', 'made', 'up', String(Date.now()).slice(-6)].join('_')
const ROUTES = {
  claude: { runtime: 'claude', model: 'haiku', mode: 'auto' },
  cursor: { runtime: 'cursor', model: 'account-default', mode: 'auto' },
  gemini: { runtime: 'gemini', model: 'account-default', mode: 'auto' },
  copilot: { runtime: 'copilot', model: 'account-default', mode: 'auto' },
  opencode: { ...FREE_ROUTE, mode: 'auto' }
}
const workspace = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-keys-ws-'))
const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `a-teammates-key-reaches-its-runs-${tag}`, port: 9786, workspace, spends: runtime !== 'opencode',
  outPath: join(recordRoot('a-teammates-key-reaches-its-runs-2026-10-24'), tag),
  seed: {
    schemaVersion: 1,
    teammates: [{ teammateId: 'tm_kit00000000000000000000', name: 'Kit', hue: 'teal', role: 'Custom', roleTitle: 'Payments', createdAt: '2026-10-24T05:00:00.000Z', route: ROUTES[runtime] }],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: true }
  }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 500)}`}`)
}
const THREAD = `(document.querySelector('.lc-thread')?.innerText ?? '').replace(/\\s+/g, ' ')`
const record = async () => String(await drive.evaluate(`window.desktop.getMissionHistory().then(async (r) => {
  const ids = r.ok ? r.data.missions.map((m) => m.missionId) : []
  const all = []
  for (const id of ids) { const one = await window.desktop.getMission?.(id).catch(() => undefined); all.push(JSON.stringify(one ?? {})) }
  return JSON.stringify(r) + all.join('')
})`))

try {
  await drive.capture('launch: Kit, no keys', () => drive.ready())
  await drive.resize(1215, 860)

  // The person keeps a key for Kit in Kit's own settings.
  const field = String(await drive.capture('Kit’s settings: the Keys field', () => drive.evaluate(`(async () => {
    document.querySelector('.lc-faces__team')?.click()
    for (let i = 0; i < 30 && document.querySelectorAll('.lc-rostercard').length === 0; i += 1) await new Promise((r) => setTimeout(r, 150))
    const card = [...document.querySelectorAll('.lc-rostercard')].find((c) => /Kit/.test(c.innerText))
    const button = card && [...card.querySelectorAll('button')].find((b) => /Edit/.test(b.textContent ?? '') || /Edit/.test(b.getAttribute('aria-label') ?? ''))
    ;(button ?? card)?.click()
    for (let i = 0; i < 20 && !document.querySelector('.lc-dialog'); i += 1) await new Promise((r) => setTimeout(r, 200))
    const name = document.querySelector('.lc-dialog input[aria-label="The key’s name"]')
    name?.scrollIntoView({ block: 'center' })
    return name ? (name.closest('.lc-field')?.innerText ?? '').replace(/\\s+/g, ' ') : 'no Keys field'
  })()`)))
  check('Kit’s settings have a Keys field that says where keys go', /keys/i.test(field) && /Kept encrypted on this computer, for Kit alone/.test(field), field)
  await drive.evaluate(`(() => { document.querySelector('.lc-dialog input[aria-label="The key’s name"]')?.focus(); return 1 })()`)
  await drive.send('Input.insertText', { text: 'PROBE_KEY' })
  await drive.evaluate(`(() => { document.querySelector('.lc-dialog input[aria-label="The key"]')?.focus(); return 1 })()`)
  await drive.send('Input.insertText', { text: VALUE })
  const added = String(await drive.capture('Add: PROBE_KEY kept for Kit, by name only', () => drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('.lc-dialog button')].find((b) => b.innerText.trim() === 'Add')?.click()
    for (let i = 0; i < 30 && !/PROBE_KEY\\s*Remove/.test(document.querySelector('.lc-dialog')?.innerText ?? ''); i += 1) await new Promise((r) => setTimeout(r, 150))
    return (document.querySelector('.lc-dialog')?.innerText ?? '').replace(/\\s+/g, ' ')
  })()`)))
  check('the key is listed by name, and its value is nowhere in the window', /PROBE_KEY Remove/.test(added) && !String(await drive.evaluate('document.body.innerText')).includes(VALUE), added.slice(added.indexOf('Keys'), added.indexOf('Keys') + 200))
  await drive.evaluate(`(async () => { ;[...document.querySelectorAll('.lc-dialog button')].find((b) => /^(Cancel|Close)$/.test(b.innerText.trim()) || b.getAttribute('aria-label') === 'Close')?.click(); await new Promise((r) => setTimeout(r, 500)); return 1 })()`)

  // Kit's turn runs a command that prints the key, as a careless one would.
  say(`  ${String(await drive.evaluate(openTeammateScript('Kit')))}`)
  const probe = `node -e "const v=process.env.PROBE_KEY||'';console.log(v?'PRESENT '+v.length+' '+v:'ABSENT')"`
  const sent = String(await drive.capture('Kit’s turn: a command that prints the key', () => drive.evaluate(sendAndWaitScript(`Run this exact shell command, then reply with only what it printed, nothing else:\n\n${probe}`, { waitSeconds: 300 }))))
  const thread = String(await drive.evaluate(THREAD))
  check(`on ${runtime}, the command saw the key, at its full length`, new RegExp(`PRESENT ${String(VALUE.length)}\\b`).test(thread), sent.slice(-200))
  // The agent may hold it back itself ("[value redacted]"); where it does not, Locust puts the key's name in brackets.
  check('the key’s value is nowhere in the window', !String(await drive.evaluate('document.body.innerText')).includes(VALUE), thread.slice(-300))
  const kept = await record()
  check('the key’s value is nowhere in what Locust keeps of the turn', !kept.includes(VALUE), String(kept.length))

  // Told it needs a key it does not have, Kit asks for it as its brief says, and the card takes it.
  await drive.evaluate(sendAndWaitScript('You need a key named WEATHER_KEY to call the weather service, and you do not have it. Ask me for it the way your instructions say, then stop. Do nothing else.', { waitSeconds: 300 }))
  const card = String(await drive.capture('Kit asks for WEATHER_KEY: the card under its answer', () => drive.evaluate(`(() => { const card = document.querySelector('.lc-keycard'); card?.scrollIntoView({ block: 'center' }); return card ? card.innerText.replace(/\\s+/g, ' ') : 'no card' })()`)))
  check('Kit asked for the key in its block, and the card offers a masked box', /Kit asks for a key: WEATHER_KEY/.test(card) && String(await drive.evaluate(`document.querySelector('.lc-keycard input')?.type ?? 'none'`)) === 'password', card)
  check('the block’s tags are not shown in the answer', !/<\/?locust-key/.test(String(await drive.evaluate(THREAD))))
  if (card !== 'no card') {
    await drive.evaluate(`(() => { document.querySelector('.lc-keycard input')?.focus(); return 1 })()`)
    await drive.send('Input.insertText', { text: `${VALUE}_weather` })
    const keptCard = String(await drive.capture('Keep: Kit has WEATHER_KEY', () => drive.evaluate(`(async () => {
      ;[...document.querySelectorAll('.lc-keycard button')].find((b) => /^Keep for Kit/.test(b.innerText.trim()))?.click()
      for (let i = 0; i < 30 && !/Kit has WEATHER_KEY/.test(document.querySelector('.lc-keycard')?.innerText ?? ''); i += 1) await new Promise((r) => setTimeout(r, 150))
      return (document.querySelector('.lc-keycard')?.innerText ?? '').replace(/\\s+/g, ' ')
    })()`)))
    check('Keep says Kit has it, and the box is gone', /Kit has WEATHER_KEY/.test(keptCard) && String(await drive.evaluate(`document.querySelector('.lc-keycard input') === null`)) === 'true', keptCard)
    const names = JSON.parse(String(await drive.evaluate(`window.desktop.teammateKeys.list('tm_kit00000000000000000000').then((r) => JSON.stringify(r.names))`)))
    check('Kit’s keys are the two, by name', JSON.stringify(names) === JSON.stringify(['PROBE_KEY', 'WEATHER_KEY']), JSON.stringify(names))
  }
  for (const [width, height] of [[960, 720], [760, 640]]) {
    await drive.resize(width, height)
    await sleep(600)
    await drive.capture(`the card, at ${String(width)} by ${String(height)}`, () => drive.evaluate(`(() => { document.querySelector('.lc-keycard')?.scrollIntoView({ block: 'center' }); return 1 })()`))
  }
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. Kit on ${runtime} (${JSON.stringify(ROUTES[runtime])}).`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
