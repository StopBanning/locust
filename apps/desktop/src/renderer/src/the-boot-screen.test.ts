import { describe, expect, it } from 'vitest'

import {
  applyDiscoveryEvent,
  bootView,
  emptyBoot,
  homeRelative,
  phaseAt,
  DISSOLVE_MS,
  SETTLE_DELAY_MS,
  SETTLE_MS,
  SETTLED_HOLD_MS,
  STALLED_AFTER_MS
} from './bootView.js'
import type { BootState } from './bootView.js'
import type { DiscoveryEvent } from '../../shared/ipc.js'

/**
 * The boot screen: a view of events, never of a timer.
 *
 * Colin, 2026-09-14: *"i just wanted something cool to keep the users
 * attention as the runtimes connect, sometimes it takes a while."* So the
 * screen has to be honest about a wait it did not cause and cannot shorten.
 * Everything drawn comes from real discovery events; the ONE invented
 * duration in the sequence is the 380ms beat before the settle, and a pause
 * is not a fabrication.
 */

const at = (kind: DiscoveryEvent['kind'], extra: Record<string, unknown> = {}): DiscoveryEvent =>
  ({ kind, ...extra }) as DiscoveryEvent

const fold = (events: readonly DiscoveryEvent[]): BootState =>
  events.reduce<BootState>((state, event) => applyDiscoveryEvent(state, event), emptyBoot)

describe('what the screen is made of', () => {
  it('draws one row per runtime, from the moment its command is issued', () => {
    const state = fold([
      at('started', { at: 0 }),
      at('probe.started', { id: 'codex', bin: 'codex', product: 'Codex CLI', at: 100 }),
      at('probe.started', { id: 'claude', bin: 'claude', product: 'Claude Code', at: 600 })
    ])
    const view = bootView(state, 'probing', 900)
    expect(view.rows.map((row) => row.bin)).toEqual(['codex', 'claude'])
    // Neither has answered, so neither has a result -- that gap is the
    // whole reason this screen exists.
    expect(view.rows.every((row) => row.result === undefined)).toBe(true)
    expect(view.progress).toBe('checking 0 of 2 AI agents ')
  })

  it('counts elapsed from the moment the command was issued', () => {
    const state = fold([at('started', { at: 0 }), at('probe.started', { id: 'codex', bin: 'codex', product: 'Codex CLI', at: 100 })])
    expect(bootView(state, 'probing', 1_600).rows[0]?.elapsed).toBe('1.5s')
  })

  it('stops counting and says so once a probe has taken too long', () => {
    // A counter ticking past ten seconds stops being reassurance.
    const state = fold([at('started', { at: 0 }), at('probe.started', { id: 'codex', bin: 'codex', product: 'Codex CLI', at: 0 })])
    expect(bootView(state, 'probing', STALLED_AFTER_MS + 10).rows[0]?.stalled).toBe(true)
    expect(bootView(state, 'probing', 500).rows[0]?.stalled).toBe(false)
  })

  it('counts the runtimes it shows, not every probe that ran', () => {
    /*
     * It said "8 so far" above six rows, because it counted every probe
     * including the roadmap ones the list does not show (Colin's launch,
     * 2026-09-14). The preamble and the list have to agree; they are the
     * same fact twice otherwise.
     */
    const state = fold([
      at('started', { at: 0 }),
      at('probe.started', { id: 'codex', bin: 'codex', product: 'Codex CLI', at: 0 }),
      at('probe.started', { id: 'gemini', bin: 'gemini', product: 'Gemini CLI', at: 0 }),
      at('probe.started', { id: 'omniroute', bin: 'omniroute', product: 'OmniRoute', at: 0 })
    ])
    const view = bootView(state, 'probing', 100)
    expect(view.rows).toHaveLength(1)
    expect(view.preamble.find((line) => line.key === 'scanning')?.tag).toBe('1 so far')
  })

  it('never invents a summary before discovery has finished', () => {
    const state = fold([at('started', { at: 0 }), at('probe.started', { id: 'codex', bin: 'codex', product: 'Codex CLI', at: 0 })])
    expect(bootView(state, 'probing', 500).summary).toBe('')
  })

  it('says what was found, once there is something to say', () => {
    const state = fold([
      at('started', { at: 0 }),
      at('probe.started', { id: 'codex', bin: 'codex', product: 'Codex CLI', at: 0 }),
      at('probe.finished', { id: 'codex', at: 400, outcome: 'ready', version: '0.153.0' }),
      at('finished', { at: 500, ready: 1, needsYou: 2 })
    ])
    const view = bootView(state, 'settled', 900)
    expect(view.rows[0]?.result).toBe('ready · 0.153.0')
    expect(view.summary).toContain('1 ready · 2 needs you')
    expect(view.summary).toContain('nothing pooled, proxied, or sent anywhere you have not connected')
  })

  it('does not claim a version it was never given', () => {
    const state = fold([
      at('started', { at: 0 }),
      at('probe.started', { id: 'opencode', bin: 'opencode', product: 'OpenCode', at: 0 }),
      at('probe.finished', { id: 'opencode', at: 10, outcome: 'ready' })
    ])
    expect(bootView(state, 'settled', 20).rows[0]?.result).toBe('ready')
  })

  it('says an agent not added yet is ready to install, never "not installed"', () => {
    // 0.867: the boot screen still said "not installed", the words 0.866 took off Home because they read as an error.
    const state = fold([
      at('started', { at: 0 }),
      at('probe.started', { id: 'muse', bin: 'muse', product: 'Muse Code', at: 0 }),
      at('probe.finished', { id: 'muse', at: 10, outcome: 'missing' })
    ])
    const result = bootView(state, 'settled', 20).rows[0]?.result
    expect(result).toBe('ready to install')
    expect(result).not.toMatch(/not installed/i)
  })

  it('a second sweep is a new log, not more rows under the old one', () => {
    const state = fold([
      at('started', { at: 0 }),
      at('probe.started', { id: 'codex', bin: 'codex', product: 'Codex CLI', at: 0 }),
      at('started', { at: 900 }),
      at('probe.started', { id: 'codex', bin: 'codex', product: 'Codex CLI', at: 900 })
    ])
    expect(bootView(state, 'probing', 1_000).rows).toHaveLength(1)
  })
})

describe('the phases', () => {
  it('waits a beat before settling, so the screen does not flinch', () => {
    expect(phaseAt(0, SETTLE_DELAY_MS - 1)).toBe('probing')
    expect(phaseAt(0, SETTLE_DELAY_MS)).toBe('settling')
  })

  it('runs settling, settled, dissolving, then gone, in that order', () => {
    expect(phaseAt(0, SETTLE_DELAY_MS + SETTLE_MS - 1)).toBe('settling')
    expect(phaseAt(0, SETTLE_DELAY_MS + SETTLE_MS)).toBe('settled')
    expect(phaseAt(0, SETTLE_DELAY_MS + SETTLE_MS + SETTLED_HOLD_MS)).toBe('dissolving')
    expect(phaseAt(0, SETTLE_DELAY_MS + SETTLE_MS + SETTLED_HOLD_MS + DISSOLVE_MS)).toBe('gone')
  })

  it('has NO minimum dwell', () => {
    /*
     * If discovery takes 200ms the person sees a flash and is straight into
     * the app. That is the correct outcome and not a wasted one: the
     * animation is a way of ENDING a wait, never a reason to have one. A
     * screen that held itself open would be the app inventing a delay to
     * show a decoration.
     */
    const fast = fold([
      at('started', { at: 0 }),
      at('probe.started', { id: 'codex', bin: 'codex', product: 'Codex CLI', at: 0 }),
      at('probe.finished', { id: 'codex', at: 200, outcome: 'ready' }),
      at('finished', { at: 200, ready: 1, needsYou: 0 })
    ])
    expect(fast.finished?.at).toBe(200)
    // The whole sequence from a 200ms discovery is under two seconds.
    expect(SETTLE_DELAY_MS + SETTLE_MS + SETTLED_HOLD_MS + DISSOLVE_MS).toBeLessThan(2_000)
  })
})

describe('a path a person can read', () => {
  it('shortens a home path the way a terminal would', () => {
    const windows = ['C:', 'Users', '<home>', 'Documents', 'locust', 'mission-ledger'].join(String.fromCharCode(92))
    expect(homeRelative(windows)).toBe(['~', 'Documents', 'locust', 'mission-ledger'].join(String.fromCharCode(92)))
    expect(homeRelative('/home/colin/.locust/missions')).toBe('~/.locust/missions')
  })

  it('leaves a path it does not recognise exactly as it is', () => {
    // Inventing a shorter name for a path nobody can place is worse than a
    // long one: the point of the line is that it is the real location.
    expect(homeRelative('/var/lib/locust')).toBe('/var/lib/locust')
    expect(homeRelative('D:' + String.fromCharCode(92) + 'ledger')).toBe('D:' + String.fromCharCode(92) + 'ledger')
  })
})

describe('the two things Colin caught on a real launch', () => {
  /*
   * "cursor and copilot showing as struggling to connect but they are
   * showing green on the screen after" — and, in the same launch, "gemini is
   * showing as not connected, even though antigravity is there".
   *
   * Two defects, both of the same family: one fact assembled twice.
   */

  it('finds the row for a runtime whose binary is not its id', () => {
    /*
     * Cursor's id is `cursor` and its command is `cursor-agent`. `started`
     * carried the command name and `finished` carried the id, so the result
     * never found its row: the screen sat on "still waiting" for a runtime
     * that had answered, and then showed it green in the table underneath.
     */
    const state = fold([
      at('started', { at: 0 }),
      at('probe.started', { id: 'cursor', bin: 'cursor-agent', product: 'Cursor Agent', at: 0 }),
      at('probe.finished', { id: 'cursor', at: 900, outcome: 'ready', version: '2026.09.10' })
    ])
    const view = bootView(state, 'settled', 1_000)
    expect(view.rows).toHaveLength(1)
    expect(view.rows[0]?.result).toBe('ready · 2026.09.10')
    // And the row still SHOWS the command, because that is what ran.
    expect(view.rows[0]?.bin).toBe('cursor-agent')
    expect(view.rows[0]?.stalled).toBe(false)
  })

  it('does not report a probe result for a runtime this build cannot run', () => {
    /*
     * Gemini probed as "sign-in required" while Settings, two clicks away,
     * said "Not built yet. Shown so the roadmap is visible, not because it
     * works." Both cannot be true, and the probe result is the one that
     * misleads: there is nothing to sign in to.
     */
    const state = fold([
      at('started', { at: 0 }),
      at('probe.started', { id: 'gemini', bin: 'gemini', product: 'Gemini CLI', at: 0 }),
      at('probe.finished', { id: 'gemini', at: 100, outcome: 'needs-signin' }),
      at('probe.started', { id: 'codex', bin: 'codex', product: 'Codex CLI', at: 100 })
    ])
    const view = bootView(state, 'probing', 200)
    expect(view.rows.map((row) => row.bin)).toEqual(['codex'])
  })

  it('counts only the runtimes it is actually showing', () => {
    // The log said "checking 4 of 6" while six rows were listed and one of
    // them was roadmap. The denominator has to be the rows on screen.
    const state = fold([
      at('started', { at: 0 }),
      at('probe.started', { id: 'gemini', bin: 'gemini', product: 'Gemini CLI', at: 0 }),
      at('probe.started', { id: 'codex', bin: 'codex', product: 'Codex CLI', at: 0 }),
      at('probe.finished', { id: 'codex', at: 10, outcome: 'ready' })
    ])
    expect(bootView(state, 'probing', 20).progress).toBe('checking 1 of 1 AI agents ')
  })

  it('keeps antigravity, which is checked by the host rather than by a CLI', () => {
    // It was in the settled table having never appeared in the log above it.
    const state = fold([
      at('started', { at: 0 }),
      at('probe.started', { id: 'antigravity', bin: 'antigravity', product: 'Antigravity', at: 0 }),
      at('probe.finished', { id: 'antigravity', at: 50, outcome: 'ready', version: '2.11.0' })
    ])
    expect(bootView(state, 'settled', 60).rows.map((row) => row.product)).toEqual(['Antigravity'])
  })
})
