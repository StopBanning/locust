import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

/**
 * Every `lc-` class a component writes is a class the stylesheet knows.
 *
 * A class name with no rule behind it does not warn, does not fail, and does
 * not look wrong in the source: the element renders, carries the name, and
 * draws nothing. It is the same silent defect as the undefined `--lc-border`
 * token that `css-tokens-exist` was written for, one level up.
 *
 * Written 2026-09-09, immediately after styling a button as
 * `lc-button--quiet` -- a name nothing defined -- while building the
 * teammate folder row. On its first run it found four more, all on the
 * Automations screen: `lc-screen__head`, `lc-screen__lede`, `lc-screen__note`
 * and `lc-section__title`. Every other screen's header is a 60px chrome bar
 * with a hairline and 24px of side padding, drawn by `lc-screen__header`;
 * Automations asked for `__head`, got nothing, and had been shipping a title
 * flush against the window edge.
 *
 * Names built by interpolation are out of scope on purpose. `lc-tone-${...}`
 * cannot be read from the source, and `tone-classes-win` is the control that
 * covers those.
 */

const RENDERER = fileURLToPath(new URL('../renderer/src/', import.meta.url))

/**
 * Classes that carry no rule ON PURPOSE: they exist to be reached, by a
 * drive's selector or a test's query, and the element's look comes from its
 * parent. Each one was checked against the stylesheet when it was added here.
 */
const REACHED_NOT_STYLED: ReadonlySet<string> = new Set([
  // A drive's selector for the Automations screen body.
  'lc-automations',
  // A Board card that answers a request in place (0.835): drawn as every `lc-boardwait` is, marked so
  // drive-conversation-board can tell it from a Scout or routine card.
  'lc-boardwait--ask',
  // Your own benchmark's table (0.763): drawn as every `lc-table` is, marked so drive-a-benchmark-runs-again reads its rows.
  'lc-benchmark__table',
  // The empty Home's team offer: drawn as the `lc-hometeam` section it shares,
  // marked so drive-team-templates can tell the offer from the team.
  'lc-teamtemplates',
  // A room's row in the conversation list (withRoomsFolded): drawn as any
  // `lc-conv`, marked so drive-room can find the room it stands for.
  'lc-conv--room',
  // The row wrapping one file's diff; `lc-filerow` and `DiffView` draw it.
  'lc-approval__file',
  // The composer's form is drawn by `command-dock`, which it also carries.
  'lc-composer__form',
  // Text inside a menu item; the item's own rule sets the type.
  'lc-context__label',
  // The sending runtime's name. `lc-handoff__title` sets the type for both
  // halves and only the RECEIVING half is lifted out of muted.
  'lc-handoff__from',
  // The post's words, inside `lc-roompost__you`, which sets the bubble.
  'lc-roompost__text',
  // A room's @ menu and its recipient tiles (0.371): drawn as the command
  // menu (`lc-slash`) and an attachment tile (`lc-attached__tile`) they also
  // carry, and named so drive-room-ask-one can tell them from those.
  'lc-mentions',
  'lc-askto__tile',
  // The notes tile in the chat box (0.376): an attachment tile it also
  // carries draws it; named so drive-diff-notes can find it.
  'lc-notestile',
  // A namespace, never worn alone: every use pairs it with `--switch`,
  // `--roletitle` or `--folder`, and those carry the layout.
  'lc-field',
  // Both of these pair with a class that draws them -- `lc-tag` and
  // `lc-runtimerow__detail` -- and name what the row IS so a drive can find
  // it.
  'lc-task__state',
  'lc-runtimerow__usage',
  // The paragraph still being written. It draws EXACTLY as `lc-para` on
  // purpose -- the whole point is that nothing changes shape when it settles
  // -- so a rule of its own would be a rule that must stay empty. Named so a
  // drive can find the arriving tail.
  'lc-para--arriving'
])

function sources(): readonly string[] {
  const found: string[] = []
  const walk = (directory: string): void => {
    for (const name of readdirSync(directory)) {
      const path = join(directory, name)
      if (statSync(path).isDirectory()) {
        walk(path)
        continue
      }
      if (/\.tsx?$/.test(name) && !name.includes('.test.')) found.push(path)
    }
  }
  walk(RENDERER)
  return found.sort()
}

/** Every class the stylesheets have a rule for. */
function styled(): ReadonlySet<string> {
  const css = ['shell.css', 'tokens.css'].map((file) => readFileSync(join(RENDERER, file), 'utf8')).join('\n')
  return new Set([...css.matchAll(/\.(lc-[A-Za-z0-9_-]+)/g)].map((match) => match[1]))
}

/**
 * Every literal `lc-` class one file writes into a `className`.
 *
 * Only the literal head of the attribute, which is where a hand-typed name
 * lives. Anything after a `${` in the same attribute is a value this cannot
 * read and is left to the controls that can.
 */
export function classesNamed(source: string): readonly string[] {
  const found: string[] = []
  for (const match of source.matchAll(/className=(?:"|'|\{`)([^"'`$]*)/g)) {
    for (const word of match[1].split(/\s+/)) {
      // A name ending in a separator is the literal HEAD of an interpolated
      // one -- `lc-tone-${tone}` reads as `lc-tone-` here -- and the class
      // that really lands cannot be read from the source. `tone-classes-win`
      // is the control for those.
      if (/^lc-[A-Za-z0-9_-]+$/.test(word) && !word.endsWith('-')) found.push(word)
    }
  }
  return found
}

describe('the classes the components write', () => {
  it('finds them, and would really notice one that is not styled', () => {
    // The control. An extractor that returns nothing reports every file
    // clean by looking at no classes.
    const seen = sources().flatMap((path) => classesNamed(readFileSync(path, 'utf8')))
    expect(seen.length).toBeGreaterThan(400)
    expect(seen).toContain('lc-screen__header')

    // THE defect, in the shape it took: a name typed into a component and
    // defined nowhere.
    expect(classesNamed('<button className="lc-button lc-button--quiet">')).toEqual(['lc-button', 'lc-button--quiet'])
    // And an interpolated name contributes nothing rather than its stem.
    expect(classesNamed('<span className={`lc-tone-${tone}`}>')).toEqual([])
    expect(styled().has('lc-button--quiet')).toBe(false)
    // And a name that IS defined must not be reported, or this would pass by
    // objecting to everything.
    expect(styled().has('lc-button')).toBe(true)
  })

  it('are all styled, or listed as reached rather than styled', () => {
    const rules = styled()
    const missing = new Map<string, string[]>()
    for (const path of sources()) {
      for (const name of classesNamed(readFileSync(path, 'utf8'))) {
        if (rules.has(name) || REACHED_NOT_STYLED.has(name)) continue
        missing.set(name, [...(missing.get(name) ?? []), path.slice(RENDERER.length)])
      }
    }
    expect(
      [...missing].map(([name, files]) => `${name} (${files.join(', ')})`),
      'a class with no rule draws nothing and looks deliberate -- style it, or list it in REACHED_NOT_STYLED with why'
    ).toEqual([])
  })
})
