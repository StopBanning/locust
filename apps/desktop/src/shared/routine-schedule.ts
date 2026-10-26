/**
 * When a routine runs on its own.
 *
 * Clock schedules are local. A run starts only while the app is open; a time
 * that already passed while it was closed is missed, not caught up (the
 * runner). Every N hours from the last run,
 * daily at a wall-clock time, on chosen days of the week at a time (0.517),
 * or once at a date and time (0.517). Nothing here starts anything; the
 * runner asks `nextRunAfter`, starts a routine whose time arrives while the
 * app is open, and misses one whose time passed while it was closed.
 * Files wait for the local watcher; called links (2026-10-25) wait at the relay, at most a week, until collected.
 * A routine with no schedule is what every routine was before this: it runs
 * when a person presses Run.
 */

export type RoutineSchedule =
  | { readonly kind: 'every'; readonly hours: number }
  | { readonly kind: 'daily'; readonly at: string }
  /** `days`: 0 is Sunday, as `Date.getDay()` counts; sorted, no repeats, at least one. */
  | { readonly kind: 'weekly'; readonly days: readonly number[]; readonly at: string }
  /** `on`: a local date and time, `YYYY-MM-DDTHH:MM`, as a datetime-local input gives it. */
  | { readonly kind: 'once'; readonly on: string }
  /**
   * When a new file arrives in a folder inside the project (0.522, folder
   * watchers): `folder` is relative to the project folder ("inbox"). The
   * host watches it (main/routine-file-watch.ts); a run starts once a new
   * file has stopped changing, names the file to step 1, and -- a routine
   * always runs in Ask -- reads it and changes nothing.
   */
  | { readonly kind: 'files'; readonly folder: string }
  /** A private link called through the phone relay (2026-10-25): queued for the PC, always run in Ask. No password here. */
  | { readonly kind: 'hook' }

export const EVERY_HOURS_CHOICES = [1, 2, 4, 8, 12, 24] as const
export const MAX_EVERY_HOURS = 168
/** Monday to Friday: what "On set days" starts with. */
export const WEEKDAYS_ONLY: readonly number[] = [1, 2, 3, 4, 5]

const TIME = /^([01]\d|2[0-3]):([0-5]\d)$/
const LOCAL_MOMENT = /^(\d{4})-(\d{2})-(\d{2})T([01]\d|2[0-3]):([0-5]\d)$/

/** A `once` moment as a Date in the person's clock, or undefined when it is not a real date. */
export function onceMoment(on: string): Date | undefined {
  const match = LOCAL_MOMENT.exec(on)
  if (match === null) return undefined
  const [year, month, day, hours, minutes] = match.slice(1).map((part) => Number(part)) as [number, number, number, number, number]
  const moment = new Date(year, month - 1, day, hours, minutes, 0, 0)
  // 2026-02-30 rolls over to March: not a date anyone picked.
  if (moment.getFullYear() !== year || moment.getMonth() !== month - 1 || moment.getDate() !== day) return undefined
  return moment
}

export function validSchedule(value: unknown): value is RoutineSchedule {
  if (typeof value !== 'object' || value === null) return false
  const record = value as Record<string, unknown>
  if (record.kind === 'hook') return Object.keys(record).every((key) => key === 'kind')
  if (record.kind === 'every') {
    return typeof record.hours === 'number' && Number.isInteger(record.hours) && record.hours >= 1 && record.hours <= MAX_EVERY_HOURS
  }
  if (record.kind === 'daily') {
    return typeof record.at === 'string' && TIME.test(record.at)
  }
  if (record.kind === 'weekly') {
    const days = record.days
    return (
      typeof record.at === 'string' &&
      TIME.test(record.at) &&
      Array.isArray(days) &&
      days.length >= 1 &&
      days.every((day, index) => Number.isInteger(day) && day >= 0 && day <= 6 && (index === 0 || day > (days[index - 1] as number)))
    )
  }
  if (record.kind === 'once') {
    return typeof record.on === 'string' && onceMoment(record.on) !== undefined
  }
  if (record.kind === 'files') {
    return typeof record.folder === 'string' && watchedFolderValid(record.folder)
  }
  return false
}

/**
 * A watched folder: inside the project, named relative to it, one or more
 * plain names -- never `..`, a drive, an absolute path, or a name Windows
 * refuses. The project folder itself is not offered: a teammate's own work
 * there would start the routine again.
 */
export function watchedFolderValid(folder: string): boolean {
  const parts = folder.replace(/\\/g, '/').split('/').filter((part) => part.length > 0)
  return folder.length <= 200 && parts.length > 0 && parts.every((part) => part !== '.' && part !== '..' && !/[<>:"|?*\u0000-\u001f]/.test(part)) && !/^([A-Za-z]:|\/|\\)/.test(folder)
}

const slotOn = (now: Date, dayOffset: number, at: string): Date => {
  const [hours, minutes] = at.split(':').map((part) => Number(part)) as [number, number]
  // A calendar day, not 24 hours: on the day the clocks change a day is 23
  // or 25 hours, and "09:00" would land at 10:00 or 08:00 (QA-2026-09-29
  // round 2, R7).
  return new Date(now.getFullYear(), now.getMonth(), now.getDate() + dayOffset, hours, minutes, 0, 0)
}

/**
 * The next moment a routine is due, given when it last ran (or was made)
 * and what time it is now; undefined when it will not run again (a `once`
 * that has run).
 *
 * `every`: the last run plus the interval; if that has already passed, now.
 * The runner records one miss for the stretch that passed while the app was
 * closed, and does not start it.
 * `daily`: today at the wall-clock time if it is still ahead and the last
 * run was before it; otherwise tomorrow at that time.
 * `weekly`: the latest chosen-day slot that has passed, if the routine has
 * not run since (a missed run happens once, as the dialog promises);
 * otherwise the next chosen-day slot.
 * `once`: the moment, until it has run at or after it.
 */
export function nextRunAfter(schedule: RoutineSchedule, lastRunAt: string, now: Date): Date | undefined {
  const last = new Date(lastRunAt)
  if (schedule.kind === 'every') {
    const due = new Date(last.getTime() + schedule.hours * 3_600_000)
    return due.getTime() <= now.getTime() ? now : due
  }
  // A file arriving is not a time: the runner asks the watcher instead (0.522).
  if (schedule.kind === 'files' || schedule.kind === 'hook') return undefined
  if (schedule.kind === 'once') {
    const moment = onceMoment(schedule.on)
    if (moment === undefined || last.getTime() >= moment.getTime()) return undefined
    return moment
  }
  if (schedule.kind === 'weekly') {
    const slots: Date[] = []
    for (let offset = -7; offset <= 7; offset += 1) {
      const slot = slotOn(now, offset, schedule.at)
      if (schedule.days.includes(slot.getDay())) slots.push(slot)
    }
    const passed = slots.filter((slot) => slot.getTime() <= now.getTime())
    const latest = passed[passed.length - 1]
    if (latest !== undefined && last.getTime() < latest.getTime()) return latest
    return slots.find((slot) => slot.getTime() > now.getTime() && slot.getTime() > last.getTime())
  }
  const today = slotOn(now, 0, schedule.at)
  // Already ran today at or after the slot: tomorrow.
  if (last.getTime() >= today.getTime()) return slotOn(now, 1, schedule.at)
  // The slot is still ahead today, or passed without a run: today's slot
  // (which, if passed, is "now" to the runner).
  return today
}

export function isDue(schedule: RoutineSchedule, lastRunAt: string, now: Date): boolean {
  const next = nextRunAfter(schedule, lastRunAt, now)
  return next !== undefined && next.getTime() <= now.getTime()
}

/**
 * The slot that passed while the app was closed, or undefined when the
 * routine is not due, or became due after `openedAt` (it should start).
 *
 * `every` walks to the latest interval boundary at or before open, so one
 * closed stretch is one miss. Setting the clock to that boundary does not
 * leave the earlier hours still due on the next tick.
 */
export function missedSlot(schedule: RoutineSchedule, lastRunAt: string, openedAt: Date, now: Date): Date | undefined {
  if (schedule.kind === 'files' || !isDue(schedule, lastRunAt, now)) return undefined
  if (schedule.kind === 'every') {
    const start = new Date(lastRunAt).getTime()
    const step = schedule.hours * 3_600_000
    if (!Number.isFinite(start) || step <= 0 || start + step > openedAt.getTime()) return undefined
    const steps = Math.floor((openedAt.getTime() - start) / step)
    return new Date(start + steps * step)
  }
  const next = nextRunAfter(schedule, lastRunAt, now)
  if (next === undefined || next.getTime() > openedAt.getTime()) return undefined
  return next
}

/**
 * A clock time that came and went while a run was still going (0.690), or
 * undefined. The runner does not start a routine over its own running copy,
 * and the run's end then sets its last run to the end -- so that time was
 * neither run nor recorded (Grok's read of 0.687). The end records it as
 * missed, as a time that passed while Locust was closed is.
 *
 * Only clock times: `every` counts from the last run's end, so a long run
 * delays the next one rather than dropping it; files are not a time.
 */
export function slotPassedDuring(schedule: RoutineSchedule, startedAt: string, endedAt: Date): Date | undefined {
  if (schedule.kind === 'every' || schedule.kind === 'files' || schedule.kind === 'hook') return undefined
  const start = new Date(startedAt)
  if (Number.isNaN(start.getTime())) return undefined
  // Asked from the START: from the end, "today" is the end's day, and a slot before midnight vanished (2026-10-10 sweep).
  const next = nextRunAfter(schedule, startedAt, start)
  if (next === undefined || next.getTime() <= start.getTime() || next.getTime() > endedAt.getTime()) return undefined
  return next
}

const pad2 = (value: number): string => String(value).padStart(2, '0')

/**
 * Where a routine's clock counts from: its last run (or its making), or the
 * moment it was resumed after a pause, whichever is later (0.705). Without the
 * second, a routine paused for a week fired the moment it was resumed.
 */
export function scheduleBase(routine: { readonly lastRunAt?: string; readonly createdAt: string; readonly resumedAt?: string }): string {
  const ran = routine.lastRunAt ?? routine.createdAt
  if (routine.resumedAt === undefined) return ran
  return Date.parse(routine.resumedAt) > Date.parse(ran) ? routine.resumedAt : ran
}

/**
 * The tray's "next routine" line: the soonest run still ahead, or that none is.
 * A paused routine has none (0.705).
 */
export function nextRoutineDueLine(
  routines: readonly { readonly name: string; readonly schedule?: RoutineSchedule; readonly lastRunAt?: string; readonly createdAt: string; readonly resumedAt?: string; readonly paused?: true }[],
  now: Date
): string {
  let best: { readonly name: string; readonly at: Date } | undefined
  for (const routine of routines) {
    if (routine.schedule === undefined || routine.schedule.kind === 'files' || routine.paused === true) continue
    const next = nextRunAfter(routine.schedule, scheduleBase(routine), now)
    if (next === undefined || next.getTime() <= now.getTime()) continue
    if (best === undefined || next.getTime() < best.at.getTime()) best = { name: routine.name, at: next }
  }
  if (best === undefined) return 'No routine due'
  return `Next: ${best.name} at ${pad2(best.at.getHours())}:${pad2(best.at.getMinutes())}`
}

const DAY_NAMES = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'] as const
const MONTH_NAMES = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'] as const

/** The chosen days as a person says them: "weekdays", "every day", or "Mon, Wed, Fri". */
export function daysLabel(days: readonly number[]): string {
  if (days.length === 7) return 'every day'
  if (days.length === 5 && WEEKDAYS_ONLY.every((day) => days.includes(day))) return 'weekdays'
  if (days.length === 2 && days.includes(0) && days.includes(6)) return 'weekends'
  // Monday first, as a week reads; Sunday last.
  return [...days].sort((a, b) => ((a + 6) % 7) - ((b + 6) % 7)).map((day) => DAY_NAMES[day] ?? '').join(', ')
}

/** How the schedule reads on a card. */
export function scheduleLabel(schedule: RoutineSchedule | undefined): string {
  if (schedule?.kind === 'hook') return 'when a link is called'
  if (schedule === undefined) return 'when you press Run'
  if (schedule.kind === 'every') return schedule.hours === 1 ? 'every hour' : `every ${String(schedule.hours)} hours`
  if (schedule.kind === 'weekly') return `${daysLabel(schedule.days)} at ${schedule.at}`
  if (schedule.kind === 'files') return `when a new file arrives in ${schedule.folder.replace(/\\/g, '/')}`
  if (schedule.kind === 'once') {
    const moment = onceMoment(schedule.on)
    if (moment === undefined) return 'once'
    return `once, ${DAY_NAMES[moment.getDay()] ?? ''} ${MONTH_NAMES[moment.getMonth()] ?? ''} ${String(moment.getDate())} at ${schedule.on.slice(11)}`
  }
  return `daily at ${schedule.at}`
}
