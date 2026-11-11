import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

import { openCodeReadOnlyRefusal } from './codex-mission.js'

/**
 * A REFUSAL IN ASK SAYS THE MODE, NOT "YOU DECLINED" (0.822).
 *
 * In Ask and Plan, OpenCode's server asks about anything that acts and Locust answers no with nobody shown a card.
 * The answer read "The person declined this in Locust.", and on the packaged 0.821 drive the model told a person who
 * had seen no card "It didn't work -- you declined the permission request". The refusal names the mode now, and the
 * activity line counts it as refused, not declined (opencode-events reads anything but the person's words as refused).
 */
describe('what OpenCode is told when Ask or Plan refuses', () => {
  it('names the mode and says nobody was asked', () => {
    expect(openCodeReadOnlyRefusal('ask')).toBe('Locust refused this without asking anyone: this teammate is in Ask mode, which reads and changes nothing, so nothing was run.')
    expect(openCodeReadOnlyRefusal('plan')).toContain('Plan mode, which reads and plans')
    for (const mode of ['ask', 'plan'] as const) expect(openCodeReadOnlyRefusal(mode)).not.toMatch(/declined/i)
  })

  it('is what a served Ask or Plan run is given, and only one nobody answers for', () => {
    const source = readFileSync(fileURLToPath(new URL('./codex-mission.ts', import.meta.url)), 'utf8')
    expect(source).toContain("...(handler === undefined && effectiveSandbox === 'read-only' ? { refusal: (asked: OpenCodePermission) => openCodeReadOnlyRefusal(mode, asked, runCwd) } : {}),")
  })

  /*
   * A READ OUTSIDE THE FOLDER IS NOT "READING IS DISABLED" (2026-11-11). Scout, in Ask on OpenCode 1.19.5, asked for
   * files in another folder; each was refused as "Ask mode, which reads and changes nothing", and it told Colin reading
   * was disabled. A read outside the folder now says which folder it reads and what to do instead.
   */
  it('says a read outside the folder is outside it, and names the folder', () => {
    const folder = '/Users/me/now-coding'
    const outside = openCodeReadOnlyRefusal('ask', { permission: 'external_directory', patterns: ['/Users/me/Documents/review/*'] }, folder)
    expect(outside).toBe(
      'Locust refused this without asking anyone: in Ask mode this teammate reads only inside its folder, /Users/me/now-coding, and /Users/me/Documents/review is outside it, so nothing was read. Read files inside that folder, or ask the person to move or attach the ones you need there.'
    )
    expect(outside).not.toMatch(/changes nothing|declined/i)
    expect(openCodeReadOnlyRefusal('plan', { permission: 'read', patterns: ['/elsewhere/PLAN.md'] }, folder)).toContain('in Plan mode this teammate reads only inside its folder, /Users/me/now-coding, and /elsewhere/PLAN.md is outside it')
    // An outside request that names no path still says where it may read.
    expect(openCodeReadOnlyRefusal('ask', { permission: 'external_directory', patterns: [] }, folder)).toContain('reads only inside its folder, /Users/me/now-coding, so nothing was read')
  })

  it('keeps the mode\'s words for everything that is not a read outside the folder', () => {
    const folder = '/Users/me/now-coding'
    const plain = openCodeReadOnlyRefusal('ask')
    expect(openCodeReadOnlyRefusal('ask', { permission: 'bash', patterns: ['ls /Users/me/Documents'] }, folder)).toBe(plain)
    expect(openCodeReadOnlyRefusal('ask', { permission: 'edit', patterns: ['/Users/me/Documents/a.md'] }, folder)).toBe(plain)
    // A read inside the folder that Ask still refuses (a .env, say) is not called "outside".
    expect(openCodeReadOnlyRefusal('ask', { permission: 'read', patterns: ['/Users/me/now-coding/.env'] }, folder)).toBe(plain)
  })
})
