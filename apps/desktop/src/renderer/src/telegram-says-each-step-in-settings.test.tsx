import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { TelegramSettings } from './components/TelegramSettings.js'

/**
 * TELEGRAM SAYS EACH STEP IN SETTINGS (0.831, components/TelegramSettings.tsx). Off, it says what it does and the
 * first two steps -- BotFather, then the token in a field that never shows it; with a bot, the code and the link that
 * pair a chat; paired, who it is and the truth about where the words go.
 */
const draw = (initial: Parameters<typeof TelegramSettings>[0]['initial']): string =>
  renderToStaticMarkup(<TelegramSettings heading={<h2>Telegram</h2>} {...(initial === undefined ? {} : { initial })} />)

describe('Telegram says each step in Settings', () => {
  it('starts with BotFather and a masked field for the token', () => {
    const off = draw({ connected: false })
    expect(off).toContain('Off. Write to your teammates from Telegram')
    expect(off).toContain('Open BotFather')
    expect(off).toMatch(/<input[^>]*type="password"[^>]*aria-label="Your bot’s token from BotFather"/)
    expect(off).toContain('autoComplete="off"')
    expect(off).toContain('kept encrypted on this computer and sent only to Telegram')
    expect(off).not.toContain('lc-phone__qr')
  })

  it('shows the code and the link that pair a chat, once a bot is connected', () => {
    const pairing = draw({ connected: true, bot: { username: 'tides_bot', name: 'Tides' }, link: { url: 'https://t.me/tides_bot?start=abcdefghijklmnop', until: new Date(Date.now() + 60_000).toISOString() } })
    expect(pairing).toContain('Connected to @tides_bot. Now pair your own chat with it')
    expect(pairing).toContain('lc-phone__qr')
    expect(pairing).toContain('Open in Telegram')
    expect(pairing).toContain('Copy the link')
    expect(pairing).toContain('Reading @tides_bot’s messages.')
    expect(pairing).not.toContain('type="password"')
  })

  it('names the paired chat, offers another or Remove, and says what Telegram can read', () => {
    const paired = draw({ connected: true, bot: { username: 'tides_bot', name: 'Tides' }, chat: { name: 'Sam', username: 'sam_here', pairedAt: new Date().toISOString() } })
    expect(paired).toContain('On. In Telegram, write /name and a message to @tides_bot')
    expect(paired).toContain('Sam (@sam_here)')
    expect(paired).toContain('Pair another chat')
    expect(paired).toContain('Remove')
    expect(paired).toContain('Telegram’s servers can read what you and your teammates write there')
    expect(paired).toContain('Approval cards there offer Allow once and Deny; Always stays here.')
    // A reason it is not connected is said where the person looks.
    const lost = draw({ connected: false, bot: { username: 'tides_bot', name: 'Tides' }, chat: { name: 'Sam', pairedAt: new Date().toISOString() }, message: 'Telegram could not be reached. Locust keeps trying.' })
    expect(lost).toContain('Telegram could not be reached. Locust keeps trying.')
    expect(lost).toContain('not connected now')
  })
})
