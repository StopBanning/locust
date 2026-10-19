import { readdirSync, readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

import { DOWNLOADS } from '../shared/locust-home.js'

import { CATALOG_INDEX_URL } from './pet-library.js'
import { MAC_RELEASES_API } from './mac-release.js'
import { PAGE_LIBRARY_HOSTS } from './page-preview.js'
import { REPORT_DESTINATION } from './report-problem.js'
import { CANARY_VERDICTS_URL } from './runtime-updates.js'
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
  // The updater (Windows), the Mac release list, the pet gallery's fetch, and five openings of the person's browser.
  'index.ts': { places: 8, said: ['Its own updates (Windows)', 'Its own updates (Mac)', 'The pet gallery', 'A link inside a previewed page asks you first', 'open a public GitHub issue', 'open your mail app', 'Locust opens that page in your browser'] },
  'mac-self-update.ts': { places: 2, said: ['Its own updates (Mac)', 'then the disk image you choose'] },
  'runtime-updates.ts': { places: 1, said: ['Keeping Codex CLI and Copilot CLI current'] },
  'pet-library.ts': { places: 1, said: ['The pet gallery'] },
  'voice-host.ts': { places: 1, said: ["Voice typing's one-time files", 'No account, API key, recording, transcript,'] },
  'voice-openai.ts': { places: 1, said: ['OpenAI voice typing', 'api.openai.com/v1/audio/transcriptions', 'No redirects are followed'] },
  'permission-host.ts': { places: 1, said: ['Locust\'s permission host listens on `127.0.0.1` only'] },
  'locust-mcp-host.ts': { places: 1, said: ['Let your other AI apps use Locust', 'authenticates the token before reading the request body'] },
  // Your phone (0.775): the one listener on the home network, not 127.0.0.1.
  'phone-host.ts': { places: 1, said: ['**Your phone** (Settings > General) is off by default', 'listener that is not on `127.0.0.1` only', 'checked before the request body is read'] },
  // The page it serves, whose requests go back to that one listener and nowhere else (its CSP allows only 'self').
  'phone-page.ts': { places: 1, said: ['It serves one page, which holds', 'nothing; everything that page shows or does needs the key'] },
  'antigravity-cascade.ts': { places: 1, said: ['Antigravity\'s own local server, at `127.0.0.1`'] },
  // Test: the models the address serves, then one capped chat request with a tool.
  'own-models.ts': { places: 2, said: ['Testing one of your own models', 'When you press Test'] },
  // The default fetch, and the one request every look and keep goes through.
  'skill-library.ts': { places: 2, said: ['Skills from GitHub', 'Redirects are followed only to those two hosts'] },
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
    expect(packages.length).toBe(4)
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
