// Drive (0.831; its row of Chat apps since 0.832): YOUR TEAMMATES IN TELEGRAM, end to end. The real window, and a stand-in for Telegram's Bot API on
// this computer (_tools/fake-telegram.mjs, as strict as Telegram on markup) playing the person's chat: the token goes
// in Settings' masked field, the chat pairs by the link, `/ada` reaches Ada and her answer comes back as a reply, a
// message with no command carries the conversation on, a stranger is ignored, and Mira's approval card comes to the
// chat with Allow once and Deny -- Allow once pressed there, the file is made. Every turn runs on the free OpenCode
// model, so nothing is spent. No real Telegram account or bot is used.
//
//   node _tools/drive-teammates-in-telegram.mjs [--packaged <Locust.exe>] [--tag <name>]
import { access, mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { FREE_ROUTE, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'
import { fakeTelegram } from './fake-telegram.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')

const telegram = fakeTelegram()
const server = await telegram.listen(0)
const API = `http://127.0.0.1:${String(server.address().port)}`
const STRANGER = { id: 9_999, is_bot: false, first_name: 'Mallory' }

const workspace = await scratchRepository('locust-drive-telegram-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-telegram-profile-'))
// Ada answers in Ask; Mira asks before each action, so the file she makes raises a card.
const ADA = { teammateId: 'tm_ada0000000000000000000', name: 'Ada', hue: 'teal', role: 'Research & Briefs', createdAt: '2026-10-23T01:00:00.000Z', route: { ...FREE_ROUTE, mode: 'ask' } }
const MIRA = { teammateId: 'tm_mira000000000000000000', name: 'Mira', hue: 'violet', role: 'Custom', roleTitle: 'Writer', createdAt: '2026-10-23T01:00:00.000Z', route: { ...FREE_ROUTE, mode: 'approve-each' } }

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `teammates-in-telegram-${tag}`,
  port: 9781,
  workspace,
  profilePath,
  env: { LOCUST_TELEGRAM_API: API },
  outPath: join(recordRoot('teammates-in-telegram-2026-10-23'), tag),
  seed: { schemaVersion: 1, teammates: [ADA, MIRA], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 600)}`}`)
}
const settingsScript = (body) => `(async () => {
  if (!document.querySelector('[data-setting="chat-apps"]')) {
    if (!document.querySelector('.lc-settings__navitem')) {
      ;[...document.querySelectorAll('button')].find((one) => one.innerText.trim() === 'Settings')?.click()
      await new Promise((r) => setTimeout(r, 900))
    }
    ;[...document.querySelectorAll('.lc-settings__navitem')].find((one) => one.innerText.trim().startsWith('General'))?.click()
    await new Promise((r) => setTimeout(r, 900))
  }
  const section = document.querySelector('[data-chat-app="telegram"]')
  section?.scrollIntoView({ block: 'center' })
  ${body}
})()`
const SECTION = `(section?.innerText ?? 'no section').replace(/\\s+/g, ' ')`
const THREAD = `(document.querySelector('.lc-thread')?.innerText ?? '').replace(/\\s+/g, ' ')`
const history = async () => JSON.parse(String(await drive.evaluate(`window.desktop.getMissionHistory().then((r) => JSON.stringify(r.ok ? r.data.missions.map((m) => ({ id: m.missionId, prompt: m.prompt, phase: m.phase, from: m.continuesFrom?.missionId ?? null })) : []))`)))
const sentSince = (count) => telegram.state.sent.slice(count)

try {
  await drive.capture('launch: Ada and Mira, no conversations', () => drive.ready())
  await drive.resize(1215, 860)

  // The empty state, as a person first sees it, at three sizes.
  const off = String(await drive.capture('Telegram, off: Set up, then BotFather and the token field', () => drive.evaluate(settingsScript(`
    ;[...section.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Set up')?.click()
    for (let i = 0; i < 20 && !section.querySelector('input'); i += 1) await new Promise((r) => setTimeout(r, 100))
    return ${SECTION}`))))
  check('off, Settings says so, and Set up starts with BotFather', /^Telegram Off Hide/.test(off) && /Open BotFather/.test(off), off.slice(0, 300))
  const masked = await drive.evaluate(settingsScript(`return section?.querySelector('input')?.type ?? 'none'`))
  check('the token goes in a masked field', masked === 'password', masked)
  for (const [width, height] of [[960, 720], [760, 640]]) {
    await drive.resize(width, height)
    await sleep(600)
    await drive.capture(`Telegram, off, at ${String(width)} by ${String(height)}`, () => drive.evaluate(settingsScript('return 1')))
  }
  await drive.resize(1215, 860)

  // The person pastes the token BotFather gave them, and presses Connect.
  await drive.evaluate(settingsScript(`section?.querySelector('input[type="password"]')?.focus(); return 1`))
  await drive.send('Input.insertText', { text: telegram.token })
  const connected = String(await drive.capture('Connect: the bot named, and the code that pairs a chat', () => drive.evaluate(settingsScript(`
    ;[...section.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Connect')?.click()
    for (let i = 0; i < 60 && !section?.querySelector('.lc-phone__qr'); i += 1) await new Promise((r) => setTimeout(r, 250))
    for (let i = 0; i < 60 && !/Reading @/.test(section?.innerText ?? ''); i += 1) await new Promise((r) => setTimeout(r, 250))
    return ${SECTION}`))))
  check('connected, Settings names the bot and shows the code and the link', /Not paired yet/.test(connected) && /Open in Telegram/.test(connected) && /Reading @locust_test_bot’s messages/.test(connected), connected.slice(0, 300))
  const shown = String(await drive.evaluate(settingsScript(`return section?.querySelector('input')?.value ?? '(no field)'`)))
  check('the token is not shown again anywhere in Settings', shown === '(no field)' && !String(await drive.evaluate('document.body.innerText')).includes(telegram.token.split(':')[1]), shown)

  // The person opens the link on their phone and presses Start: Telegram sends the bot "/start <code>".
  const link = JSON.parse(String(await drive.evaluate(`window.desktop.telegram.settings().then((s) => JSON.stringify(s.link ?? {}))`)))
  const code = new URL(link.url).searchParams.get('start')
  telegram.say(`/start ${code}`)
  const paired = String(await drive.capture('Start pressed in Telegram: the chat is paired', () => drive.evaluate(settingsScript(`
    for (let i = 0; i < 60 && !/Sam \\(@sam_here\\)/.test(section?.innerText ?? ''); i += 1) await new Promise((r) => setTimeout(r, 250))
    return ${SECTION}`))))
  check('Settings shows the paired chat, and what Telegram can read', /Sam \(@sam_here\)/.test(paired) && /Telegram’s servers can read what you and your teammates write there/.test(paired), paired.slice(0, 400))
  const welcome = await telegram.until((state) => state.sent.find((one) => one.text.startsWith('Paired with Locust on')), 10_000, 'the welcome').catch(() => undefined)
  check('the chat is told it is paired, and how to write to a teammate', welcome !== undefined && /\/ada and a message writes to Ada/.test(welcome.text), welcome?.text)
  const menu = await telegram.until((state) => state.commands.at(-1), 10_000, 'the menu').catch(() => [])
  check('the bot’s menu names each teammate, then Locust’s own', JSON.stringify(menu.map((one) => one.command)) === JSON.stringify(['ada', 'mira', 'who', 'new', 'waiting', 'help']), JSON.stringify(menu))
  for (const [width, height] of [[960, 720], [760, 640]]) {
    await drive.resize(width, height)
    await sleep(600)
    await drive.capture(`Telegram, paired, at ${String(width)} by ${String(height)}`, () => drive.evaluate(settingsScript('return 1')))
  }
  await drive.resize(1215, 860)

  // A stranger finds the bot: ignored, nothing started.
  const beforeStranger = telegram.state.sent.length
  telegram.say('/ada delete the folder', { from: STRANGER })
  await sleep(3000)
  check('a stranger’s message starts nothing and gets no answer', (await history()).length === 0 && telegram.state.sent.length === beforeStranger, JSON.stringify(sentSince(beforeStranger).map((one) => one.text)))

  // /ada and a message: Ada's turn, here in the window, and her answer back in the chat as a reply.
  const asked = telegram.say('/ada Reply with the word HERON in capitals and nothing else.')
  const answered = await telegram.until((state) => state.sent.find((one) => one.html.startsWith('<b>Ada</b>') && /HERON/.test(one.text)), 240_000, 'Ada’s answer').catch(() => undefined)
  check('Ada’s answer comes back in the chat, as a reply to the message', answered !== undefined && answered.reply_to === asked, answered === undefined ? telegram.read().join(' | ') : answered.text)
  check('while Ada worked, the chat showed the bot typing', telegram.state.actions.some((one) => one.action === 'typing'), String(telegram.state.actions.length))
  const first = await history()
  check('the turn is in Locust, as the person’s own message', first.length === 1 && first[0].prompt === 'Reply with the word HERON in capitals and nothing else.', JSON.stringify(first))
  const desk = String(await drive.capture('the window: the conversation sent from Telegram', () => drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('button')].find((one) => /^(Back|Close settings)$/.test(one.innerText.trim()))?.click()
    await new Promise((r) => setTimeout(r, 600))
    ;[...document.querySelectorAll('.lc-convrow button.lc-conv')].find((r) => /HERON/i.test(r.innerText))?.click()
    for (let i = 0; i < 20 && !/HERON[^]*HERON/.test(${THREAD}); i += 1) await new Promise((r) => setTimeout(r, 500))
    return ${THREAD}
  })()`)))
  check('the window shows the message from Telegram as the person’s, then Ada’s answer', /Reply with the word HERON/.test(desk) && /HERON/.test(desk.slice(desk.indexOf('nothing else.') + 12)), desk.slice(-300))

  // No command: the same teammate, the same conversation.
  const next = telegram.say('Now reply with the word EGRET in capitals and nothing else.')
  const followed = await telegram.until((state) => state.sent.find((one) => one.html.startsWith('<b>Ada</b>') && /EGRET/.test(one.text)), 240_000, 'the follow-up').catch(() => undefined)
  const second = await history()
  const egret = second.find((one) => /EGRET/.test(one.prompt))
  check('a message with no command carries Ada’s conversation on', followed?.reply_to === next && egret?.from === first[0].id, JSON.stringify(second))

  // Mira asks before each action: her card comes to the chat, and Allow once there lets her make the file.
  telegram.say('/mira Create a file named telegram-check.txt in this folder containing the word OK. Use your file-writing tool.')
  const card = await telegram.until((state) => state.sent.find((one) => one.text.startsWith('Mira needs your approval')), 300_000, 'Mira’s card').catch(() => undefined)
  const buttons = card?.reply_markup?.inline_keyboard?.[0]?.map((one) => one.text) ?? []
  check('Mira’s card comes to the chat with Allow once and Deny, and no Always', JSON.stringify(buttons) === JSON.stringify(['Allow once', 'Deny']), card?.text)
  await drive.capture('the window: Mira’s card, also waiting here', () => drive.evaluate('1'))
  if (card !== undefined) {
    const allow = card.reply_markup.inline_keyboard[0][0].callback_data
    telegram.press(card.message_id, allow)
    let made = false
    for (let i = 0; i < 60 && !made; i += 1) { await sleep(1000); made = await access(join(workspace, 'telegram-check.txt')).then(() => true, () => false) }
    check('Allow once in the chat let Mira make the file', made, made ? 'telegram-check.txt is there' : '(no file)')
    const settled = telegram.state.edits.find((one) => one.message_id === card.message_id)
    check('the card in the chat says Allowed once, and its buttons are gone', settled !== undefined && /Allowed once\./.test(settled.html) && (settled.reply_markup?.inline_keyboard ?? []).length === 0, settled?.html)
    const mira = await telegram.until((state) => state.sent.find((one) => one.html.startsWith('<b>Mira</b>')), 240_000, 'Mira’s answer').catch(() => undefined)
    check('Mira’s answer comes back once she is done', mira !== undefined, mira?.text)
  }
  check('Telegram took every message Locust sent: no markup refused', telegram.state.refused.length === 0, JSON.stringify(telegram.state.refused.slice(0, 2)))
  await drive.capture('the window: after the chat', () => drive.evaluate('1'))

  // Remove: the token and the chat are forgotten, and Settings is back where it began.
  const removed = String(await drive.capture('Remove: back to off', () => drive.evaluate(settingsScript(`
    ;[...section.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Remove')?.click()
    for (let i = 0; i < 40 && !/Set up|Open BotFather/.test(section?.innerText ?? ''); i += 1) await new Promise((r) => setTimeout(r, 250))
    return ${SECTION}`))))
  check('Remove forgets the bot and the chat', /Off/.test(removed) && /Set up|Open BotFather/.test(removed) && !/Sam/.test(removed), removed.slice(0, 200))
  const pollsAfter = telegram.state.polls
  await sleep(3000)
  check('removed, Locust stops reading the bot', telegram.state.polls === pollsAfter, `${String(pollsAfter)} then ${String(telegram.state.polls)}`)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. Telegram: a stand-in for its Bot API at ${API} (_tools/fake-telegram.mjs). Ada (Ask) and Mira (Approve each) run on ${FREE_ROUTE.model}.`, extra: `Checks failed: ${String(failures)}` })
  server.close()
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
