import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

import { REMOTE_CONTROL_LABEL } from '../shared/claude-remote-control.js'

/**
 * No harness may select something this app cannot draw.
 *
 * A drive that queries a class the renderer no longer produces gets back
 * nothing — and nothing is exactly what a missing feature looks like. So the
 * app changes, a drive keeps passing or keeps reporting an absence, and the
 * lie survives until someone spends an afternoon on it.
 *
 * That has happened three times in this repo. Drives guarded on an `is-open`
 * class `ActivityCard` has never set, so the guard was always true. A drive
 * read the user's message bubble as `.lc-msg, .lc-turn, [class*="user"]`,
 * matched nothing, printed `[]`, and a screenshot happened to cover for it.
 * `verify-firstrun` counted runtime rows with three selectors that between
 * them matched none, and printed `runtime rows: []` as if that were an answer.
 *
 * The rule here is narrow on purpose: a `querySelector` string is a failure
 * only when it names at least one `lc-` class and NONE of the classes in it
 * exist. A selector with one live branch is fine — that is what a fallback
 * chain is for — and a selector with no `lc-` class at all is not this test's
 * business.
 *
 * `aria-label` selectors are checked too, and they rot the same way: a changed
 * avatar title once broke 68 of them at once. Only EXACT (`[aria-label="x"]`)
 * and PREFIX (`[aria-label^="x"]`) forms, against every string literal the app
 * puts in an `aria-label` — including the ones inside a JSX expression, which
 * is the false-positive mode this check had on its first run. It reported five
 * dead selectors for `Save as routine` and `Edit routine`; both live inside
 * `aria-label={editing ? 'Edit routine' : 'Save as routine'}`, and "fixing"
 * those five would have broken two working smokes. The app was read before
 * anything was changed, which is the only reason it did not happen.
 *
 * A label held in a named string constant is read as its words. Any other
 * label built entirely from a variable cannot be judged here and is not
 * claimed to be.
 */

const ROOT = fileURLToPath(new URL('../../../../', import.meta.url))
const SRC = join(ROOT, 'apps', 'desktop', 'src')

/**
 * Selectors that match nothing ON PURPOSE.
 *
 * An assertion that something is ABSENT has to name it to look for it, so
 * "matches nothing" is its passing state. Each entry needs a reason.
 */
const ABSENCE_ASSERTIONS: readonly { readonly file: string; readonly selector: string; readonly why: string }[] = [
  {
    file: 'avatar-smoke.mjs',
    selector: 'progress, [role="progressbar"], .lc-progress',
    why: 'asserts "no progress bar exists anywhere" — matching nothing is the pass'
  },
  {
    file: 'drive-context-menu.mjs',
    selector: '.lc-context__title',
    why: 'the menu lost its title bar on purpose (a-context-menu-stays-in-the-window.test.tsx); "no title bar" is the pass'
  },
  {
    file: 'drive-teammate-model.mjs',
    selector: '.lc-sidebar__addmenu',
    why: 'the + opens the right-click menu instead of the old add menu; oldMenu === false is checked'
  },
  {
    file: 'drive-newline-hint.mjs',
    selector: '.lc-composer__newline',
    why: 'the Shift+Enter hint was removed on Colin\'s word (Composer.tsx); the drive reads it as ABSENT'
  },
  {
    file: 'probe-composer-row.mjs',
    selector: '.lc-workroom__context',
    why: 'the context ring left the conversation header for the chat box; ringStillInHeader false is the pass'
  }
]

function readAll(directory: string, endings: readonly string[]): string[] {
  const out: string[] = []
  const walk = (at: string): void => {
    for (const name of readdirSync(at)) {
      const path = join(at, name)
      if (statSync(path).isDirectory()) {
        walk(path)
        continue
      }
      if (endings.some((end) => name.endsWith(end)) && !name.includes('.test.')) {
        out.push(readFileSync(path, 'utf8'))
      }
    }
  }
  walk(directory)
  return out
}

const appSource = readAll(SRC, ['.tsx', '.ts', '.css'])
/*
 * What the app can DRAW is what a component renders -- never what the
 * stylesheet still has a rule for.
 *
 * This read `.css` too, so a control that was removed while its rule stayed
 * behind went on counting as drawable forever. Six drives were querying
 * classes no component renders and passing this test on the strength of dead
 * CSS: `.lc-attachchip`, `.lc-composer__newline`, the whole
 * `.lc-picker__effort*` family, `.lc-peer__open`, `.lc-workroom__context`.
 * Their assertions are unfalsifiable -- "chip drawn before choosing
 * anything" reads the absence of a class nothing can produce, so it is
 * always false and always green (Grok's audit, 2026-09-13, confirming its
 * own earlier finding was right and this test was wrong).
 *
 * `.ts` stays: a helper that assembles a class name is still the app
 * drawing it.
 */
const appClasses = new Set(
  readAll(SRC, ['.tsx', '.ts']).flatMap((text) => text.match(/lc-[a-zA-Z0-9_-]+/g) ?? [])
)

/*
 * Named string constants, so `aria-label={REMOTE_CONTROL_LABEL}` is read as
 * its words. Without this, the clean-machine drive's check that no claude.ai
 * switch is drawn on a machine without Claude Code was reported dead, although
 * the app draws that switch wherever Claude Code is installed (2026-10-16).
 */
const appConstants = new Map<string, string[]>()
for (const text of appSource) {
  for (const [, name, value] of text.matchAll(/\bconst ([A-Z][A-Z0-9_]*) = '([^']+)'/g)) {
    if (name !== undefined && value !== undefined) appConstants.set(name, [...(appConstants.get(name) ?? []), value])
  }
}

/** Every literal an `aria-label` can hold, plus the fixed prefix of a template one. */
const appLabels = new Set<string>()
const appLabelPrefixes: string[] = []
for (const text of appSource) {
  for (const [, literal] of text.matchAll(/aria-label="([^"{}]+)"/g)) appLabels.add(literal)
  // A line drawn without React, as Locust in a browser draws its own (desk-remote.ts, 0.834).
  for (const [, literal] of text.matchAll(/setAttribute\('aria-label', '([^']+)'\)/g)) appLabels.add(literal)
  // One level of `${...}` inside the braces, so a ternary of two templates is read whole (0.705).
  for (const [, expression] of text.matchAll(/aria-label=\{((?:[^{}]|\{[^{}]*\})*)\}/g)) {
    // A ternary of two literals is two literal labels. Missing this is what
    // made the first run of this check report five false positives.
    for (const value of appConstants.get(expression.trim()) ?? []) appLabels.add(value)
    const outsideTemplates = expression.replace(/`[^`]*`/g, '')
    for (const [, quoted] of outsideTemplates.matchAll(/'([^']+)'/g)) appLabels.add(quoted)
    for (const [, quoted] of outsideTemplates.matchAll(/"([^"]+)"/g)) appLabels.add(quoted)
    // And a ternary of two templates is two prefixes: `Resume ${name}` : `Pause ${name}` (0.705).
    for (const [, template] of expression.matchAll(/`([^`]*)`/g)) {
      const fixed = template.split('$')[0] ?? ''
      if (fixed.length > 0 && fixed !== template) appLabelPrefixes.push(fixed)
      else if (fixed.length > 0) appLabels.add(fixed)
    }
  }
  for (const [, template] of text.matchAll(/aria-label=\{`([^`]*)`\}/g)) {
    const fixed = template.split('$')[0] ?? ''
    if (fixed.length > 0) appLabelPrefixes.push(fixed)
  }
}

/** Whether anything the app draws could carry this label. */
function labelIsReachable(label: string, exact: boolean): boolean {
  if (exact && appLabels.has(label)) return true
  if (!exact && [...appLabels].some((known) => known.startsWith(label))) return true
  return appLabelPrefixes.some((prefix) => prefix.startsWith(label) || label.startsWith(prefix))
}

/*
 * querySelector AND querySelectorAll (0.712). This read `querySelectorAll?\(`,
 * which matches "querySelectorAll(" and "querySelectorAl(" -- never
 * "querySelector(". Every single-element lookup in every harness went
 * unchecked: 46 dead selectors in 34 files when it was fixed, among them the
 * first-hour drive's failure detector (`.lc-notice--error, .lc-failcard`,
 * neither drawn), so a failed first answer read as one still coming.
 */
const CALL = /querySelector(?:All)?\(\s*(['"`])(.*?)\1/gs

function deadSelectors(text: string, file: string): string[] {
  const dead: string[] = []
  for (const [, , selector] of text.matchAll(CALL)) {
    const classes = selector.match(/lc-[a-zA-Z0-9_-]+/g) ?? []
    if (classes.length === 0) continue
    if (classes.some((name) => appClasses.has(name))) continue
    const trimmed = selector.trim()
    if (ABSENCE_ASSERTIONS.some((entry) => entry.file === file && entry.selector === trimmed)) continue
    dead.push(trimmed)
  }
  return dead
}

const harnesses = ['_tools', '_smoke'].flatMap((folder) => {
  const at = join(ROOT, folder)
  return readdirSync(at)
    .filter((name) => name.endsWith('.mjs'))
    .map((name) => ({ name, text: readFileSync(join(at, name), 'utf8') }))
})

function deadLabels(text: string): string[] {
  const dead: string[] = []
  for (const [, label] of text.matchAll(/aria-label="([^"\]]+)"\]/g)) {
    if (!labelIsReachable(label, true)) dead.push(`[aria-label="${label}"]`)
  }
  for (const [, label] of text.matchAll(/aria-label\^="([^"\]]+)"\]/g)) {
    if (!labelIsReachable(label, false)) dead.push(`[aria-label^="${label}"]`)
  }
  return dead
}

describe('no harness selects something the app cannot draw', () => {
  it('found the app classes and the harnesses at all', () => {
    // Without this, a wrong path makes every case below vacuously green: no
    // classes known would flag everything, no harnesses found would flag
    // nothing.
    expect(appClasses.size).toBeGreaterThan(200)
    expect(harnesses.length).toBeGreaterThan(20)
    expect(appClasses.has('lc-bubble')).toBe(true)
    // The ternary case specifically, because missing it is what this check got
    // wrong first time.
    expect(appLabels.has('Save as routine')).toBe(true)
    // And a label held in a named constant, which it reported dead first time.
    expect(appLabels.has(REMOTE_CONTROL_LABEL)).toBe(true)
    expect(appLabels.size).toBeGreaterThan(40)
  })

  it('reads querySelector as well as querySelectorAll (0.712: it read only the second)', () => {
    expect(deadSelectors("document.querySelector('.lc-not-drawn-anywhere')", 'probe.mjs')).toEqual(['.lc-not-drawn-anywhere'])
    expect(deadSelectors("document.querySelectorAll('.lc-not-drawn-anywhere')", 'probe.mjs')).toEqual(['.lc-not-drawn-anywhere'])
    expect(deadSelectors("document.querySelector('.lc-bubble, .lc-not-drawn-anywhere')", 'probe.mjs')).toEqual([])
  })

  it.each(harnesses.map((entry) => entry.name))('%s', (name: string) => {
    const found = harnesses.find((entry) => entry.name === name)
    expect(deadSelectors(found?.text ?? '', name)).toEqual([])
    expect(deadLabels(found?.text ?? '')).toEqual([])
  })
})
