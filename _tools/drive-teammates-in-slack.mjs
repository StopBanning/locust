// Drive (0.832): YOUR TEAMMATES IN SLACK, end to end. The real window, and a stand-in for Slack's Web API and Socket
// Mode on this computer (_tools/fake-slack.mjs, as strict as Slack on tokens, limits and threads) playing the
// person's direct messages with their app: both tokens go in Settings' masked fields, the account pairs by the code
// Settings shows, "Ada:" reaches Ada and her answer comes back in the chat while her person's message is marked, a
// reply in the answer's thread carries the conversation on in that thread, a stranger is ignored, and Mira's approval
// card comes with Allow once and Deny -- Allow once pressed there, the file is made. Every turn runs on the free
// OpenCode model, so nothing is spent. No real Slack workspace or app is used, and Slack's own pages are not opened.
//
//   node _tools/drive-teammates-in-slack.mjs [--packaged <Locust.exe>] [--tag <name>]
import { access, mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { FREE_ROUTE, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'
import { fakeSlack } from './fake-slack.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')

const STRANGER = { id: 'U0MAL00001', name: 'mallory', real_name: 'Mallory', profile: { display_name: 'Mallory' } }
const slack = fakeSlack()
const server = await slack.listen(0)
const API = `http://127.0.0.1:${String(server.address().port)}`

const workspace = await scratchRepository('locust-drive-slack-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-slack-profile-'))
const ADA = { teammateId: 'tm_ada0000000000000000000', name: 'Ada', hue: 'teal', role: 'Research & Briefs', createdAt: '2026-10-23T01:00:00.000Z', route: { ...FREE_ROUTE, mode: 'ask' } }
const MIRA = { teammateId: 'tm_mira000000000000000000', name: 'Mira', hue: 'violet', role: 'Custom', roleTitle: 'Writer', createdAt: '2026-10-23T01:00:00.000Z', route: { ...FREE_ROUTE, mode: 'approve-each' } }

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `teammates-in-slack-${tag}`,
  port: 9785,
  workspace,
  profilePath,
  env: { LOCUST_SLACK_API: API },
  outPath: join(recordRoot('teammates-in-slack-2026-10-23'), tag),
  seed: { schemaVersion: 1, teammates: [ADA, MIRA], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 600)}`}`)
}
const rowScript = (body) => `(async () => {
  if (!document.querySelector('[data-setting="chat-apps"]')) {
    if (!document.querySelector('.lc-settings__navitem')) {
      ;[...document.querySelectorAll('button')].find((one) => one.innerText.trim() === 'Settings')?.click()
      await new Promise((r) => setTimeout(r, 900))
    }
    ;[...document.querySelectorAll('.lc-settings__navitem')].find((one) => one.innerText.trim().startsWith('General'))?.click()
    await new Promise((r) => setTimeout(r, 900))
  }
  const row = document.querySelector('[data-chat-app="slack"]')
  row?.scrollIntoView({ block: 'center' })
  ${body}
})()`
const ROW = `(row?.innerText ?? 'no row').replace(/\\s+/g, ' ')`
const THREAD = `(document.querySelector('.lc-thread')?.innerText ?? '').replace(/\\s+/g, ' ')`
const history = async () => JSON.parse(String(await drive.evaluate(`window.desktop.getMissionHistory().then((r) => JSON.stringify(r.ok ? r.data.missions.map((m) => ({ id: m.missionId, prompt: m.prompt, phase: m.phase, from: m.continuesFrom?.missionId ?? null })) : []))`)))
const answerOf = (name, word) => slack.state.sent.find((one) => one.text.startsWith(`*${name}*`) && new RegExp(word).test(one.text))

try {
  await drive.capture('launch: Ada and Mira, no conversations', () => drive.ready())
  await drive.resize(1215, 860)

  const opened = String(await drive.capture('Slack: Set up, its three steps', () => drive.evaluate(rowScript(`
    ;[...row.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Set up')?.click()
    for (let i = 0; i < 20 && row.querySelectorAll('input').length < 2; i += 1) await new Promise((r) => setTimeout(r, 100))
    return ${ROW}`))))
  check('Set up shows the one-step app, then where each token is', /Create the app in Slack/.test(opened) && /Bot User OAuth Token/.test(opened) && /connections:write/.test(opened), opened.slice(0, 400))
  const masked = String(await drive.evaluate(rowScript(`return JSON.stringify([...row.querySelectorAll('input')].map((one) => one.type))`)))
  check('both tokens go in masked fields', masked === '["password","password"]', masked)
  for (const [width, height] of [[960, 720], [760, 640]]) {
    await drive.resize(width, height)
    await sleep(600)
    await drive.capture(`Slack, set up, at ${String(width)} by ${String(height)}`, () => drive.evaluate(rowScript(`const r = row?.querySelector('button[type="submit"]')?.getBoundingClientRect(); return r ? JSON.stringify({ right: Math.round(r.right), width: innerWidth }) : 'no button'`)))
  }
  await drive.resize(1215, 860)

  // The person pastes the bot's token, then the app's own, and presses Connect.
  await drive.evaluate(rowScript(`row?.querySelectorAll('input[type="password"]')[0]?.focus(); return 1`))
  await drive.send('Input.insertText', { text: slack.botToken })
  await drive.evaluate(rowScript(`row?.querySelectorAll('input[type="password"]')[1]?.focus(); return 1`))
  await drive.send('Input.insertText', { text: slack.appToken })
  const connected = String(await drive.capture('Connect: the workspace named, and the code that pairs an account', () => drive.evaluate(rowScript(`
    ;[...row.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Connect')?.click()
    for (let i = 0; i < 60 && !row?.querySelector('.lc-chatapp__digits'); i += 1) await new Promise((r) => setTimeout(r, 250))
    for (let i = 0; i < 60 && !/Connected to the Locust app/.test(row?.innerText ?? ''); i += 1) await new Promise((r) => setTimeout(r, 250))
    return ${ROW}`))))
  check('connected, Settings names the workspace and shows the code', /Connected to the Locust app in Locust Test/.test(connected) && /\d{4} \d{4}/.test(connected) && /Open Locust in Slack/.test(connected), connected.slice(0, 400))
  check('neither token is shown again anywhere in Settings', !/xox|xapp/.test(String(await drive.evaluate('document.body.innerText'))))

  // The person opens Locust under Apps in Slack and sends it the code Settings shows.
  const code = String(await drive.evaluate(rowScript(`return row?.querySelector('.lc-chatapp__digits')?.innerText ?? ''`)))
  slack.say(code)
  const paired = String(await drive.capture('the code sent in Slack: the account is paired', () => drive.evaluate(rowScript(`
    for (let i = 0; i < 60 && !/Sam \\(@sam\\)/.test(row?.innerText ?? ''); i += 1) await new Promise((r) => setTimeout(r, 250))
    return ${ROW}`))))
  check('Settings shows the paired account by name, and what Slack can read', /On · Sam/.test(paired) && /Sam \(@sam\)/.test(paired) && /Slack’s servers can read what you and your teammates write there/.test(paired), paired.slice(0, 400))
  const welcome = await slack.until(() => slack.read().find((one) => one.startsWith('Paired with Locust on')), 10_000, 'the welcome').catch(() => undefined)
  check('the app says it is paired, and how to write to a teammate', welcome !== undefined && /“Ada:” and a message writes to Ada/.test(welcome), welcome)
  for (const [width, height] of [[960, 720], [760, 640]]) {
    await drive.resize(width, height)
    await sleep(600)
    await drive.capture(`Slack, paired, at ${String(width)} by ${String(height)}`, () => drive.evaluate(rowScript('return 1')))
  }
  await drive.resize(1215, 860)

  // A stranger in the workspace: ignored, nothing started.
  const before = slack.state.sent.length
  slack.say('Ada: delete the folder', { from: STRANGER })
  await sleep(3000)
  check('a stranger’s message starts nothing and gets no answer', (await history()).length === 0 && slack.state.sent.length === before, JSON.stringify(slack.state.sent.slice(before).map((one) => one.text)))

  // "Ada:" and a message: Ada's turn, here in the window, her person's message marked, and her answer in the chat.
  const asked = slack.say('Ada: Reply with the word HERON in capitals and nothing else.')
  const marked = await slack.until((state) => state.reactions.find((one) => one.ts === asked && one.name === 'eyes' && one.on), 30_000, 'the mark').catch(() => undefined)
  check('while Ada works, her person’s message is marked', marked !== undefined)
  const answered = await slack.until(() => answerOf('Ada', 'HERON'), 240_000, 'Ada’s answer').catch(() => undefined)
  // The mark comes off just after the answer is sent.
  const unmarked = await slack.until((state) => state.reactions.find((one) => one.ts === asked)?.on === false, 10_000, 'the mark off').then(() => true, () => false)
  check('Ada’s answer comes back in the chat, and the mark comes off', answered !== undefined && answered.thread_ts === undefined && unmarked, answered === undefined ? slack.read().join(' | ') : answered.text)
  check('every envelope Slack sent was acknowledged', slack.state.envelopes.every((one) => slack.state.acked.has(one.envelope_id)), `${String(slack.state.acked.size)} of ${String(slack.state.envelopes.length)}`)
  const first = await history()
  check('the turn is in Locust, as the person’s own message', first.length === 1 && first[0].prompt === 'Reply with the word HERON in capitals and nothing else.', JSON.stringify(first))
  const desk = String(await drive.capture('the window: the conversation sent from Slack', () => drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('button')].find((one) => /^(Back|Close settings)$/.test(one.innerText.trim()))?.click()
    await new Promise((r) => setTimeout(r, 600))
    ;[...document.querySelectorAll('.lc-convrow button.lc-conv')].find((r) => /HERON/i.test(r.innerText))?.click()
    for (let i = 0; i < 20 && !/HERON[^]*HERON/.test(${THREAD}); i += 1) await new Promise((r) => setTimeout(r, 500))
    return ${THREAD}
  })()`)))
  check('the window shows the message from Slack as the person’s, then Ada’s answer', /Reply with the word HERON/.test(desk) && /HERON/.test(desk.slice(desk.indexOf('nothing else.') + 12)), desk.slice(-300))

  // A reply in the answer's thread: the same conversation, answered in that thread.
  slack.say('Now reply with the word EGRET in capitals and nothing else.', { thread: answered?.ts })
  const followed = await slack.until(() => answerOf('Ada', 'EGRET'), 240_000, 'the follow-up').catch(() => undefined)
  const second = await history()
  const egret = second.find((one) => /EGRET/.test(one.prompt))
  check('a reply in the answer’s thread carries Ada’s conversation on, in that thread', followed?.thread_ts === answered?.ts && egret?.from === first[0].id, JSON.stringify(second))

  // Mira asks before each action: her card comes to Slack, and Allow once there lets her make the file.
  slack.say('Mira: Create a file named slack-check.txt in this folder containing the word OK. Use your file-writing tool.')
  const card = await slack.until((state) => state.sent.find((one) => one.text.startsWith('*Mira needs your approval*')), 300_000, 'Mira’s card').catch(() => undefined)
  const buttons = card?.blocks?.find((one) => one.type === 'actions')?.elements ?? []
  check('Mira’s card comes with Allow once and Deny, and no Always', JSON.stringify(buttons.map((one) => one.text.text)) === JSON.stringify(['Allow once', 'Deny']), card?.text)
  await drive.capture('the window: Mira’s card, also waiting here', () => drive.evaluate('1'))
  if (card !== undefined && buttons.length === 2) {
    slack.press(card.ts, buttons[0].action_id)
    let made = false
    for (let i = 0; i < 60 && !made; i += 1) { await sleep(1000); made = await access(join(workspace, 'slack-check.txt')).then(() => true, () => false) }
    check('Allow once in Slack let Mira make the file', made, made ? 'slack-check.txt is there' : '(no file)')
    const settled = slack.state.edits.find((one) => one.ts === card.ts)
    check('the card says Allowed once, and its buttons are gone', settled !== undefined && /Allowed once\./.test(settled.text) && settled.blocks.length === 0, settled?.text)
    const mira = await slack.until(() => slack.state.sent.find((one) => one.text.startsWith('*Mira*')), 240_000, 'Mira’s answer').catch(() => undefined)
    check('Mira’s answer comes back once she is done', mira !== undefined, mira?.text)
  }
  check('Slack took every message Locust sent: nothing refused', slack.state.refused.length === 0, JSON.stringify(slack.state.refused.slice(0, 2)))
  await drive.capture('the window: after Slack', () => drive.evaluate('1'))

  const removed = String(await drive.capture('Remove: back to off', () => drive.evaluate(rowScript(`
    ;[...row.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Remove')?.click()
    for (let i = 0; i < 40 && !/Set up/.test(row?.innerText ?? ''); i += 1) await new Promise((r) => setTimeout(r, 250))
    return ${ROW}`))))
  check('Remove forgets the app and the account', /Off/.test(removed) && /Set up/.test(removed) && !/Sam/.test(removed), removed.slice(0, 200))
  const opens = slack.state.opened
  await sleep(3000)
  check('removed, Locust opens no new connection to Slack', slack.state.opened === opens, `${String(opens)} then ${String(slack.state.opened)}`)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. Slack: a stand-in for its Web API and Socket Mode at ${API} (_tools/fake-slack.mjs). Ada (Ask) and Mira (Approve each) run on ${FREE_ROUTE.model}.`, extra: `Checks failed: ${String(failures)}` })
  server.close()
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
