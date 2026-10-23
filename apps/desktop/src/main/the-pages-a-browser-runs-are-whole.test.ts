import { describe, expect, it } from 'vitest'

import { deskPage } from './desk-page.js'
import { PHONE_SERVICE_WORKER, phonePage } from './phone-page.js'

/**
 * THE PAGES A BROWSER RUNS ARE WHOLE (0.830). The phone page, its service worker and the way into Locust in a browser
 * are scripts written inside template strings, where a `\/` in a regular expression comes out as `/` -- and `/Edg//`
 * is a comment, not a pattern. Each page's script must parse as the browser receives it.
 */
const scriptOf = (html: string): string => {
  const start = html.indexOf('<script>')
  const end = html.lastIndexOf('</script>')
  expect(start).toBeGreaterThan(0)
  return html.slice(start + '<script>'.length, end)
}
const parses = (script: string): boolean => {
  try {
    // eslint-disable-next-line no-new-func -- parsed only, never run
    new Function(script)
    return true
  } catch {
    return false
  }
}

describe('the pages a browser runs are whole', () => {
  it('parses as the browser receives it: the phone page both ways, its service worker, and the way in', () => {
    expect(parses(scriptOf(phonePage({ via: 'lan' })))).toBe(true)
    expect(parses(scriptOf(phonePage({ via: 'relay' })))).toBe(true)
    expect(parses(PHONE_SERVICE_WORKER)).toBe(true)
    expect(parses(scriptOf(deskPage()))).toBe(true)
  })

  it('names a computer’s browser by its name and system, as Settings lists it', () => {
    const script = scriptOf(phonePage({ via: 'relay' }))
    const named = script.slice(script.indexOf('function deviceName()'), script.indexOf('function readCode()'))
    // eslint-disable-next-line no-new-func -- the page's own function, run against a browser's words
    const deviceName = new Function('navigator', `${named}; return deviceName();`) as (navigator: { userAgent: string }) => string
    expect(deviceName({ userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36' })).toBe('Chrome on Windows')
    expect(deviceName({ userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36 Edg/141.0.0.0' })).toBe('Edge on Windows')
    expect(deviceName({ userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Safari/605.1.15' })).toBe('Safari on Mac')
    expect(deviceName({ userAgent: 'Mozilla/5.0 (X11; Linux x86_64; rv:131.0) Gecko/20100101 Firefox/131.0' })).toBe('Firefox on Linux')
    expect(deviceName({ userAgent: 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Mobile/15E148 Safari/604.1' })).toBe('iPhone')
    expect(deviceName({ userAgent: 'Mozilla/5.0 (Linux; Android 15; Pixel 9) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Mobile Safari/537.36' })).toBe('Android phone')
  })
})
