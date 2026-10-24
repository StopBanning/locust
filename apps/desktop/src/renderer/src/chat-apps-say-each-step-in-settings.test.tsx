import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import type { ChatAppSettings } from '../../shared/chat-apps.js'
import { SLACK_MANIFEST_LINK } from '../../shared/slack.js'
import { ChatAppsSettings, DiscordSettings, SlackSettings } from './components/ChatAppsSettings.js'
import { TelegramSettings } from './components/TelegramSettings.js'

/**
 * CHAT APPS SAY EACH STEP IN SETTINGS (0.831 Telegram, 0.832 Discord and Slack; components/ChatAppsSettings.tsx).
 * One row an app, Discord first: off, a name and Set up; set up, each step said where it is done, the token in a field
 * that never shows it, then the code that pairs the person's account, then who is paired and the truth about where
 * the words go.
 */
const soon = (): string => new Date(Date.now() + 60_000).toISOString()
const list = (row: ReturnType<typeof DiscordSettings>): string => renderToStaticMarkup(<ul>{row}</ul>)

describe('chat apps say each step in Settings', () => {
  it('names Discord, Slack and Telegram in that order, each off with Set up', () => {
    const section = renderToStaticMarkup(<ChatAppsSettings heading={<h2>Chat apps</h2>} />)
    expect(section).toContain('Write to your teammates from Discord, Slack or Telegram')
    const order = ['data-chat-app="discord"', 'data-chat-app="slack"', 'data-chat-app="telegram"'].map((one) => section.indexOf(one))
    expect(order.every((at) => at > -1)).toBe(true)
    expect([...order].sort((a, b) => a - b)).toEqual(order)
    expect(section.match(/>Set up</g)).toHaveLength(3)
    // Off and not opened, no steps show.
    expect(section).not.toContain('type="password"')
  })

  it('walks Discord from the Developer Portal to the code sent to the bot', () => {
    const off = list(<DiscordSettings initial={{ connected: false }} open />)
    expect(off).toContain('Open the Developer Portal')
    expect(off).toMatch(/<input[^>]*type="password"[^>]*aria-label="Your bot’s token from Discord"/)
    expect(off).toContain('kept encrypted on this computer and sent only to Discord')
    const pairing = list(<DiscordSettings initial={{
      connected: true, bot: { username: 'Locust#4242', name: 'Locust', invite: 'https://discord.com/oauth2/authorize?client_id=1', open: 'https://discord.com/users/1' }, code: { code: '47291836', until: soon() }
    }} />)
    expect(pairing).toContain('Not paired yet')
    expect(pairing).toContain('Connected to Locust#4242. It reads only its direct messages.')
    expect(pairing).toContain('Add the bot to a server')
    expect(pairing).toContain('4729 1836')
    expect(pairing).toContain('Copy the code')
    expect(pairing).toContain('Open the bot in Discord')
    expect(pairing).not.toContain('type="password"')
    const paired = list(<DiscordSettings initial={{ connected: true, bot: { username: 'Locust#4242', name: 'Locust' }, chat: { name: 'Sam', username: 'sam_here', pairedAt: new Date().toISOString() } }} />)
    expect(paired).toContain('On · Sam')
    expect(paired).toContain('Sam (@sam_here)')
    expect(paired).toContain('Pair another account')
    expect(paired).toContain('type / to pick a teammate')
    expect(paired).toContain('Discord’s servers can read what you and your teammates write there')
    expect(paired).toContain('Approval cards there offer Allow once and Deny; Always stays here.')
  })

  it('walks Slack from the app it makes in one step to its two tokens and the code', () => {
    const off = list(<SlackSettings initial={{ connected: false }} open />)
    expect(off).toContain('Create the app in Slack')
    expect(off).toMatch(/aria-label="The bot’s token from Slack"/)
    expect(off).toMatch(/aria-label="The app’s own token from Slack"/)
    expect(off.match(/type="password"/g)).toHaveLength(2)
    expect(off).toContain('connections:write')
    expect(SLACK_MANIFEST_LINK.startsWith('https://api.slack.com/apps?new_app=1&manifest_json=')).toBe(true)
    const pairing = list(<SlackSettings initial={{ connected: true, bot: { username: 'locust', name: 'locust', team: 'Tides', open: 'https://slack.com/app_redirect?app=A1&team=T1' }, code: { code: '12345678', until: soon() } }} />)
    expect(pairing).toContain('Connected to the Locust app in Tides.')
    expect(pairing).toContain('1234 5678')
    expect(pairing).toContain('Open Locust in Slack')
    const paired = list(<SlackSettings initial={{ connected: false, bot: { username: 'locust', name: 'locust', team: 'Tides' }, chat: { name: 'Sam', username: 'sam', pairedAt: new Date().toISOString() }, message: 'Slack could not be reached. Locust keeps trying.' }} />)
    expect(paired).toContain('Paired in Tides · not connected now')
    expect(paired).toContain('Slack could not be reached. Locust keeps trying.')
    expect(paired).toContain('“Ada: check the build”')
  })

  it('walks Telegram from BotFather to the link that pairs a chat', () => {
    const off = list(<TelegramSettings initial={{ connected: false }} open />)
    expect(off).toContain('Open BotFather')
    expect(off).toMatch(/<input[^>]*type="password"[^>]*aria-label="Your bot’s token from BotFather"/)
    expect(off).toContain('autoComplete="off"')
    expect(off).toContain('kept encrypted on this computer and sent only to Telegram')
    expect(off).not.toContain('lc-phone__qr')
    const pairing = list(<TelegramSettings initial={{ connected: true, bot: { username: 'tides_bot', name: 'Tides' }, link: { url: 'https://t.me/tides_bot?start=abcdefghijklmnop', until: soon() } }} />)
    expect(pairing).toContain('lc-phone__qr')
    expect(pairing).toContain('Open in Telegram')
    expect(pairing).toContain('Copy the link')
    expect(pairing).toContain('Reading @tides_bot’s messages.')
    const paired: ChatAppSettings = { connected: true, bot: { username: 'tides_bot', name: 'Tides' }, chat: { name: 'Sam', username: 'sam_here', pairedAt: new Date().toISOString() } }
    const drawn = list(<TelegramSettings initial={paired} />)
    expect(drawn).toContain('Sam (@sam_here)')
    expect(drawn).toContain('Pair another chat')
    expect(drawn).toContain('Telegram’s servers can read what you and your teammates write there')
  })
})
