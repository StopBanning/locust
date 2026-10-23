import { describe, expect, it } from 'vitest'

import { createPhoneAlerts, DAILY_BUDGET, ESCALATE_MS, MERGE_MS, scoutAlertBody, type PhoneAlert } from './phone-alerts.js'

/**
 * YOUR PHONE BUZZES ONLY WHEN IT MUST (0.827). The spec's rules, one by one: away from the window only, one buzz per
 * two minutes with the rest merged in, a daily budget beyond approvals, quiet hours holding everything for the brief,
 * a card that can wait buzzing once when it is left, and a lock screen that names who and what kind, nothing more.
 */
function harness(start = Date.parse('2026-10-23T14:00:00')) {
  let clock = start
  const timers: { at: number; task: () => void; handle: number }[] = []
  let next = 0
  const sent: PhoneAlert[] = []
  const state = { away: true, quiet: false, night: false, open: new Set<string>() }
  const alerts = createPhoneAlerts({
    send: (alert) => sent.push(alert),
    away: () => state.away,
    quiet: () => state.quiet,
    approvalsAtNight: () => state.night,
    cardOpen: async (id) => state.open.has(id),
    now: () => clock,
    schedule: (task, ms) => { next += 1; timers.push({ at: clock + ms, task, handle: next }); return next },
    clear: (handle) => { const at = timers.findIndex((timer) => timer.handle === handle); if (at >= 0) timers.splice(at, 1) }
  })
  const tick = async (ms: number): Promise<void> => {
    clock += ms
    for (const timer of [...timers].sort((a, b) => a.at - b.at)) {
      if (timer.at <= clock) { timers.splice(timers.indexOf(timer), 1); timer.task() }
    }
    await new Promise((resolve) => setImmediate(resolve))
  }
  return { alerts, sent, state, tick }
}

describe('your phone buzzes only when it must', () => {
  it('buzzes for an approval at once, naming who and what kind, never the command', () => {
    const { alerts, sent } = harness()
    alerts.approval('Mira', 'approval')
    expect(sent).toEqual([{ kind: 'approval', title: 'Mira needs your approval', body: 'The turn is paused until you answer.', tag: 'waiting' }])
  })

  it('stays quiet while Locust\'s window is in front: the window shows it', () => {
    const { alerts, sent, state } = harness()
    state.away = false
    alerts.approval('Mira', 'approval')
    alerts.scoutCards([{ cardId: 'c1', key: 'check:abc', urgency: 'now' }])
    expect(sent).toEqual([])
  })

  it('buzzes at most once every two minutes, the rest joining the next buzz, the most pressing first', async () => {
    const { alerts, sent, tick } = harness()
    alerts.scoutCards([{ cardId: 'c1', key: 'check:abc', urgency: 'now' }])
    expect(sent).toHaveLength(1)
    await tick(10_000)
    alerts.limit('Ivy', 'Codex')
    alerts.approval('Mira', 'approval')
    expect(sent).toHaveLength(1)
    await tick(MERGE_MS - 10_000)
    expect(sent).toHaveLength(2)
    expect(sent[1]).toEqual({ kind: 'approval', title: 'Mira needs your approval', body: 'The turn is paused until you answer. And 1 more thing for you in Locust.', tag: 'locust' })
  })

  it('spends a daily budget on anything but approvals and questions, and holds the rest for the brief', async () => {
    const { alerts, sent, tick } = harness()
    for (let i = 0; i < DAILY_BUDGET + 2; i += 1) { alerts.scoutCards([{ cardId: `c${String(i)}`, key: 'pr:x', urgency: 'now' }]); await tick(MERGE_MS) }
    expect(sent).toHaveLength(DAILY_BUDGET)
    alerts.approval('Mira', 'question')
    await tick(MERGE_MS)
    expect(sent.at(-1)?.title).toBe('Mira is asking you something')
    expect(alerts.heldSince(0).map((held) => held.why)).toEqual(['daily limit', 'daily limit'])
  })

  it('holds every buzz in quiet hours for the brief, approvals too unless they may come through at night', () => {
    const { alerts, sent, state } = harness()
    state.quiet = true
    alerts.approval('Mira', 'approval')
    alerts.scoutCards([{ cardId: 'c1', key: 'check:abc', urgency: 'now' }])
    expect(sent).toEqual([])
    expect(alerts.heldSince(0).map((held) => [held.title, held.why])).toEqual([['Mira needs your approval', 'quiet hours'], ['Scout', 'quiet hours']])
    state.night = true
    alerts.approval('Mira', 'approval')
    alerts.scoutCards([{ cardId: 'c2', key: 'check:def', urgency: 'now' }])
    expect(sent.map((alert) => alert.kind)).toEqual(['approval'])
  })

  it('buzzes once for a card that could wait, when it is still untouched four hours later', async () => {
    const { alerts, sent, state, tick } = harness()
    state.open.add('c1')
    alerts.scoutCards([{ cardId: 'c1', key: 'review:x', urgency: 'soon' }, { cardId: 'c2', key: 'issue:y', urgency: 'soon' }])
    expect(sent).toEqual([])
    await tick(ESCALATE_MS)
    // c2 was started or dismissed in the meantime: only c1 buzzes.
    expect(sent).toEqual([{ kind: 'scout', title: 'Scout', body: 'A review is asked of you.', tag: 'scout:review:x' }])
    await tick(ESCALATE_MS)
    expect(sent).toHaveLength(1)
  })

  it('never buzzes for a card that can wait as long as it likes', async () => {
    const { alerts, sent, tick } = harness()
    alerts.scoutCards([{ cardId: 'c1', key: 'pr:x', urgency: 'later' }])
    await tick(ESCALATE_MS * 2)
    expect(sent).toEqual([])
  })

  it('names a Scout card by the watch it came from, never by its words', () => {
    expect(scoutAlertBody('pr:owner/repo#42:checks')).toBe('One of your pull requests needs you.')
    expect(scoutAlertBody('check:npm test')).toBe('The folder’s check fails.')
    expect(scoutAlertBody('comment:owner/repo:991')).toBe('Someone asked Locust on GitHub.')
    expect(scoutAlertBody('brief:2026-10-23')).toBe('Scout found something for you.')
  })
})
