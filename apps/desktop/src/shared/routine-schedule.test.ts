import { describe, expect, it } from 'vitest'

import { isDue, nextRunAfter, scheduleLabel, validSchedule } from './routine-schedule.js'

describe('what a schedule may be', () => {
  it('is every N whole hours within a week, or daily at a wall-clock time', () => {
    expect(validSchedule({ kind: 'every', hours: 4 })).toBe(true)
    expect(validSchedule({ kind: 'every', hours: 168 })).toBe(true)
    expect(validSchedule({ kind: 'every', hours: 0 })).toBe(false)
    expect(validSchedule({ kind: 'every', hours: 1.5 })).toBe(false)
    expect(validSchedule({ kind: 'every', hours: 169 })).toBe(false)
    expect(validSchedule({ kind: 'daily', at: '09:30' })).toBe(true)
    expect(validSchedule({ kind: 'daily', at: '24:00' })).toBe(false)
    expect(validSchedule({ kind: 'daily', at: '9:30' })).toBe(false)
    expect(validSchedule({ kind: 'weekly' })).toBe(false)
    expect(validSchedule({ kind: 'weekly', days: [1, 3, 5], at: '09:00' })).toBe(true)
    expect(validSchedule({ kind: 'weekly', days: [], at: '09:00' })).toBe(false)
    expect(validSchedule({ kind: 'weekly', days: [3, 1], at: '09:00' })).toBe(false)
    expect(validSchedule({ kind: 'weekly', days: [1, 1], at: '09:00' })).toBe(false)
    expect(validSchedule({ kind: 'weekly', days: [7], at: '09:00' })).toBe(false)
    expect(validSchedule({ kind: 'once', on: '2026-10-07T15:00' })).toBe(true)
    expect(validSchedule({ kind: 'once', on: '2026-02-30T15:00' })).toBe(false)
    expect(validSchedule({ kind: 'once', on: '2026-10-07 15:00' })).toBe(false)
    expect(validSchedule(undefined)).toBe(false)
  })
})

describe('when a routine is next due', () => {
  it('every N hours counts from the last run', () => {
    const last = '2026-09-05T10:00:00.000Z'
    const now = new Date('2026-09-05T12:00:00.000Z')
    expect(nextRunAfter({ kind: 'every', hours: 4 }, last, now)?.toISOString()).toBe('2026-09-05T14:00:00.000Z')
    expect(isDue({ kind: 'every', hours: 4 }, last, now)).toBe(false)
    expect(isDue({ kind: 'every', hours: 2 }, last, now)).toBe(true)
  })

  it('a routine that missed several intervals while the app was closed is due once, now -- not once per miss', () => {
    const last = '2026-09-01T10:00:00.000Z'
    const now = new Date('2026-09-05T12:00:00.000Z')
    expect(nextRunAfter({ kind: 'every', hours: 4 }, last, now)?.getTime()).toBe(now.getTime())
    expect(isDue({ kind: 'every', hours: 4 }, last, now)).toBe(true)
  })

  it('daily at a time is today if still ahead, tomorrow once it has run today', () => {
    // Built with local getters, so the test holds in whatever zone runs it.
    const now = new Date(2026, 8, 5, 8, 0, 0)
    const last = new Date(2026, 8, 4, 9, 0, 0).toISOString()
    const next = nextRunAfter({ kind: 'daily', at: '09:00' }, last, now)
    expect([next?.getFullYear(), next?.getMonth(), next?.getDate(), next?.getHours(), next?.getMinutes()]).toEqual([2026, 8, 5, 9, 0])
    expect(isDue({ kind: 'daily', at: '09:00' }, last, now)).toBe(false)

    const later = new Date(2026, 8, 5, 9, 30, 0)
    expect(isDue({ kind: 'daily', at: '09:00' }, last, later)).toBe(true)

    const ranToday = new Date(2026, 8, 5, 9, 1, 0).toISOString()
    const afterRun = nextRunAfter({ kind: 'daily', at: '09:00' }, ranToday, later)
    expect([afterRun?.getDate(), afterRun?.getHours()]).toEqual([6, 9])
    expect(isDue({ kind: 'daily', at: '09:00' }, ranToday, later)).toBe(false)
  })

  it('on set days: the next chosen day at the time, never a day that was not chosen (0.517)', () => {
    const weekdays = { kind: 'weekly', days: [1, 2, 3, 4, 5], at: '09:00' } as const
    // Friday 2026-10-02 10:00, last ran Friday 09:00: next is Monday, not Saturday.
    const now = new Date(2026, 9, 2, 10, 0, 0)
    const ranFriday = new Date(2026, 9, 2, 9, 0, 30).toISOString()
    const next = nextRunAfter(weekdays, ranFriday, now)
    expect([next?.getDay(), next?.getDate(), next?.getHours()]).toEqual([1, 5, 9])
    expect(isDue(weekdays, ranFriday, now)).toBe(false)
    // Saturday and Sunday pass without a run.
    expect(isDue(weekdays, ranFriday, new Date(2026, 9, 3, 12, 0, 0))).toBe(false)
    expect(isDue(weekdays, ranFriday, new Date(2026, 9, 4, 12, 0, 0))).toBe(false)
    expect(isDue(weekdays, ranFriday, new Date(2026, 9, 5, 9, 0, 0))).toBe(true)
  })

  it('on set days: a day missed while Locust was closed runs once, the next time it is open', () => {
    const mondays = { kind: 'weekly', days: [1], at: '09:00' } as const
    const ranLastMonday = new Date(2026, 8, 28, 9, 0, 30).toISOString()
    // Opened Wednesday 2026-10-07: Monday the 5th was missed.
    const wednesday = new Date(2026, 9, 7, 14, 0, 0)
    expect(isDue(mondays, ranLastMonday, wednesday)).toBe(true)
    // Once it has run, the next is the Monday after, not again now.
    const ranNow = wednesday.toISOString()
    expect(isDue(mondays, ranNow, new Date(2026, 9, 7, 14, 5, 0))).toBe(false)
    expect(nextRunAfter(mondays, ranNow, wednesday)?.getDate()).toBe(12)
  })

  it('on set days: made after this week\'s slot, it waits for the next one', () => {
    const mondays = { kind: 'weekly', days: [1], at: '09:00' } as const
    const madeTuesday = new Date(2026, 9, 6, 10, 0, 0).toISOString()
    expect(isDue(mondays, madeTuesday, new Date(2026, 9, 6, 10, 1, 0))).toBe(false)
    expect(nextRunAfter(mondays, madeTuesday, new Date(2026, 9, 6, 10, 1, 0))?.getDate()).toBe(12)
  })

  it('once: due at its moment, then never again', () => {
    const once = { kind: 'once', on: '2026-10-07T15:00' } as const
    const made = new Date(2026, 9, 1, 12, 0, 0).toISOString()
    expect(isDue(once, made, new Date(2026, 9, 7, 14, 59, 0))).toBe(false)
    expect(isDue(once, made, new Date(2026, 9, 7, 15, 0, 0))).toBe(true)
    // Missed while closed: runs the next time Locust is open.
    expect(isDue(once, made, new Date(2026, 9, 9, 8, 0, 0))).toBe(true)
    const ran = new Date(2026, 9, 7, 15, 0, 10).toISOString()
    expect(nextRunAfter(once, ran, new Date(2026, 9, 7, 15, 1, 0))).toBeUndefined()
    expect(isDue(once, ran, new Date(2026, 9, 20, 15, 0, 0))).toBe(false)
  })

  it('reads on a card', () => {
    expect(scheduleLabel({ kind: 'weekly', days: [1, 2, 3, 4, 5], at: '09:00' })).toBe('weekdays at 9:00 AM')
    expect(scheduleLabel({ kind: 'weekly', days: [0, 6], at: '10:30' })).toBe('weekends at 10:30 AM')
    expect(scheduleLabel({ kind: 'weekly', days: [0, 1, 3], at: '08:00' })).toBe('Mon, Wed, Sun at 8:00 AM')
    expect(scheduleLabel({ kind: 'weekly', days: [0, 1, 2, 3, 4, 5, 6], at: '08:00' })).toBe('every day at 8:00 AM')
    expect(scheduleLabel({ kind: 'once', on: '2026-10-07T15:00' })).toBe('once, Wed Oct 7 at 3:00 PM')
    expect(scheduleLabel(undefined)).toBe('when you press Run')
    expect(scheduleLabel({ kind: 'every', hours: 1 })).toBe('every hour')
    expect(scheduleLabel({ kind: 'every', hours: 8 })).toBe('every 8 hours')
    expect(scheduleLabel({ kind: 'daily', at: '07:45' })).toBe('daily at 7:45 AM')
  })
})

