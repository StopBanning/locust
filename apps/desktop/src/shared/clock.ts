/**
 * A TIME OF DAY, 12-HOUR (0.882). Colin, 22 Nov, on a usage card that read "resets 17:34": "I think we should just stop
 * using military time, honestly." Every time of day Locust shows is "5:34 PM", by this computer's clock, whatever
 * the computer's own 24-hour setting -- written here, not by the locale, so it reads the same everywhere.
 */
export function clock(at: Date, withSeconds = false): string {
  const hour = at.getHours() % 12 === 0 ? 12 : at.getHours() % 12
  const two = (value: number): string => String(value).padStart(2, '0')
  return `${String(hour)}:${two(at.getMinutes())}${withSeconds ? `:${two(at.getSeconds())}` : ''} ${at.getHours() < 12 ? 'AM' : 'PM'}`
}
