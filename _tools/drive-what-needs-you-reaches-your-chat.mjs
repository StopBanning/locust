// Drive (0.837): WHAT NEEDS YOU REACHES YOUR CHAT APP. The real window, and the stand-in for Discord on this computer
// (_tools/fake-discord.mjs). Discord is set up and paired as in drive-teammates-in-discord.mjs; then Settings' "Tell
// me here when something needs me" is shown off, turned on, and Mira's card from a turn started IN THE WINDOW reaches
// Discord with Allow once and Deny -- pressed there, the file is made. A card from a turn started in Discord arrives
// once, not twice. Turned off, a window card stays in the window. Every turn runs on a free OpenCode model, so nothing
// is spent. No real Discord account or bot is used.
//
//   node _tools/drive-what-needs-you-reaches-your-chat.mjs [--packaged <Locust.exe>] [--tag <name>] [--out <folder>]
import { access, mkdtemp, readFile, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { FREE_ROUTE, openTeammateScript, say, scratchRepository, sendAndWaitScript, sleep, startDrive } from './drive-lib.mjs'
import { fakeDiscord } from './fake-discord.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const OUT = arg('--out') ?? join('D:/work/scratch/chat-app-alerts', tag)
// --haiku: Mira on Claude Haiku through Claude Code (LOCUST_SPEND=1), for when the free models are out.
const HAIKU = process.argv.includes('--haiku')
const ROUTE = HAIKU ? { runtime: 'claude', model: 'haiku' } : FREE_ROUTE

const discord = fakeDiscord({})
const server = await discord.listen(0)
const API = `http://127.0.0.1:${String(server.address().port)}`

const workspace = await scratchRepository('locust-drive-chat-alerts-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-chat-alerts-profile-'))
// Mira asks before each action, so a file she makes raises a card.
const MIRA = { teammateId: 'tm_mira000000000000000000', name: 'Mira', hue: 'violet', role: 'Custom', roleTitle: 'Writer', createdAt: '2026-10-23T01:00:00.000Z', route: { ...ROUTE, mode: 'approve-each' } }

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `chat-alerts-${tag}`,
  port: 9785,
  workspace,
  profilePath,
  env: { LOCUST_DISCORD_API: API },
  outPath: OUT,
  ...(HAIKU ? { spends: true } : {}),
  seed: { schemaVersion: 1, teammates: [MIRA], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
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
  const alerts = row?.querySelector('[data-chat-alerts="discord"]')
  ;(alerts ?? row)?.scrollIntoView({ block: 'center' })
  ${body}
})()`
const ROW = `(row?.innerText ?? 'no row').replace(/\\s+/g, ' ')`
const SWITCH = `row?.querySelector('[role="switch"][aria-label="Tell me in Discord when something needs me"]')`
const closeSettings = `(async () => { ;[...document.querySelectorAll('button')].find((one) => /^(Back|Close settings)$/.test(one.innerText.trim()))?.click(); await new Promise((r) => setTimeout(r, 600)); return 1 })()`
const cardsFor = (pattern) => discord.state.sent.filter((one) => one.content.startsWith('**Mira needs your approval**') && pattern.test(one.content))
const made = async (name, seconds) => {
  for (let i = 0; i < seconds; i += 1) { if (await access(join(workspace, name)).then(() => true, () => false)) return true; await sleep(1000) }
  return false
}
// The window behind another app: the person is away from it. The router reads the window's focus.
// A turn started in the window runs in the message box's mode: Approve, so a file Mira makes raises a card.
const approveEach = () => drive.evaluate(`(async () => {
  for (let tries = 0; tries < 3; tries += 1) {
    const button = document.querySelector('button[aria-label="Permission mode"]')
    if (!button) { await new Promise((r) => setTimeout(r, 500)); continue }
    button.click()
    await new Promise((r) => setTimeout(r, 400))
    const item = [...document.querySelectorAll('[role="menu"][aria-label="Permission mode"] [role="menuitemradio"]')].find((b) => /^Approve/.test(b.innerText.trim()))
    if (item) { item.click(); await new Promise((r) => setTimeout(r, 300)); return 'approve' }
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    await new Promise((r) => setTimeout(r, 300))
  }
  return 'no approve item; mode button: ' + (document.querySelector('button[aria-label="Permission mode"]')?.innerText ?? 'none') + '; settings open: ' + !!document.querySelector('.lc-settings__navitem')
})()`)
// A drive's window may well hold the focus, and then Locust shows the card itself: minimized, as by someone stepping
// away, the alert rules let it out (drive-your-phone-alerts.mjs does the same).
const away = () => drive.evaluate('window.desktop.minimize()')

try {
  await drive.capture('launch: Mira, no conversations', () => drive.ready())
  await drive.resize(1215, 860)

  // Discord set up and paired, as the person does it.
  await drive.evaluate(rowScript(`;[...row.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Set up')?.click(); for (let i = 0; i < 20 && !row.querySelector('input'); i += 1) await new Promise((r) => setTimeout(r, 100)); return 1`))
  await drive.evaluate(rowScript(`row?.querySelector('input[type="password"]')?.focus(); return 1`))
  await drive.send('Input.insertText', { text: discord.token })
  await drive.evaluate(rowScript(`
    ;[...row.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Connect')?.click()
    for (let i = 0; i < 60 && !row?.querySelector('.lc-chatapp__digits'); i += 1) await new Promise((r) => setTimeout(r, 250))
    return 1`))
  discord.say(String(await drive.evaluate(rowScript(`return row?.querySelector('.lc-chatapp__digits')?.innerText ?? ''`))))
  const paired = String(await drive.evaluate(rowScript(`for (let i = 0; i < 60 && !/Sam \\(@sam_here\\)/.test(row?.innerText ?? ''); i += 1) await new Promise((r) => setTimeout(r, 250)); return ${ROW}`)))
  check('Discord is paired with the person’s account', /On · Sam/.test(paired), paired.slice(0, 200))

  const off = String(await drive.capture('Discord paired: Tell me here when something needs me, off', () => drive.evaluate(rowScript(`return ${ROW} + ' || ' + (${SWITCH}?.getAttribute('aria-checked') ?? 'no switch')`))))
  check('paired, Settings offers the switch, off, saying what Discord can read', /Tell me here when something needs me/.test(off) && /Discord’s servers can read who is asking, what the card asks to do and the first line of its file or command/.test(off) && /\|\| false$/.test(off), off.slice(-500))
  for (const [width, height] of [[960, 720], [760, 640]]) {
    await drive.resize(width, height)
    await sleep(600)
    const fits = String(await drive.capture(`the switch at ${String(width)} by ${String(height)}`, () => drive.evaluate(rowScript(`const r = ${SWITCH}?.getBoundingClientRect(); return r ? JSON.stringify({ right: Math.round(r.right), width: innerWidth }) : 'no switch'`))))
    const box = fits.startsWith('{') ? JSON.parse(fits) : undefined
    check(`at ${String(width)} by ${String(height)} the switch is inside the window`, box !== undefined && box.right <= box.width, fits)
  }
  await drive.resize(1215, 860)

  // A window card with the switch OFF stays in the window.
  await drive.evaluate(closeSettings)
  await drive.evaluate(openTeammateScript('Mira'))
  await sleep(800)
  say(`  mode: ${String(await approveEach())}`)
  await drive.evaluate(sendAndWaitScript('Create a file named before-on.txt in this folder containing the word OK. Use your file-writing tool.', { settle: false }))
  await away()
  const waitedOff = await drive.evaluate(`(async () => { for (let i = 0; i < 240 && !document.querySelector('[role=group][aria-label="Approval required"]'); i += 1) await new Promise((r) => setTimeout(r, 1000)); return !!document.querySelector('[role=group][aria-label="Approval required"]') })()`)
  await sleep(3000)
  check('off, a card from a window turn waits in the window and not in Discord', waitedOff === true && cardsFor(/before-on/).length === 0, `card in window: ${String(waitedOff)}; in Discord: ${String(cardsFor(/before-on/).length)}`)
  // Answered in the window, so it is out of the way: Approve once (Deny… opens a menu of its own).
  await drive.evaluate(`(async () => { ;[...document.querySelectorAll('[role=group][aria-label="Approval required"] button')].find((b) => /^Approve once/.test(b.innerText.trim()))?.click(); await new Promise((r) => setTimeout(r, 800)); for (let i = 0; i < 120 && document.querySelector('button[aria-label^="Stop the running"]'); i += 1) await new Promise((r) => setTimeout(r, 1000)); return 1 })()`)

  // On.
  const on = String(await drive.capture('Tell me here when something needs me, on', () => drive.evaluate(rowScript(`${SWITCH}?.click(); for (let i = 0; i < 40 && ${SWITCH}?.getAttribute('aria-checked') !== 'true'; i += 1) await new Promise((r) => setTimeout(r, 100)); return ${SWITCH}?.getAttribute('aria-checked') ?? 'no switch'`))))
  check('the switch turns on', on === 'true', on)
  await drive.evaluate(closeSettings)

  // A window card, the person away: it reaches Discord with Allow once and Deny.
  await drive.evaluate(openTeammateScript('Mira'))
  await sleep(800)
  say(`  mode: ${String(await approveEach())}`)
  await drive.evaluate(sendAndWaitScript('Create a file named from-the-window.txt in this folder containing the word OK. Use your file-writing tool.', { settle: false }))
  await away()
  const card = await discord.until(() => cardsFor(/from-the-window/)[0], 300_000, 'the window card in Discord').catch(() => undefined)
  const buttons = card?.components?.[0]?.components ?? []
  check('a card from a turn started in the window reaches Discord, with Allow once and Deny', card !== undefined && JSON.stringify(buttons.map((one) => one.label)) === JSON.stringify(['Allow once', 'Deny']), card?.content ?? discord.read().slice(-3).join(' | '))
  await drive.capture('the window: Mira’s card, also in Discord', () => drive.evaluate('1'))
  if (card !== undefined && buttons.length === 2) {
    discord.press(card.id, buttons[0].custom_id)
    check('Allow once in Discord lets Mira make the file', await made('from-the-window.txt', 90))
    const settled = discord.state.edits.find((one) => one.id === card.id)
    check('the card in Discord says Allowed once, and its buttons are gone', settled !== undefined && /Allowed once\./.test(settled.content) && settled.components.length === 0, settled?.content)
  }
  await drive.evaluate(`(async () => { for (let i = 0; i < 180 && document.querySelector('button[aria-label^="Stop the running"]'); i += 1) await new Promise((r) => setTimeout(r, 1000)); return 1 })()`)

  // A card from a turn started in Discord: once, not twice.
  discord.command('mira', 'Create a file named from-discord.txt in this folder containing the word OK. Use your file-writing tool.')
  await away()
  const own = await discord.until(() => cardsFor(/from-discord/)[0], 300_000, 'the Discord turn’s card').catch(() => undefined)
  await sleep(150_000)
  check('a card from a turn started in Discord arrives once, not twice', own !== undefined && cardsFor(/from-discord/).length === 1, String(cardsFor(/from-discord/).length))
  const kept = JSON.parse(await readFile(join(profilePath, 'discord.json'), 'utf8').catch(() => '{}'))
  say(`  Locust's Discord file: chat ${kept.chat === undefined ? 'none' : 'paired'}, alerts ${String(kept.alerts)}, turns ${JSON.stringify((kept.turns ?? []).map((one) => one.missionId))}, cards ${JSON.stringify((kept.cards ?? []).map((one) => one.approvalId))}`)
  say(`  the window's waiting cards: ${String(await drive.evaluate(`(async () => JSON.stringify([...document.querySelectorAll('[role=group][aria-label="Approval required"]')].length))()`))}; sent to Discord: ${String(discord.state.sent.length)}`)
  say(`  Discord's answers to commands: ${JSON.stringify(discord.state.responses.map((one) => one.content).slice(-3))}`)
  await drive.capture('the window after the Discord turn', () => drive.evaluate(`(document.querySelector('.lc-thread')?.innerText ?? '').replace(/\\s+/g, ' ').slice(-600)`))
  if (own !== undefined) discord.press(own.id, own.components[0].components[1].custom_id)
  check('Discord took every message Locust sent: nothing refused', discord.state.refused.length === 0, JSON.stringify(discord.state.refused.slice(0, 2)))
  await writeFile(join(OUT, 'discord-received.txt'), discord.state.sent.map((one) => `${one.content}${(one.components ?? []).length > 0 ? `  [${one.components[0].components.map((b) => b.label).join(' | ')}]` : ''}`).join('\n\n---\n\n'), 'utf8')
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. Discord: a stand-in at ${API} (_tools/fake-discord.mjs). Mira (Approve each) runs on ${ROUTE.model}.`, extra: `Checks failed: ${String(failures)}` })
  server.close()
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
