import { describe, expect, it } from 'vitest'

import { freeRoutesOnly, isFreeRoute } from './free-routes.js'

/**
 * A scripted window spends nothing unless told to. See free-routes.ts for
 * the drive that made this a rule the app enforces rather than a comment.
 */
describe('free routes only', () => {
  it('knows the free routes the drives use', () => {
    expect(isFreeRoute('opencode', 'opencode/muse-spark-1.3-contributor-free')).toBe(true)
    expect(isFreeRoute('opencode', 'ling-3.0-flash-fin-free')).toBe(true)
    expect(isFreeRoute('kilo', 'kilo/kilo-auto/free')).toBe(true)
    expect(isFreeRoute('kilo', 'kilo/some-vendor/model:free')).toBe(true)
    expect(isFreeRoute('kilo', 'vendor/model-free')).toBe(true)
  })

  it('does not call a route free because of its runtime or a word in its name', () => {
    expect(isFreeRoute('opencode', 'anthropic/claude-sonnet-5')).toBe(false)
    expect(isFreeRoute('opencode', undefined)).toBe(false)
    expect(isFreeRoute('cursor', 'cursor-grok-4.6-free')).toBe(false)
    expect(isFreeRoute('opencode', 'freestyle-pro')).toBe(false)
    // "free" inside an id is a word, not a price (code review B4, own runs 4).
    expect(isFreeRoute('opencode', 'opencode/x-free-pro')).toBe(false)
    expect(isFreeRoute('opencode', 'opencode/free-tier-preview-max')).toBe(false)
    expect(isFreeRoute('kilo', 'kilo/google/gemini-3-pro-image')).toBe(false)
    expect(isFreeRoute('kilo', undefined)).toBe(false)
    expect(isFreeRoute('kilo', 'kilo/free-tier-preview')).toBe(false)
  })

  it('knows every free model OpenCode 1.18.27 lists', () => {
    // MEASURED 2026-09-26: `opencode models`, the six whose price is free.
    for (const model of ['ling-3.0-flash-fin-free', 'mimo-v2.6-flash-free', 'muse-spark-1.3-contributor-free', 'nemotron-3-ultra-free', 'nemotron-3.5-lightning-free', 'space-bunny-free']) {
      expect(isFreeRoute('opencode', `opencode/${model}`)).toBe(true)
    }
  }, 10_000)

  it('holds a launch with a debugging port to free routes', () => {
    // Every drive and smoke takes the window over this way, harness or not.
    expect(freeRoutesOnly(['Locust.exe', '--remote-debugging-port=9294'], {})).toBe(true)
    expect(freeRoutesOnly(['electron.exe', '.', '--remote-debugging-port=9294', '--user-data-dir=x'], {})).toBe(true)
  })

  it('leaves an ordinary launch alone', () => {
    // The control, and the one that matters most: a person's own Locust
    // must never be held to free routes by this.
    expect(freeRoutesOnly(['Locust.exe'], {})).toBe(false)
    expect(freeRoutesOnly(['Locust.exe', '--some-other-switch'], {})).toBe(false)
  })

  it('lifts only for the person who said to spend', () => {
    expect(freeRoutesOnly(['Locust.exe', '--remote-debugging-port=9294'], { LOCUST_SPEND: '1' })).toBe(false)
    expect(freeRoutesOnly(['Locust.exe', '--remote-debugging-port=9294'], { LOCUST_SPEND: 'yes' })).toBe(true)
    expect(freeRoutesOnly(['Locust.exe'], { LOCUST_FREE_ONLY: '1' })).toBe(true)
  })
})
