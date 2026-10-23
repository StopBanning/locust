import { describe, expect, it } from 'vitest'

import { htmlProblem } from '../../../../_tools/fake-telegram.mjs'
import {
  answerMessages, packedMessages, parsedCommand, plainOf, teammateCommands, telegramBlocks, TELEGRAM_MAX_PARTS,
  TELEGRAM_MESSAGE_MAX, TELEGRAM_REST_IN_LOCUST, TELEGRAM_TOKEN
} from './telegram.js'

/**
 * AN ANSWER READS RIGHT IN TELEGRAM (0.831, shared/telegram.ts). A teammate answers in Markdown, and Telegram takes a
 * small HTML and refuses a whole message over one bad tag. Every answer here is checked by the stand-in's parser
 * (_tools/fake-telegram.mjs), which is as strict as Telegram's on the tags Locust sends: whatever Locust makes of an
 * answer, Telegram takes, in messages it allows.
 */
const ANSWERS: readonly string[] = [
  '# Plan\n\nFirst **read** the `config.ts` file, then *run* the tests.\n\n- one\n- two\n  - nested\n\n1. first\n2. second',
  '```ts\nconst a = 1 < 2 && b > 3\nconsole.log("<b>not a tag</b>")\n```\n\nDone.',
  '| File | Lines |\n|---|---|\n| a.ts | 12 |\n| b & c.ts | 40 |\n\nThat is the table.',
  '> A quote with **bold**\n> and a [link](https://example.com/a?b=1&c=2)\n\nAfter it.',
  'Snake_case_names and 2 * 3 * 4 and a lone * star, an <angle> & an ampersand.',
  '**unclosed bold and `unclosed code and [broken](link',
  '~~struck~~ and __not bold here__ and ***both*** and ** spaced **.',
  'A link [with **bold** inside](https://example.com) and **bold [with a link](https://example.com) inside**.',
  '```\nno language\n```\n~~~python\nprint("tilde fence")\n~~~',
  '---\n\nAfter a rule.\n\n***\n',
  ''
]

describe('an answer reads right in Telegram', () => {
  it('turns every answer into HTML Telegram takes, in messages under its limit', () => {
    for (const answer of ANSWERS) {
      const messages = answerMessages('Ada', answer)
      expect(messages[0]!.startsWith('<b>Ada</b>'), answer).toBe(true)
      for (const message of messages) {
        expect(htmlProblem(message), `${answer}\n=>\n${message}`).toBeUndefined()
        expect(message.length).toBeLessThanOrEqual(TELEGRAM_MESSAGE_MAX)
      }
    }
  })

  it('keeps what Markdown meant: bold, code, blocks of code, links, lists, quotes and tables', () => {
    const [plan] = answerMessages('Ada', ANSWERS[0]!)
    expect(plan).toContain('<b>Plan</b>')
    expect(plan).toContain('<b>read</b>')
    expect(plan).toContain('<code>config.ts</code>')
    expect(plan).toContain('<i>run</i>')
    expect(plan).toContain('• one')
    expect(plan).toContain('  • nested')
    const [code] = answerMessages('Ada', ANSWERS[1]!)
    expect(code).toContain('<pre><code class="language-ts">const a = 1 &lt; 2 &amp;&amp; b &gt; 3')
    expect(code).toContain('&lt;b&gt;not a tag&lt;/b&gt;')
    const [table] = answerMessages('Ada', ANSWERS[2]!)
    expect(table).toContain('<pre>| File | Lines |\n| a.ts | 12 |\n| b &amp; c.ts | 40 |</pre>')
    const [quote] = answerMessages('Ada', ANSWERS[3]!)
    expect(quote).toContain('<blockquote>A quote with <b>bold</b>\nand a <a href="https://example.com/a?b=1&amp;c=2">link</a></blockquote>')
    // Names with underscores and arithmetic are not emphasis.
    const [plain] = answerMessages('Ada', ANSWERS[4]!)
    expect(plain).toContain('Snake_case_names and 2 * 3 * 4 and a lone * star, an &lt;angle&gt; &amp; an ampersand.')
  })

  it('splits a long answer between blocks, and says where the rest is past four messages', () => {
    const paragraphs = Array.from({ length: 40 }, (_, index) => `Paragraph ${String(index)}: ${'word '.repeat(80)}`).join('\n\n')
    const messages = answerMessages('Ada', paragraphs)
    expect(messages).toHaveLength(TELEGRAM_MAX_PARTS)
    expect(messages.at(-1)!.endsWith(TELEGRAM_REST_IN_LOCUST)).toBe(true)
    for (const message of messages) {
      expect(htmlProblem(message)).toBeUndefined()
      expect(message.length).toBeLessThanOrEqual(TELEGRAM_MESSAGE_MAX)
    }
    // A short answer is one message, with nothing said about a rest.
    expect(answerMessages('Ada', 'Done.')).toEqual(['<b>Ada</b>\n\nDone.'])
  })

  it('cuts a block of code too long for one message between its lines, each piece whole', () => {
    const code = `\`\`\`js\n${Array.from({ length: 400 }, (_, index) => `const line${String(index)} = "${'x'.repeat(20)}" // <${String(index)}>`).join('\n')}\n\`\`\``
    const blocks = telegramBlocks(code)
    expect(blocks.length).toBeGreaterThan(1)
    for (const block of blocks) {
      expect(block.startsWith('<pre><code class="language-js">')).toBe(true)
      expect(htmlProblem(block)).toBeUndefined()
      expect(block.length).toBeLessThanOrEqual(TELEGRAM_MESSAGE_MAX)
    }
    // Every line is still there, once.
    expect(blocks.map(plainOf).join('\n').split('\n')).toHaveLength(400)
    // A paragraph longer than a message is split between words.
    const long = telegramBlocks('word '.repeat(2_000))
    expect(long.length).toBeGreaterThan(1)
    for (const block of long) expect(block.length).toBeLessThanOrEqual(TELEGRAM_MESSAGE_MAX)
    expect(packedMessages(['a', 'b'])).toEqual(['a\n\nb'])
  })

  it('names each teammate by a command Telegram allows, never one of Locust’s own or one taken', () => {
    const commands = teammateCommands([
      { teammateId: 't1', name: 'Ada' },
      { teammateId: 't2', name: 'ada' },
      { teammateId: 't3', name: 'Help' },
      { teammateId: 't4', name: 'Zoë Ng-Smith' },
      { teammateId: 't5', name: 'Борис' },
      { teammateId: 't6', name: 'A really long teammate name that goes past thirty-two letters' },
      { teammateId: 't7', name: 'Who' }
    ])
    expect([...commands.values()]).toEqual(['ada', 'ada2', 'help2', 'zoe_ng_smith', 'teammate', 'a_really_long_teammate_name', 'who2'])
    for (const command of commands.values()) expect(command).toMatch(/^[a-z0-9_]{1,32}$/)
  })

  it('reads a command as Telegram sends one, and leaves another bot’s alone', () => {
    expect(parsedCommand('/ada check the build', 'locust_test_bot')).toEqual({ command: 'ada', rest: 'check the build' })
    expect(parsedCommand('/Ada@Locust_Test_Bot  hello\nthere ', 'locust_test_bot')).toEqual({ command: 'ada', rest: 'hello\nthere' })
    expect(parsedCommand('/ada@other_bot hello', 'locust_test_bot')).toBeUndefined()
    expect(parsedCommand('just words', 'locust_test_bot')).toBeUndefined()
    expect(parsedCommand('/start Zx9_-abc', 'locust_test_bot')).toEqual({ command: 'start', rest: 'Zx9_-abc' })
  })

  it('knows a bot’s token by its shape', () => {
    expect(TELEGRAM_TOKEN.test('123456789:AAHfakefakefakefakefakefakefakefake1')).toBe(true)
    expect(TELEGRAM_TOKEN.test('123456789:short')).toBe(false)
    expect(TELEGRAM_TOKEN.test(' 123456789:AAHfakefakefakefakefakefakefakefake1')).toBe(false)
    expect(TELEGRAM_TOKEN.test('sk-ant-123456789012345678901234567890')).toBe(false) // public-guard: allow (a made-up key in a test)
  })
})
