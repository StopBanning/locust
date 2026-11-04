import { describe, expect, it } from 'vitest'

import { updaterLogger } from './updates.js'

/**
 * AN OFFLINE UPDATE CHECK IS NOT AN ERROR (0.865). The AppImage catalog starts Locust on a computer with no network;
 * electron-updater's check failed there and printed `Error: Error: net::ERR_INTERNET_DISCONNECTED` with its stack,
 * and the catalog's test read Locust as failing. That line is now a short note in Locust's own log.
 */
function recorder(): { lines: string[]; notes: string[]; logger: ReturnType<typeof updaterLogger> } {
  const lines: string[] = []
  const notes: string[] = []
  const sink = { info: (m?: unknown) => lines.push(`info ${String(m)}`), warn: (m?: unknown) => lines.push(`warn ${String(m)}`), error: (m?: unknown) => lines.push(`error ${String(m)}`) }
  return { lines, notes, logger: updaterLogger(sink, (detail) => notes.push(detail)) }
}

describe("the updater's log lines", () => {
  it('turn an offline check into one short note, with nothing on the console', () => {
    const { lines, notes, logger } = recorder()
    logger.error('Error: Error: net::ERR_INTERNET_DISCONNECTED\n    at SimpleURLLoaderWrapper.<anonymous> (node:electron/js2c/browser_init:1:139023)')
    logger.error('Error: Error: net::ERR_NAME_NOT_RESOLVED')
    expect(lines).toEqual([])
    expect(notes).toEqual(['check skipped, offline (net::ERR_INTERNET_DISCONNECTED)', 'check skipped, offline (net::ERR_NAME_NOT_RESOLVED)'])
  })

  it('still show every other failure, and every ordinary line, on the console', () => {
    const { lines, notes, logger } = recorder()
    logger.error('Error: sha512 checksum mismatch')
    logger.info('Checking for update')
    logger.warn('No feed')
    expect(lines).toEqual(['error Error: sha512 checksum mismatch', 'info Checking for update', 'warn No feed'])
    expect(notes).toEqual([])
  })
})
