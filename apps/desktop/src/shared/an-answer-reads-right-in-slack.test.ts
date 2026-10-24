import { describe, expect, it } from 'vitest'

import { CHAT_MAX_PARTS } from './chat-apps.js'
import {
  SLACK_APP_TOKEN, SLACK_BOT_TOKEN, SLACK_MANIFEST, SLACK_MANIFEST_LINK, SLACK_MESSAGE_MAX, SLACK_REST_IN_LOCUST, slackBlocks, slackEscape, slackMessages, slackPlain
} from './slack.js'

/**
 * AN ANSWER READS RIGHT IN SLACK (0.832, shared/slack.ts). Slack's markup is Markdown's cousin -- one star for bold,
 * an underscore for italics, links in angle brackets, no headings, lists or tables -- and three of its marks are its
 * own. Whatever Locust makes of an answer is in Slack's markup, in messages Slack takes; and the app Slack is asked
 * to make is the one Locust needs and no more.
 */
const fences = (message: string): number => message.split('\n').filter((line) => line.startsWith('```')).length

describe('an answer reads right in Slack', () => {
  it('turns Markdown into Slack’s own markup', () => {
    const [plan] = slackMessages('Ada', '# Plan\n\nFirst **read** the `config.ts` file, then *run* the tests.\n\n- one\n- two\n  - nested\n\n1. first')
    expect(plan).toBe('*Ada*\n\n*Plan*\n\nFirst *read* the `config.ts` file, then _run_ the tests.\n\n• one\n• two\n    • nested\n\n1. first')
    const [link] = slackMessages('Ada', 'See [the docs](https://example.com/a?b=1&c=2) and ~~old~~ news.')
    expect(link).toContain('See <https://example.com/a?b=1&amp;c=2|the docs> and ~old~ news.')
    const [quote] = slackMessages('Ada', '> A quote with **bold** & <angles>\n\nAfter it.')
    expect(quote).toContain('> A quote with *bold* &amp; &lt;angles&gt;\n\nAfter it.')
    const [table] = slackMessages('Ada', '| File | Lines |\n|---|---|\n| a & b.ts | 12 |')
    expect(table).toContain('```\n| File | Lines |\n| a &amp; b.ts | 12 |\n```')
    const [code] = slackMessages('Ada', '```ts\nif (a < b && c) {}\n```')
    // Slack's blocks of code have no language: the fence opens on its own.
    expect(code).toContain('```\nif (a &lt; b &amp;&amp; c) {}\n```')
    // Underscores in names and lone stars are only words.
    const [plain] = slackMessages('Ada', 'Snake_case_names and 2 * 3 * 4.')
    expect(plain).toContain('Snake_case_names and 2 * 3 * 4.')
  })

  it('splits a long answer into messages Slack takes, each block of code closed, and says where the rest is', () => {
    const paragraphs = Array.from({ length: 60 }, (_, index) => `Paragraph ${String(index)}: ${'word '.repeat(100)}`).join('\n\n')
    const messages = slackMessages('Ada', paragraphs)
    expect(messages).toHaveLength(CHAT_MAX_PARTS)
    expect(messages.at(-1)!.endsWith(SLACK_REST_IN_LOCUST)).toBe(true)
    for (const message of messages) expect(message.length).toBeLessThanOrEqual(SLACK_MESSAGE_MAX)
    const code = slackBlocks(`\`\`\`\n${Array.from({ length: 400 }, (_, index) => `line ${String(index)} ${'x'.repeat(30)}`).join('\n')}\n\`\`\``)
    expect(code.length).toBeGreaterThan(1)
    for (const block of code) {
      expect(fences(block)).toBe(2)
      expect(block.length).toBeLessThanOrEqual(SLACK_MESSAGE_MAX)
    }
  })

  it('reads a person’s words back from Slack’s markup', () => {
    expect(slackPlain('see <https://example.com/a?b=1&amp;c=2|the docs> &amp; <https://example.com>')).toBe('see the docs (https://example.com/a?b=1&c=2) & https://example.com')
    expect(slackPlain('ask <@U0SAM00001> in <#C0GENERAL1|general>, <!here> 1 &lt; 2')).toBe('ask @someone in #general, @here 1 < 2')
    expect(slackEscape('a < b & c > d')).toBe('a &lt; b &amp; c &gt; d')
  })

  it('knows each token by its shape', () => {
    // Made-up tokens, put together here so no file holds one whole for a secret scanner to take for real.
    const bot = ['xoxb', '1000000000001', '2000000000002', 'fake'.repeat(6)].join('-')
    const app = ['xapp', '1', 'A0LOCUST01', '3000000000003', 'fake'.repeat(5)].join('-')
    const user = ['xoxp', '1000000000001', '2000000000002', 'fake'.repeat(6)].join('-')
    expect(SLACK_BOT_TOKEN.test(bot)).toBe(true)
    expect(SLACK_APP_TOKEN.test(app)).toBe(true)
    expect(SLACK_BOT_TOKEN.test(app)).toBe(false)
    expect(SLACK_APP_TOKEN.test(user)).toBe(false)
  })

  it('asks Slack for an app that reads only its direct messages, over Socket Mode, with nothing of Locust’s to reach', () => {
    expect(SLACK_MANIFEST.settings.socket_mode_enabled).toBe(true)
    expect(SLACK_MANIFEST.settings.event_subscriptions.bot_events).toEqual(['message.im'])
    expect([...SLACK_MANIFEST.oauth_config.scopes.bot].sort()).toEqual(['chat:write', 'im:history', 'reactions:write', 'users:read'])
    expect(SLACK_MANIFEST.features.app_home).toEqual({ home_tab_enabled: false, messages_tab_enabled: true, messages_tab_read_only_enabled: false })
    expect(JSON.stringify(SLACK_MANIFEST)).not.toMatch(/request_url|https?:\/\//)
    expect(SLACK_MANIFEST.display_information.name.length).toBeLessThanOrEqual(35)
    expect(SLACK_MANIFEST.display_information.description.length).toBeLessThanOrEqual(140)
    const link = new URL(SLACK_MANIFEST_LINK)
    expect(`${link.origin}${link.pathname}`).toBe('https://api.slack.com/apps')
    expect(link.searchParams.get('new_app')).toBe('1')
    expect(JSON.parse(link.searchParams.get('manifest_json')!)).toEqual(SLACK_MANIFEST)
  })
})
