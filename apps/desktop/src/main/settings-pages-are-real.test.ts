import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

/**
 * The settings search knows every setting that is actually on the page.
 *
 * `SETTINGS_PAGES` lists each page's headings so that typing "auto" finds
 * Auto mode rather than only matching page names. That list is written by
 * hand beside JSX that renders the headings, which is exactly the shape that
 * drifts: add a section, forget the index, and the search quietly stops
 * finding it. Nothing on screen looks wrong, which is what makes it bad.
 *
 * So the index is held to the file: every `lc-settings__heading` the screen
 * renders is in it, and nothing is in it that the screen does not render.
 *
 * Both files are READ rather than imported, which is how the other guards
 * here work (`classes-are-styled`, `says-what-is-still-true`): the main
 * tsconfig does not list renderer sources, and adding them to it to satisfy a
 * test would change what the app compiles.
 */

const source = (path: string): string => readFileSync(fileURLToPath(new URL(path, import.meta.url)), 'utf8')
const SCREENS = source('../renderer/src/components/Screens.tsx')
const PAGES = source('../renderer/src/settingsPages.ts')

/** Every settings heading the screen renders, in the order it renders them. */
function renderedHeadings(): readonly string[] {
  const found: string[] = []
  for (const match of SCREENS.matchAll(/<h2 className="lc-settings__heading">([^<]+)<\/h2>/g)) {
    const text = (match[1] ?? '').trim()
    // JSX writes `&amp;` for an ampersand; the index holds what a person reads.
    if (text.length > 0) found.push(text.replace(/&amp;/g, '&'))
  }
  return found
}

/** Every heading named in the index, however the entries are formatted. */
function indexedHeadings(): readonly string[] {
  const found: string[] = []
  for (const block of PAGES.matchAll(/headings:\s*\[([^\]]*)\]/g)) {
    for (const quoted of (block[1] ?? '').matchAll(/'([^']+)'/g)) found.push(quoted[1] ?? '')
  }
  return found
}

/** Every page id the index declares. */
function pageIds(): readonly string[] {
  return [...PAGES.matchAll(/\bid:\s*'([^']+)'/g)].map((match) => match[1] ?? '')
}

describe('the settings pages', () => {
  it('are found at all, so this cannot pass by reading nothing', () => {
    // The control. An extractor that matches nothing would call every index
    // complete, including an empty one.
    expect(renderedHeadings().length).toBeGreaterThan(10)
    expect(indexedHeadings().length).toBeGreaterThan(10)
    expect(pageIds().length).toBeGreaterThan(3)
  })

  it('index every heading the screen renders, and invent none', () => {
    // One heading may be written on two branches (0.849): a section a browser draws in its place, and the section.
    expect([...indexedHeadings()].sort()).toEqual([...new Set(renderedHeadings())].sort())
  })

  it('are the areas the screen actually splits on', () => {
    // Each page boundary in the JSX is written as `shownPage === '<id>'`, so a
    // page in the index with no branch would render nothing at all.
    for (const id of pageIds()) {
      expect(SCREENS, id).toContain(`shownPage === '${id}'`)
    }
    // And two pages with one id would make one of them unreachable.
    expect(new Set(pageIds()).size).toBe(pageIds().length)
  })
})
