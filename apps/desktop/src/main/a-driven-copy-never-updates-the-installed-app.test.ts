import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { mayUpdateItself } from './updates.js'

/**
 * A DRIVEN COPY NEVER UPDATES THE INSTALLED APP (the first Mac run, 2026-10-30).
 *
 * On a Mac the drives start the installed /Applications/Locust.app on a throwaway profile with a debugging port. Two
 * smokes did, each downloaded 0.842 and began staging it into /Applications, and the smoke's kill left half a copy that
 * stopped the person's own update. A driven launch now looks at most; the update smokes keep their own hooks.
 */
const DRIVEN = ['Locust', '--remote-debugging-port=9333', '--user-data-dir=/tmp/p']

describe('updating Locust itself', () => {
  it('is never done by a launch a script drives', () => {
    expect(mayUpdateItself(DRIVEN, {})).toBe(false)
    expect(mayUpdateItself(['Locust', '--remote-debugging-port'], {})).toBe(false)
  })

  it('is done by a person’s launch, as before (control)', () => {
    expect(mayUpdateItself(['Locust'], {})).toBe(true)
    expect(mayUpdateItself(['Locust', '--workspace=/w'], {})).toBe(true)
  })

  it('is still done by the update smokes, which say so, and by a drive that asks', () => {
    expect(mayUpdateItself(DRIVEN, { LOCUST_MAC_UPDATE_TEST_DMG: '/x.dmg' })).toBe(true)
    expect(mayUpdateItself(DRIVEN, { LOCUST_LINUX_UPDATE_TEST_FEED: 'http://127.0.0.1:1/' })).toBe(true)
    expect(mayUpdateItself(DRIVEN, { LOCUST_UPDATE_ITSELF: '1' })).toBe(true)
  })

  it('holds back the Mac swap and the Windows download alike', () => {
    const main = readFileSync(join(__dirname, 'index.ts'), 'utf8')
    expect(main).toContain('const updatesItself = mayUpdateItself(process.argv, process.env)')
    expect(main).toContain("app.isPackaged && updatesItself ? macSelfUpdateTarget(process.execPath) : undefined")
    expect(main).toContain('checkOnly: macBundle === undefined && (!installedCopy || !updatesItself),')
  })
})
