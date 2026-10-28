import { readdirSync, readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

import { DOWNLOADS } from '../shared/locust-home.js'

import { CATALOG_INDEX_URL } from './pet-library.js'
import { MAC_RELEASES_API } from './mac-release.js'
import { PAGE_LIBRARY_HOSTS } from './page-preview.js'
import { REPORT_DESTINATION } from './report-problem.js'
import { CANARY_VERDICTS_URL } from './runtime-updates.js'
import { DISCORD_API } from './discord.js'
import { SLACK_API } from './slack.js'
import { TELEGRAM_API } from './telegram.js'
import { VOICE_OPENAI_URL } from './voice-openai.js'

/**
 * THE NETWORK IS WRITTEN DOWN (0.618; the PRD's R22).
 *
 * docs/NETWORK.md lists every connection Locust itself makes, for a person or
 * a buyer to check. A list nobody keeps is a claim that goes stale the first
 * time someone adds a fetch, so the places in the main process that reach the
 * network are counted here, file by file, and each is tied to the section of
 * the document that says it. A new one fails until the document and this list
 * both name it; an address the code uses must be the one the document gives.
 */
const read = (path: string): string => readFileSync(fileURLToPath(new URL(path, import.meta.url)), 'utf8')
const NETWORK = read('../../../../docs/NETWORK.md')

/**
 * A line that reaches the network: a fetch, a socket, a server, the updater, the person's browser.
 * A fetch handed in, so a test can stand in for it, is a fetch too: own-models.ts's Test called
 * `fetcher(` and went unseen until 0.640.
 */
const REACHES = /\bfetch(?:er)?\(|\bhttps?\.(?:request|get)\(|\bnet\.(?:request|fetch)\(|new WebSocket\(|createServer\(|from 'electron-updater'|openExternal\(/
const code = (text: string): string[] => text.split('\n').filter((line) => !/^\s*(?:\/\/|\*|\/\*)/.test(line))

/** Each file that reaches the network, how many places in it do, and the document's words for them. */
const LISTED: Readonly<Record<string, { readonly places: number; readonly said: readonly string[] }>> = {
  'terminal-ipc.ts': { places: 1, said: ['Links in the terminal', 'HTTP and HTTPS only'] },
  // The updater (Windows), the Mac release list, the pet gallery's fetch, and six openings of the person's browser:
  // the sixth (0.838) is Open in your browser's code and GitHub's sign-in.
  'index.ts': { places: 9, said: ['**Open in your browser**', 'opens GitHub\'s sign-in in your browser, through Locust\'s relay', 'Its own updates (Windows)', 'Its own updates (Mac)', 'The pet gallery', 'A link inside a previewed page asks you first', 'open a public GitHub issue', 'open your mail app', 'Locust opens that page in your browser'] },
  'mac-self-update.ts': { places: 2, said: ['Its own updates (Mac)', 'then the disk image you choose'] },
  'runtime-updates.ts': { places: 1, said: ['Keeping Codex CLI and Copilot CLI current'] },
  'pet-library.ts': { places: 1, said: ['The pet gallery'] },
  'voice-host.ts': { places: 1, said: ["Voice typing's one-time files", 'No account, API key, recording, transcript,'] },
  'voice-openai.ts': { places: 1, said: ['OpenAI voice typing', 'api.openai.com/v1/audio/transcriptions', 'No redirects are followed'] },
  'permission-host.ts': { places: 1, said: ['Locust\'s permission host listens on `127.0.0.1` only'] },
  'browser-host.ts': { places: 1, said: ['Locust\'s browser bridge listens on `127.0.0.1` only', 'in its own isolated Chromium session'] },
  'locust-mcp-host.ts': { places: 1, said: ['Let your other AI apps use Locust', 'authenticates the token before reading the request body'] },
  // Your phone (0.775): the one listener on the home network, not 127.0.0.1.
  'phone-host.ts': { places: 1, said: ['**Your phone** (Settings > General) is off by default', 'listener that is not on `127.0.0.1` only', 'checked before the request body is read'] },
  // The page it serves, whose requests go back to that one listener and nowhere else (its CSP allows only 'self');
  // the same page from Locust's relay opens its WebSocket to the relay that served it, twice: pairing, then paired.
  'phone-page.ts': { places: 3, said: ['It serves one page, which holds', 'nothing; everything that page shows or does needs the key', 'and keeps it open while this is on'] },
  // Your phone from anywhere (0.826): one WebSocket out to the relay, and nothing listening.
  // The look's question and letter, and an ask's letter (0.838).
  'scout-away-look.ts': { places: 3, said: ["**Scout's away look**", 'runs on GitHub, not on your computer', 'never its command'] },
  'desk-page.ts': { places: 1, said: ['**Locust in a browser**', 'never runs code a computer sent it', 'adds no connection of this computer'] },
  // And GitHub, asked once for each browser that signs in whose account its token is (0.838).
  'phone-anywhere.ts': { places: 2, said: ['**Your phone, from anywhere**', 'listens to nothing', 'the relay passes along what it cannot read', 'asks `api.github.com/user` once whose account it is', 'kept nowhere'] },
  // Your teammates in Telegram (0.831): one request at a time to the Bot API, and nothing listening.
  'telegram.ts': { places: 1, said: ['**Your teammates in Telegram**', '`api.telegram.org`', 'no other address is ever sent the token', "Telegram's", 'servers can read what is written there'] },
  // Your teammates in Discord and Slack (0.832): each one connection out -- Discord's gateway, Slack's Socket Mode -- and its REST calls.
  'discord.ts': { places: 2, said: ['**Your teammates in Discord**', '`discord.com`', '`gateway.discord.gg`', 'only direct messages', "Discord's servers can read what is written there"] },
  'slack.ts': { places: 2, said: ['**Your teammates in Slack**', '`slack.com`', 'Socket Mode', 'a `*.slack.com` address', "Slack's servers can read what is written there"] },
  'antigravity-cascade.ts': { places: 1, said: ['Antigravity\'s own local server, at `127.0.0.1`'] },
  // Test: the models the address serves, then one capped chat request with a tool.
  'own-models.ts': { places: 2, said: ['Testing one of your own models', 'When you press Test'] },
  // The default fetch, and the one request every look and keep goes through.
  'skill-library.ts': { places: 2, said: ['Skills from GitHub', 'Redirects are followed only to those two hosts'] },
  'github-issues.ts': { places: 1, said: ['Pictures on a GitHub issue', 'github.com/user-attachments', 'githubusercontent.com', 'When you choose an issue'] },
  // A free port picked on 127.0.0.1 for the tunnel to OpenCode's server on the codespace (0.761); a sign-in's port
  // checked free here, and one request through its tunnel to the agent's own server there (0.768).
  'cloud-computer.ts': { places: 3, said: ['Your cloud computer', 'Locust picks a free port on `127.0.0.1`', 'checks that port is free here', 'asks the agent\'s own server there once'] }
}

describe('every place Locust reaches the network is in docs/NETWORK.md', () => {
  const MAIN = fileURLToPath(new URL('.', import.meta.url))
  const found = Object.fromEntries(
    readdirSync(MAIN)
      .filter((name) => name.endsWith('.ts') && !name.endsWith('.test.ts'))
      .map((name) => [name, code(readFileSync(MAIN + name, 'utf8')).filter((line) => REACHES.test(line)).length] as const)
      .filter(([, places]) => places > 0)
  )

  it('finds the files that do, and no others, each with as many places as listed', () => {
    expect(Object.keys(found).length).toBeGreaterThan(3)
    expect(found).toEqual(Object.fromEntries(Object.entries(LISTED).map(([name, entry]) => [name, entry.places])))
  })

  it('names each in the document', () => {
    for (const [name, entry] of Object.entries(LISTED)) for (const words of entry.said) expect(NETWORK, `${name}: ${words}`).toContain(words)
  })
})

describe('the addresses are the ones the code uses', () => {
  it('reference clones use the person\'s gh and only a fixed GitHub HTTPS fallback', () => {
    const source = read('./conversation-references.ts')
    expect(source).toContain("run('gh', ['repo', 'clone'")
    expect(source).toContain('https://github.com/${repository}.git')
    expect(NETWORK).toContain('Conversation references from GitHub')
    expect(NETWORK).toContain('reference-trash')
    expect(NETWORK).toContain('500 MB')
  })
  it('each chat app has one fixed HTTPS destination, and a stand-in only on this computer', () => {
    expect(TELEGRAM_API).toBe('https://api.telegram.org')
    expect(DISCORD_API).toBe('https://discord.com')
    expect(SLACK_API).toBe('https://slack.com')
    for (const api of [TELEGRAM_API, DISCORD_API, SLACK_API]) expect(NETWORK).toContain(`\`${new URL(api).host}\``)
    const wiring = read('./index.ts')
    const standIn = wiring.slice(wiring.indexOf('const standIn = '), wiring.indexOf('const telegramLink = '))
    expect(standIn).toContain('process.env[variable]')
    expect(standIn).toContain('(?:127')
    expect(standIn).toContain('? named : real')
    for (const named of ["standIn('LOCUST_TELEGRAM_API', TELEGRAM_API)", "standIn('LOCUST_DISCORD_API', DISCORD_API)", "standIn('LOCUST_SLACK_API', SLACK_API)"]) expect(wiring).toContain(named)
    // The sockets: Discord's gateway is only ever Discord's, and Slack's only ever Slack's, unless a stand-in is local.
    expect(read('./discord.ts')).toContain('/^wss:\\/\\/[a-z0-9.-]+\\.discord\\.gg(?:\\/|$)/i')
    expect(read('./slack.ts')).toContain('/^wss:\\/\\/[a-z0-9.-]+\\.slack\\.com\\//i')
  })
  it('OpenAI voice typing has one fixed HTTPS destination', () => {
    expect(VOICE_OPENAI_URL).toBe('https://api.openai.com/v1/audio/transcriptions')
    expect(NETWORK).toContain(VOICE_OPENAI_URL)
  })
  it('the update feed, the Mac release list, the verdict file, the pet gallery and the report page', () => {
    const feed = read('../../electron-builder.yml')
    expect(/provider:\s*generic/.test(feed)).toBe(true)
    expect(/url:\s*(\S+)/.exec(feed)![1]).toBe(DOWNLOADS)
    expect(NETWORK).toContain(`\`${new URL(DOWNLOADS).host}\``)
    expect(NETWORK).toContain(new URL(MAC_RELEASES_API).host + new URL(MAC_RELEASES_API).pathname)
    expect(NETWORK).toContain(CANARY_VERDICTS_URL.replace(/^https:\/\//, ''))
    expect(NETWORK).toContain(new URL(CATALOG_INDEX_URL).host)
    expect(new URL(CATALOG_INDEX_URL).pathname.startsWith('/pets/')).toBe(true)
    expect(REPORT_DESTINATION).toMatch(/^https:\/\/github\.com\//)
  })

  it('every host a previewed page may load from, and no other', () => {
    for (const host of PAGE_LIBRARY_HOSTS) expect(NETWORK, host).toContain(`\`${host}\``)
    expect(NETWORK).toContain(`eight public hosts`)
    expect(PAGE_LIBRARY_HOSTS).toHaveLength(8)
  })

  it('every package Locust installs with npm', () => {
    const packages = [...read('../shared/runtime-install.ts').matchAll(/kind: 'npm', packageName: '([^']+)'/g)].map((match) => match[1]!)
    // Kilo's CLI joined them in 0.838.x: one npm install, as the others.
    expect(packages.length).toBe(5)
    for (const name of packages) expect(NETWORK, name).toContain(`\`${name}\``)
  })

  it('Settings says the same, and links the list in the public copy', () => {
    const screens = read('../renderer/src/components/Screens.tsx')
    // One line of words, whatever the source's wrapping.
    const row = screens.slice(screens.indexOf('<dt>Network</dt>'), screens.indexOf('</dd>', screens.indexOf('<dt>Network</dt>'))).replace(/\s+/g, ' ')
    for (const words of ['checks for and downloads its own updates', 'installs the AI agents you ask it to', 'keeps Codex CLI and Copilot CLI current', 'reads the pet gallery', 'a model of your own about it when you press Test', '<NetworkListLink />']) expect(row, words).toContain(words)
    const link = screens.slice(screens.indexOf('function NetworkListLink'), screens.indexOf('function fewNames'))
    expect(link).toContain('openLink(NETWORK_DOC_LINK).then')
    expect(link).toContain('Every connection, listed')
    expect(read('../shared/outbound-links.ts')).toContain("NETWORK_DOC_LINK = 'https://github.com/StopBanning/locust/blob/main/docs/NETWORK.md'")
    expect(read('../../../../_tools/public-export.mjs')).toContain("'docs/NETWORK.md'")
  })

  it('the window loads nothing but a previewed page\'s libraries, in the installed app', () => {
    expect(read('./index.ts')).toContain('respond({ cancel: !(fromPagePreview(details.frame) && pageMayReach(details.url, details.method)) })')
    expect(read('./index.ts')).toContain('crashReporter.start({ uploadToServer: false })')
  })
})
