import { describe, expect, it } from 'vitest'

import { CHAT_MAX_PARTS } from './chat-apps.js'
import { DISCORD_MESSAGE_MAX, DISCORD_REST_IN_LOCUST, DISCORD_TOKEN, discordBlocks, discordEscape, discordInvite, discordMessages } from './discord.js'

/**
 * AN ANSWER READS RIGHT IN DISCORD (0.832, shared/discord.ts). Discord reads most of a teammate's Markdown itself, so
 * an answer goes nearly as written -- but in messages of 2,000 characters at most, with every block of code closed in
 * the message it opens in, a table as a block of code, and the marks it does not read made into ones it does.
 */
const ANSWERS: readonly string[] = [
  '# Plan\n\nFirst **read** the `config.ts` file, then *run* the tests.\n\n- one\n- two\n  - nested\n\n1. first\n2. second',
  '```ts\nconst a = 1 < 2 && b > 3\nconsole.log("<b>not a tag</b>")\n```\n\nDone.',
  '| File | Lines |\n|---|---|\n| a.ts | 12 |\n| b & c.ts | 40 |\n\nThat is the table.',
  '> A quote with **bold**\n> and a [link](https://example.com/a?b=1&c=2)\n\nAfter it.',
  '#### A small heading\n\n![the chart](https://example.com/chart.png)\n\n---\n\nAfter a rule.',
  '**unclosed bold and `unclosed code and [broken](link',
  '```\ncode with ``` inside it\n```',
  ''
]
const fences = (message: string): number => message.split('\n').filter((line) => line.startsWith('```')).length

describe('an answer reads right in Discord', () => {
  it('turns every answer into messages under Discord’s limit, each block of code closed where it opens', () => {
    for (const answer of ANSWERS) {
      const messages = discordMessages('Ada', answer)
      expect(messages[0]!.startsWith('**Ada**'), answer).toBe(true)
      for (const message of messages) {
        expect(message.length, message).toBeLessThanOrEqual(2_000)
        expect(fences(message) % 2, message).toBe(0)
      }
    }
  })

  it('keeps what Discord reads, and makes the rest into what it does', () => {
    const [plan] = discordMessages('Ada', ANSWERS[0]!)
    expect(plan).toContain('# Plan')
    expect(plan).toContain('First **read** the `config.ts` file, then *run* the tests.')
    expect(plan).toContain('- one\n- two\n  - nested')
    const [code] = discordMessages('Ada', ANSWERS[1]!)
    expect(code).toContain('```ts\nconst a = 1 < 2 && b > 3\nconsole.log("<b>not a tag</b>")\n```')
    const [table] = discordMessages('Ada', ANSWERS[2]!)
    expect(table).toContain('```\n| File | Lines |\n| a.ts | 12 |\n| b & c.ts | 40 |\n```')
    const [quote] = discordMessages('Ada', ANSWERS[3]!)
    expect(quote).toContain('> A quote with **bold**\n> and a [link](https://example.com/a?b=1&c=2)')
    // A heading Discord has no size for is bold; a picture is a link to it; a rule is nothing.
    const [small] = discordMessages('Ada', ANSWERS[4]!)
    expect(small).toBe('**Ada**\n\n**A small heading**\n\n[the chart](https://example.com/chart.png)\n\nAfter a rule.')
    // Three backticks inside a block of code do not end it early.
    const [inside] = discordMessages('Ada', ANSWERS[6]!)
    expect(fences(inside)).toBe(2)
  })

  it('splits a long answer between blocks, and says where the rest is past four messages', () => {
    const paragraphs = Array.from({ length: 40 }, (_, index) => `Paragraph ${String(index)}: ${'word '.repeat(80)}`).join('\n\n')
    const messages = discordMessages('Ada', paragraphs)
    expect(messages).toHaveLength(CHAT_MAX_PARTS)
    expect(messages.at(-1)!.endsWith(DISCORD_REST_IN_LOCUST)).toBe(true)
    for (const message of messages) expect(message.length).toBeLessThanOrEqual(DISCORD_MESSAGE_MAX)
    expect(discordMessages('Ada', 'Done.')).toEqual(['**Ada**\n\nDone.'])
  })

  it('cuts a block of code too long for one message between its lines, each piece fenced again', () => {
    const code = `\`\`\`js\n${Array.from({ length: 300 }, (_, index) => `const line${String(index)} = "${'x'.repeat(20)}"`).join('\n')}\n\`\`\``
    const blocks = discordBlocks(code)
    expect(blocks.length).toBeGreaterThan(1)
    for (const block of blocks) {
      expect(block.startsWith('```js\n')).toBe(true)
      expect(block.endsWith('\n```')).toBe(true)
      expect(block.length).toBeLessThanOrEqual(DISCORD_MESSAGE_MAX)
    }
    // Every line is still there, once.
    expect(blocks.flatMap((block) => block.split('\n').slice(1, -1))).toHaveLength(300)
    const long = discordBlocks('word '.repeat(2_000))
    expect(long.length).toBeGreaterThan(1)
    for (const block of long) expect(block.length).toBeLessThanOrEqual(DISCORD_MESSAGE_MAX)
  })

  it('escapes plain words so none of them is read as a mark', () => {
    expect(discordEscape('snake_case *star* `tick` ~strike~ |pipe| [a](b) <@123> @everyone \\ back')).toBe('snake\\_case \\*star\\* \\`tick\\` \\~strike\\~ \\|pipe\\| \\[a\\](b) \\<\\@123> \\@everyone \\\\ back')
    expect(discordEscape('# not a heading\n> not a quote\n- not a list\n1. not a number')).toBe('\\# not a heading\n\\> not a quote\n\\- not a list\n1\\. not a number')
    // In the middle of a line, those are only words.
    expect(discordEscape('Settings > General - Chat apps #2')).toBe('Settings > General - Chat apps #2')
  })

  it('knows a bot’s token by its shape, and adds the bot to a server asking no permissions', () => {
    expect(DISCORD_TOKEN.test('MTEwMDAwMDAwMDAwMDAwMDAwMQ.GfakeA.fake_fake_fake_fake_fake_fake_fake_fak')).toBe(true)
    expect(DISCORD_TOKEN.test('MTEwMDAwMDAwMDAwMDAwMDAwMQ.GfakeA')).toBe(false)
    expect(DISCORD_TOKEN.test('123456789:AAHfakefakefakefakefakefakefakefake1')).toBe(false)
    expect(discordInvite('1100000000000000001')).toBe('https://discord.com/oauth2/authorize?client_id=1100000000000000001&scope=bot%20applications.commands&permissions=0')
  })
})
