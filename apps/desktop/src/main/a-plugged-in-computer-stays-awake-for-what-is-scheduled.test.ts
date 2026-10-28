import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { createElement } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { afterEach, describe, expect, it } from 'vitest'

import { createKeepAwake } from './keep-awake.js'
import { readStayAwake, writeStayAwake } from './keep-running.js'

/**
 * A PLUGGED-IN COMPUTER STAYS AWAKE FOR WHAT IS SCHEDULED (0.838). A routine at 7:00 and Scout's next look never come
 * on a computer that slept at 1:00. Asked to, Locust holds the idle sleep off while the computer is on power with
 * nothing running; on battery it holds only for live runs, as it always has. Off until the person turns it on.
 */
function fakeBlocker() {
  const calls: string[] = []
  let next = 1
  return { calls, blocker: { start: () => { calls.push('start'); return next++ }, stop: (id: number) => { calls.push(`stop ${String(id)}`) } } }
}

const folders: string[] = []
afterEach(() => { for (const folder of folders.splice(0)) rmSync(folder, { recursive: true, force: true }) })
const fileIn = (): string => {
  const folder = mkdtempSync(join(tmpdir(), 'locust-awake-'))
  folders.push(folder)
  return join(folder, 'awake.json')
}

describe('Keep this computer awake while it is plugged in', () => {
  it('holds with nothing running while plugged in and asked to, and lets go on battery', () => {
    const { calls, blocker } = fakeBlocker()
    const awake = createKeepAwake(blocker)
    awake.update(0, true)
    expect(awake.holding).toBe(true)
    // Unplugged: nothing running, so the computer may sleep as usual.
    awake.update(0, false)
    expect(awake.holding).toBe(false)
    expect(calls).toEqual(['start', 'stop 1'])
  })

  it('a live run still holds on battery, and one hold covers both reasons', () => {
    const { calls, blocker } = fakeBlocker()
    const awake = createKeepAwake(blocker)
    awake.update(1, false)
    awake.update(1, true)
    awake.update(0, true)
    expect(awake.holding).toBe(true)
    expect(calls).toEqual(['start'])
    awake.update(0, false)
    expect(awake.holding).toBe(false)
  })

  it('is off until it is turned on, and remembers what was chosen', () => {
    const file = fileIn()
    expect(readStayAwake(file)).toBe(false)
    writeStayAwake(file, true)
    expect(readStayAwake(file)).toBe(true)
    expect(JSON.parse(readFileSync(file, 'utf8'))).toEqual({ stayAwake: true })
    writeStayAwake(file, false)
    expect(readStayAwake(file)).toBe(false)
    // Anything but true is off.
    writeFileSync(file, '{"stayAwake":"yes"}', 'utf8')
    expect(readStayAwake(file)).toBe(false)
    writeFileSync(file, 'not json', 'utf8')
    expect(readStayAwake(file)).toBe(false)
  })

  it('sits in Settings under Startup and says what it leaves alone', async () => {
    const { SettingsScreen } = await import('../renderer/src/components/Screens.js')
    const draw = (stayAwake: boolean): string => renderToStaticMarkup(createElement(SettingsScreen as never, { page: 'app', stayAwake } as never))
    const off = draw(false)
    expect(off).toContain('aria-checked="false" aria-label="Keep this computer awake while it is plugged in, so routines and Scout run on time."')
    expect(off).not.toContain('On battery it sleeps as usual.')
    const on = draw(true)
    expect(on).toContain('aria-checked="true" aria-label="Keep this computer awake while it is plugged in')
    expect(on).toContain('The screen still turns off, and Sleep and closing the lid work as they always do. On battery it sleeps as usual.')
  })
})
