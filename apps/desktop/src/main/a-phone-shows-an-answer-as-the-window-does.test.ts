import { describe, expect, it } from 'vitest'

import { PHONE_ANSWER, phonePage } from './phone-page.js'

/**
 * AN ANSWER READS ON THE PHONE AS IT DOES IN THE WINDOW (0.845).
 *
 * The phone showed an answer's markdown as typed: Kit's three bullets came out as "- **Plan header**: …" while the
 * window beside it drew a list with bold names (filming the launch clip, 2026-10-31). The page now draws paragraphs,
 * lists, headings, code and bold, and still puts every word from a run in as text, never as HTML.
 *
 * The page's own renderer runs here, word for word, over an `el` and `txt` that record what they were asked to make.
 */
interface Made {
  readonly tag: string
  readonly className?: string
  readonly text?: string
  readonly children: Made[]
  append(child: Made): void
  readonly lastChild: Made | undefined
}
function el(tag: string, attrs?: Record<string, unknown> | null, children?: readonly (Made | null)[]): Made {
  const made: Made = {
    tag,
    ...(typeof attrs?.class === 'string' ? { className: attrs.class } : {}),
    ...(typeof attrs?.text === 'string' ? { text: attrs.text } : {}),
    children: [],
    append(child) { this.children.push(child) },
    get lastChild() { return this.children.at(-1) }
  }
  for (const child of children ?? []) if (child) made.children.push(child)
  return made
}
const txt = (text: string): Made => ({ tag: '#text', text, children: [], append() { throw new Error('a text node has no children') }, lastChild: undefined })
const { answerOf, plainOf } = new Function('el', 'txt', `${PHONE_ANSWER}\nreturn { answerOf: answerOf, plainOf: plainOf }`)(el, txt) as {
  answerOf: (source: string) => Made
  plainOf: (source: string) => string
}
/** What was made, as compact markup: text as it is, elements by tag (and class). */
const shape = (made: Made): string =>
  made.tag === '#text' ? made.text ?? '' : `<${made.tag}${made.className === undefined ? '' : `.${made.className}`}>${made.text ?? ''}${made.children.map(shape).join('')}</${made.tag}>`
const tags = (made: Made): string[] => [made.tag, ...made.children.flatMap(tags)]

describe('an answer reads on the phone as it does in the window', () => {
  it('draws the bullets Kit sent the phone as a list with bold names', () => {
    const kit = '- **Plan header**: The weekend name and proposed dates set by the host.\n- **Availability grid**: A day-by-day time grid.\n- **Share prompt**: An "Invite friends" button.'
    expect(shape(answerOf(kit))).toBe(
      '<div.answer><ul>' +
        '<li><strong>Plan header</strong>: The weekend name and proposed dates set by the host.</li>' +
        '<li><strong>Availability grid</strong>: A day-by-day time grid.</li>' +
        '<li><strong>Share prompt</strong>: An "Invite friends" button.</li>' +
        '</ul></div>'
    )
  })

  it('draws paragraphs, a heading, a numbered list, code and emphasis', () => {
    const source = '## Steps\nFirst, run `npm test`\nand read it.\n\n1. Open *it*\n2. Close __it__\n\n```js\nnpm run build\n  --watch\n```\nDone.'
    expect(shape(answerOf(source))).toBe(
      '<div.answer>' +
        '<p.h>Steps</p>' +
        '<p>First, run <code>npm test</code>\nand read it.</p>' +
        '<ol><li>Open <em>it</em></li><li>Close <strong>it</strong></li></ol>' +
        '<pre><code>npm run build\n  --watch</code></pre>' +
        '<p>Done.</p>' +
        '</div>'
    )
  })

  it('carries an item on over a line indented under it, and shows an unclosed fence as code', () => {
    expect(shape(answerOf('- one\n  more of one\n- two'))).toBe('<div.answer><ul><li>one more of one</li><li>two</li></ul></div>')
    expect(shape(answerOf('Run this:\n```\nls -la'))).toBe('<div.answer><p>Run this:</p><pre><code>ls -la</code></pre></div>')
  })

  it('keeps every word from a run as text, never as HTML, and a link as its words', () => {
    const made = answerOf('<img src=x onerror=alert(1)> **<b>bold</b>** [click](javascript:alert(1)) 2 * 3 * 4')
    expect(tags(made)).toEqual(['div', 'p', '#text', 'strong', '#text', '#text', '#text'])
    expect(shape(made)).toBe('<div.answer><p><img src=x onerror=alert(1)> <strong><b>bold</b></strong> click 2 * 3 * 4</p></div>')
  })

  it('gives the list of conversations the words without the marks', () => {
    expect(plainOf('- **Plan header**: The `weekend` name\n## Next\n```\ncode\n```\n[docs](https://example.com) __done__')).toBe('Plan header: The weekend name Next code docs done')
  })

  it('is in the page both ways, drawing the thread and the snippets', () => {
    for (const via of ['lan', 'relay'] as const) {
      const page = phonePage({ via })
      expect(page).toContain(PHONE_ANSWER)
      expect(page).toContain('said.append(answerOf(turn.answer))')
      expect(page).toContain("text: plainOf(c.snippet)")
      expect(page).not.toContain("el('div', { class: 'answer', text: turn.answer })")
    }
  })
})
