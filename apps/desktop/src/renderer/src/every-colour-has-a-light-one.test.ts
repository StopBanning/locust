import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

/**
 * EVERY COLOUR HAS A LIGHT ONE (0.789). A colour token added to the dark
 * theme and not to the light one is drawn in light in its dark value -- ivory
 * text on paper, a hairline of white on white -- and nothing would say so
 * until someone opened that screen in light. So each colour in `:root` is
 * either given again under `:root[data-theme='light']`, or named here as the
 * same in both themes, with the reason.
 */
const tokens = readFileSync(new URL('./tokens.css', import.meta.url), 'utf8')

function block(selector: string): string {
  const start = tokens.indexOf(`${selector} {`)
  if (start < 0) throw new Error(`no ${selector} block in tokens.css`)
  return tokens.slice(start, tokens.indexOf('\n}', start))
}

const COLOUR = /#[0-9a-f]{3,8}\b|rgba?\(/i
const declared = (text: string): Map<string, string> =>
  new Map([...text.matchAll(/^\s+(--lc-[a-z0-9-]+):\s*([^;]+);/gm)].map((match) => [match[1]!, match[2]!.trim()]))

/** The same in both themes, on purpose. */
const SHARED = [
  // Who a teammate is: its hue and the ink its face is drawn in (pearl alone is greyed, below).
  /^--lc-hue-(?!pearl$)/,
  // The runtimes' own brand marks.
  /^--lc-mark-/,
  // A file's kind: each language's own colour, on a badge that carries its own ink, as editors draw it on either ground.
  /^--lc-filetype-/,
  // The boot screen is a lit screen in a dark bezel, in any room.
  /^--lc-boot-/,
  // Find's current hit is solid amber with dark letters, which reads on either ground.
  /^--lc-find-now/,
  // A white slider thumb, on either track.
  /^--lc-effort-thumb$/
]

describe('the light theme', () => {
  const dark = declared(block(':root'))
  const light = declared(block(":root[data-theme='light']"))

  it('gives every colour token a light value, or says why it is the same in both', () => {
    const missing = [...dark]
      .filter(([, value]) => COLOUR.test(value))
      .map(([name]) => name)
      .filter((name) => !light.has(name) && !SHARED.some((pattern) => pattern.test(name)))
    expect(missing).toEqual([])
  })

  it('names only tokens the dark theme has, so a typo cannot leave a colour behind', () => {
    expect([...light.keys()].filter((name) => !dark.has(name))).toEqual([])
  })

  it('draws its readable text dark enough on its own window', () => {
    const luminance = (hex: string): number => {
      const [r, g, b] = [1, 3, 5].map((at) => Number.parseInt(hex.slice(at, at + 2), 16) / 255).map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4))
      return 0.2126 * r! + 0.7152 * g! + 0.0722 * b!
    }
    const contrast = (a: string, b: string): number => {
      const [high, low] = [luminance(a), luminance(b)].sort((x, y) => y - x)
      return (high! + 0.05) / (low! + 0.05)
    }
    const window = light.get('--lc-bg-window')!
    const sidebar = light.get('--lc-bg-sidebar')!
    for (const name of ['--lc-text-primary', '--lc-text-secondary', '--lc-text-muted', '--lc-green-text', '--lc-blue-text', '--lc-amber-text', '--lc-red-text', '--lc-violet-text']) {
      expect(contrast(light.get(name)!, window), `${name} on the window`).toBeGreaterThanOrEqual(4.5)
      expect(contrast(light.get(name)!, sidebar), `${name} on the sidebar`).toBeGreaterThanOrEqual(4.5)
    }
  })
})
