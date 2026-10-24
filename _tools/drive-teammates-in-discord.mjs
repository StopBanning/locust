// Drive (0.832): YOUR TEAMMATES IN DISCORD, end to end. The real window, and a stand-in for Discord's REST and
// gateway on this computer (_tools/fake-discord.mjs, as strict as Discord on lengths, buttons and commands) playing
// the person's direct messages with their bot: the token goes in Settings' masked field, the account pairs by the code
// Settings shows, `/ada` from Discord's list reaches Ada and her answer comes back as a reply, a plain message carries
// the conversation on, a stranger is ignored, and Mira's approval card comes with Allow once and Deny -- Allow once
// pressed there, the file is made. Every turn runs on the free OpenCode model, so nothing is spent. No real Discord
// account or bot is used.
//
//   node _tools/drive-teammates-in-discord.mjs [--packaged <Locust.exe>] [--tag <name>]
import { access, mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { FREE_ROUTE, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'
import { fakeDiscord } from './fake-discord.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')

const STRANGER = { id: '1200000000000000999', username: 'mallory', global_name: 'Mallory' }
const STRANGER_CHANNEL = '1300000000000000999'
const discord = fakeDiscord({ channels: [STRANGER_CHANNEL] })
const server = await discord.listen(0)
const API = `http://127.0.0.1:${String(server.address().port)}`

const workspace = await scratchRepository('locust-drive-discord-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-discord-profile-'))
// Ada answers in Ask; Mira asks before each action, so the file she makes raises a card.
const ADA = { teammateId: 'tm_ada0000000000000000000', name: 'Ada', hue: 'teal', role: 'Research & Briefs', createdAt: '2026-10-23T01:00:00.000Z', route: { ...FREE_ROUTE, mode: 'ask' } }
const MIRA = { teammateId: 'tm_mira000000000000000000', name: 'Mira', hue: 'violet', role: 'Custom', roleTitle: 'Writer', createdAt: '2026-10-23T01:00:00.000Z', route: { ...FREE_ROUTE, mode: 'approve-each' } }

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `teammates-in-discord-${tag}`,
  port: 9784,
  workspace,
  profilePath,
  env: { LOCUST_DISCORD_API: API },
  outPath: join(recordRoot('teammates-in-discord-2026-10-23'), tag),
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
  const row = document.querySelector('[data-chat-app="discord"]')
  row?.scrollIntoView({ block: 'center' })
  ${body}
})()`
const ROW = `(row?.innerText ?? 'no row').replace(/\\s+/g, ' ')`
const THREAD = `(document.querySelector('.lc-thread')?.innerText ?? '').replace(/\\s+/g, ' ')`
const history = async () => JSON.parse(String(await drive.evaluate(`window.desktop.getMissionHistory().then((r) => JSON.stringify(r.ok ? r.data.missions.map((m) => ({ id: m.missionId, prompt: m.prompt, phase: m.phase, from: m.continuesFrom?.missionId ?? null })) : []))`)))
const answerOf = (name, word) => discord.state.sent.find((one) => one.content.startsWith(`**${name}**`) && new RegExp(word).test(one.content))

try {
  await drive.capture('launch: Ada and Mira, no conversations', () => drive.ready())
  await drive.resize(1215, 860)

  // The empty state, as a person first sees it: Chat apps, Discord first, each off.
  const section = String(await drive.capture('Chat apps: Discord, Slack and Telegram, each off', () => drive.evaluate(rowScript(`return (document.querySelector('[data-setting="chat-apps"]')?.innerText ?? 'no section').replace(/\\s+/g, ' ')`))))
  check('Settings names Discord first, then Slack and Telegram, each off', /Chat apps/.test(section) && section.indexOf('Discord') < section.indexOf('Slack') && section.indexOf('Slack') < section.indexOf('Telegram') && (section.match(/Set up/g) ?? []).length === 3, section.slice(0, 300))
  const opened = String(await drive.capture('Discord: Set up, its first two steps', () => drive.evaluate(rowScript(`
    ;[...row.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Set up')?.click()
    for (let i = 0; i < 20 && !row.querySelector('input'); i += 1) await new Promise((r) => setTimeout(r, 100))
    return ${ROW}`))))
  check('Set up shows the Developer Portal and the token field', /Open the Developer Portal/.test(opened) && /Reset Token/.test(opened), opened.slice(0, 300))
  const masked = await drive.evaluate(rowScript(`return row?.querySelector('input')?.type ?? 'none'`))
  check('the token goes in a masked field', masked === 'password', masked)
  for (const [width, height] of [[960, 720], [760, 640]]) {
    await drive.resize(width, height)
    await sleep(600)
    await drive.capture(`Discord, set up, at ${String(width)} by ${String(height)}`, () => drive.evaluate(rowScript(`const r = row?.querySelector('button[type="submit"]')?.getBoundingClientRect(); return r ? JSON.stringify({ right: Math.round(r.right), width: innerWidth }) : 'no button'`)))
  }
  await drive.resize(1215, 860)

  // The person pastes their bot's token, and presses Connect.
  await drive.evaluate(rowScript(`row?.querySelector('input[type="password"]')?.focus(); return 1`))
  await drive.send('Input.insertText', { text: discord.token })
  const connected = String(await drive.capture('Connect: the bot named, and the code that pairs an account', () => drive.evaluate(rowScript(`
    ;[...row.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Connect')?.click()
    for (let i = 0; i < 60 && !row?.querySelector('.lc-chatapp__digits'); i += 1) await new Promise((r) => setTimeout(r, 250))
    for (let i = 0; i < 60 && !/It reads only its direct messages/.test(row?.innerText ?? ''); i += 1) await new Promise((r) => setTimeout(r, 250))
    return ${ROW}`))))
  check('connected, Settings names the bot, says where to add it, and shows the code', /Not paired yet/.test(connected) && /Connected to Locust test#4242\. It reads only its direct messages/.test(connected) && /Add the bot to a server/.test(connected) && /\d{4} \d{4}/.test(connected), connected.slice(0, 400))
  check('the gateway asked only for direct messages', JSON.stringify(discord.state.intents) === JSON.stringify([1 << 12]), JSON.stringify(discord.state.intents))
  check('the token is not shown again anywhere in Settings', !String(await drive.evaluate('document.body.innerText')).includes(discord.token.split('.')[2]))

  // The person adds the bot to a server and sends it the code Settings shows, as they read it there.
  const code = String(await drive.evaluate(rowScript(`return row?.querySelector('.lc-chatapp__digits')?.innerText ?? ''`)))
  discord.say(code)
  const paired = String(await drive.capture('the code sent in Discord: the account is paired', () => drive.evaluate(rowScript(`
    for (let i = 0; i < 60 && !/Sam \\(@sam_here\\)/.test(row?.innerText ?? ''); i += 1) await new Promise((r) => setTimeout(r, 250))
    return ${ROW}`))))
  check('Settings shows the paired account, and what Discord can read', /On · Sam/.test(paired) && /Sam \(@sam_here\)/.test(paired) && /Discord’s servers can read what you and your teammates write there/.test(paired), paired.slice(0, 400))
  const welcome = await discord.until(() => discord.read().find((one) => one.startsWith('Paired with Locust on')), 10_000, 'the welcome').catch(() => undefined)
  check('the bot says it is paired, and how to write to a teammate', welcome !== undefined && /\/ada and a message writes to Ada/.test(welcome) && /Type \/ here/.test(welcome), welcome)
  const commands = await discord.until((state) => state.commands.at(-1), 10_000, 'the commands').catch(() => [])
  check('Discord’s list of commands names each teammate, then Locust’s own, in direct messages only', JSON.stringify(commands.map((one) => one.name)) === JSON.stringify(['ada', 'mira', 'who', 'new', 'waiting', 'help']) && commands.every((one) => JSON.stringify(one.contexts) === '[1]'), JSON.stringify(commands.map((one) => one.name)))
  for (const [width, height] of [[960, 720], [760, 640]]) {
    await drive.resize(width, height)
    await sleep(600)
    await drive.capture(`Discord, paired, at ${String(width)} by ${String(height)}`, () => drive.evaluate(rowScript('return 1')))
  }
  await drive.resize(1215, 860)

  // A stranger who shares a server with the bot: ignored, nothing started.
  const before = discord.state.sent.length
  discord.say('/ada delete the folder', { from: STRANGER, channelId: STRANGER_CHANNEL })
  await sleep(3000)
  check('a stranger’s message starts nothing and gets no answer', (await history()).length === 0 && discord.state.sent.length === before, JSON.stringify(discord.state.sent.slice(before).map((one) => one.content)))

  // /ada from Discord's list: Ada's turn, here in the window, and her answer back as a reply.
  const asked = discord.command('ada', 'Reply with the word HERON in capitals and nothing else.')
  const echo = await discord.until((state) => state.responses.find((one) => one.interaction === asked.id), 15_000, 'the command’s answer').catch(() => undefined)
  check('the command is answered at once, saying who it went to', echo?.content === '**To Ada**: Reply with the word HERON in capitals and nothing else.' && discord.state.callbacks.some((one) => one.id === asked.id && one.type === 5), echo?.content)
  const answered = await discord.until(() => answerOf('Ada', 'HERON'), 240_000, 'Ada’s answer').catch(() => undefined)
  check('Ada’s answer comes back as a reply to that message', answered !== undefined && answered.reply_to === echo?.id, answered === undefined ? discord.read().join(' | ') : answered.content)
  check('while Ada worked, the bot showed typing', discord.state.typing.length > 0, String(discord.state.typing.length))
  const first = await history()
  check('the turn is in Locust, as the person’s own message', first.length === 1 && first[0].prompt === 'Reply with the word HERON in capitals and nothing else.', JSON.stringify(first))
  const desk = String(await drive.capture('the window: the conversation sent from Discord', () => drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('button')].find((one) => /^(Back|Close settings)$/.test(one.innerText.trim()))?.click()
    await new Promise((r) => setTimeout(r, 600))
    ;[...document.querySelectorAll('.lc-convrow button.lc-conv')].find((r) => /HERON/i.test(r.innerText))?.click()
    for (let i = 0; i < 20 && !/HERON[^]*HERON/.test(${THREAD}); i += 1) await new Promise((r) => setTimeout(r, 500))
    return ${THREAD}
  })()`)))
  check('the window shows the message from Discord as the person’s, then Ada’s answer', /Reply with the word HERON/.test(desk) && /HERON/.test(desk.slice(desk.indexOf('nothing else.') + 12)), desk.slice(-300))

  // A plain message: the same teammate, the same conversation.
  const next = discord.say('Now reply with the word EGRET in capitals and nothing else.')
  const followed = await discord.until(() => answerOf('Ada', 'EGRET'), 240_000, 'the follow-up').catch(() => undefined)
  const second = await history()
  const egret = second.find((one) => /EGRET/.test(one.prompt))
  check('a plain message carries Ada’s conversation on, answered as a reply to it', followed?.reply_to === next && egret?.from === first[0].id, JSON.stringify(second))

  // Mira asks before each action: her card comes to Discord, and Allow once there lets her make the file.
  discord.command('mira', 'Create a file named discord-check.txt in this folder containing the word OK. Use your file-writing tool.')
  const card = await discord.until((state) => state.sent.find((one) => one.content.startsWith('**Mira needs your approval**')), 300_000, 'Mira’s card').catch(() => undefined)
  const buttons = card?.components?.[0]?.components ?? []
  check('Mira’s card comes with Allow once and Deny, and no Always', JSON.stringify(buttons.map((one) => one.label)) === JSON.stringify(['Allow once', 'Deny']), card?.content)
  await drive.capture('the window: Mira’s card, also waiting here', () => drive.evaluate('1'))
  if (card !== undefined && buttons.length === 2) {
    discord.press(card.id, buttons[0].custom_id)
    let made = false
    for (let i = 0; i < 60 && !made; i += 1) { await sleep(1000); made = await access(join(workspace, 'discord-check.txt')).then(() => true, () => false) }
    check('Allow once in Discord let Mira make the file', made, made ? 'discord-check.txt is there' : '(no file)')
    const settled = discord.state.edits.find((one) => one.id === card.id)
    check('the card says Allowed once, and its buttons are gone', settled !== undefined && /Allowed once\./.test(settled.content) && settled.components.length === 0, settled?.content)
    const mira = await discord.until(() => discord.state.sent.find((one) => one.content.startsWith('**Mira**')), 240_000, 'Mira’s answer').catch(() => undefined)
    check('Mira’s answer comes back once she is done', mira !== undefined, mira?.content)
  }
  check('Discord took every message Locust sent: nothing refused', discord.state.refused.length === 0, JSON.stringify(discord.state.refused.slice(0, 2)))
  await drive.capture('the window: after Discord', () => drive.evaluate('1'))

  // Remove: the token and the account are forgotten, the bot's commands go, and Settings is back where it began.
  const removed = String(await drive.capture('Remove: back to off', () => drive.evaluate(rowScript(`
    ;[...row.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Remove')?.click()
    for (let i = 0; i < 40 && !/Set up/.test(row?.innerText ?? ''); i += 1) await new Promise((r) => setTimeout(r, 250))
    return ${ROW}`))))
  check('Remove forgets the bot and the account', /Off/.test(removed) && /Set up/.test(removed) && !/Sam/.test(removed), removed.slice(0, 200))
  const cleared = await discord.until((state) => state.commands.at(-1)?.length === 0, 10_000, 'the commands cleared').then(() => true, () => false)
  check('removed, the bot’s commands are taken down', cleared)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. Discord: a stand-in for its REST and gateway at ${API} (_tools/fake-discord.mjs). Ada (Ask) and Mira (Approve each) run on ${FREE_ROUTE.model}.`, extra: `Checks failed: ${String(failures)}` })
  server.close()
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
