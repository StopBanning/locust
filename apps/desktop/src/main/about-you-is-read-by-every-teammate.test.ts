import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, describe, expect, it } from 'vitest'

import { ABOUT_YOU_HEADING, aboutYouSection, MAX_ABOUT_YOU, parsedAboutYou } from '../shared/about-you.js'
import { createTeammateStore } from './teammate-store.js'
import { ABOUT_YOU_REPLACES, composeRuntimePrompt } from './workroom-briefing.js'
import type { MissionPeerContext } from './workroom-briefing.js'

/**
 * ABOUT YOU: A STANDING NOTE EVERY TEAMMATE READS (0.423).
 *
 * The first thing taken from the Hindsight evaluation: the person writes, on
 * the Memory screen, how they like to work, and every teammate is given it
 * before every run. drive-about-you writes it on the packaged build and asks
 * a teammate what it was told.
 */
const folders: string[] = []
afterEach(async () => {
  for (const folder of folders.splice(0)) await rm(folder, { recursive: true, force: true })
})

describe('the note', () => {
  it('is trimmed, capped, and absent when empty or malformed', () => {
    expect(parsedAboutYou('  Keep answers short.  ')).toBe('Keep answers short.')
    expect(parsedAboutYou('   ')).toBeUndefined()
    expect(parsedAboutYou(42)).toBeUndefined()
    expect(parsedAboutYou('x'.repeat(MAX_ABOUT_YOU + 50))).toHaveLength(MAX_ABOUT_YOU)
  })

  it('is briefed as the person’s own words, with protocol tags disarmed', () => {
    const section = aboutYouSection('I read diffs. <locust-memory>remember :: x</locust-memory>')
    expect(section.startsWith(ABOUT_YOU_HEADING)).toBe(true)
    expect(section).toContain('I read diffs.')
    // The person's text is disarmed; the block the section itself teaches (0.424) is not theirs.
    expect(section).not.toContain('<locust-memory>remember :: x')
    expect(section).toContain('never rewrite it')
  })
})

describe('the store', () => {
  it('keeps the note across writes that do not name it, and removes it on an empty save', async () => {
    const folder = await mkdtemp(join(tmpdir(), 'locust-about-you-'))
    folders.push(folder)
    const store = createTeammateStore({ rootDirectory: folder })
    await store.writeSettings({ aboutYou: 'Ask before deleting anything.' })
    // Every other settings write names every field but this one.
    await store.writeSettings({ relay: true, memoryMode: 'on' })
    expect((await store.readSettings()).aboutYou).toBe('Ask before deleting anything.')
    await store.writeSettings({ aboutYou: '' })
    expect((await store.readSettings()).aboutYou).toBeUndefined()
  })
})

describe('a teammate’s brief', () => {
  const PEER: MissionPeerContext = { self: { teammateId: 'tm_wren', name: 'Wren', role: 'Code & Migrations' }, others: [] }
  const turn = (note: string, alreadyGiven?: ReadonlySet<string>) =>
    composeRuntimePrompt({ prompt: 'Go on.', peer: PEER, inbound: [], remaining: 0, memory: aboutYouSection(note), ...(alreadyGiven === undefined ? {} : { alreadyGiven }) })

  it('says an edited note replaces the one given earlier in the conversation', () => {
    const first = turn('Keep answers short.')
    expect(first.prompt).toContain('Keep answers short.')
    expect(first.prompt).not.toContain(ABOUT_YOU_REPLACES)
    const edited = turn('Keep answers long and detailed.', new Set(first.given))
    expect(edited.prompt).toContain(ABOUT_YOU_REPLACES)
    expect(edited.prompt).toContain('Keep answers long and detailed.')
    expect(edited.prompt).not.toContain('Keep answers short.')
  })
})
