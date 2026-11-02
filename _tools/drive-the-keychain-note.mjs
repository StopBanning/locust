// The keychain is asked only for a key, and the window says so first (0.854, main/keychain.ts).
//
//   node _tools/drive-the-keychain-note.mjs [--packaged <exe>] [--tag <name>]
//
// A drive reads a mock keychain, which never asks; LOCUST_KEYCHAIN_ASKS=1 has the dev build say the note anyway and
// hold it 3 s. Three launches: a teammate's key kept, on this build's first launch -- the note goes up once the window
// shows, then comes down, and the build is remembered; the same profile again -- nothing said; and a profile that keeps
// no key -- nothing said, nothing remembered.

import { mkdir, readFile } from 'node:fs/promises'
import { join } from 'node:path'

import { recordRoot, say, scratchRepository, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag')
const outPath = tag === undefined ? undefined : join(recordRoot('beta-fixes-2026-09-24'), `keychain-note-${tag}`)
if (outPath !== undefined) await mkdir(outPath, { recursive: true })

const T0 = '2026-11-01T20:00:00.000Z'
const NOTE = '[data-keychain-note]'
const seed = {
  schemaVersion: 1,
  teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: T0 }],
  missionOwners: {},
  settings: { swarm: false, memoryMode: 'off' }
}
// A key as the store keeps it: only whether one is there is read before the note.
const keptKey = JSON.stringify({ schemaVersion: 1, teammates: { tm_wren: { STRIPE_KEY: 'c2VhbGVkIGZvciB0aGUgZHJpdmU=' } } })
const env = { LOCUST_KEYCHAIN_ASKS: '1' }
const verdicts = []
const judge = (name, pass, seen) => {
  verdicts.push(`${name}: ${pass ? 'PASS' : 'FAIL'} (${seen})`)
  say(`${pass ? '[PASS]' : '[FAIL]'} ${name} -- ${seen}`)
}
const remembered = async (profile) => {
  try { return (JSON.parse(await readFile(join(profile, 'keychain-opened.json'), 'utf8'))).version } catch { return undefined }
}
/** Whether the note shows within `ms`, read every tenth of a second. */
const noteWithin = async (drive, ms) => {
  try { return String(await drive.waitForSelector(NOTE, { timeoutMs: ms, everyMs: 100, what: 'the keychain note' })) } catch { return undefined }
}

const workspace = await scratchRepository('locust-drive-keychain-ws-')
let first
try {
  first = await startDrive({ name: 'keychain-note', port: 9571, workspace, seed, files: { 'teammate-keys.json': keptKey }, env, keep: true, sendsNothing: true, ...(packaged === undefined ? {} : { packaged }), ...(outPath === undefined ? {} : { outPath }) })
  let words
  await first.capture('a key kept, this build first: the note, once the window shows', async () => {
    words = await noteWithin(first, 90_000)
    return words ?? 'no note in 90 s'
  })
  judge('with a key kept, the window says macOS may ask before the keychain is opened', words !== undefined && /Always Allow/.test(words), words ?? 'never shown')
  let gone = false
  try { await first.waitFor(`document.querySelector('${NOTE}') === null`, { timeoutMs: 15_000, everyMs: 100, what: 'the note to come down' }); gone = true } catch { gone = false }
  await first.capture('the keychain opened: the note is down', async () => (gone ? 'the note came down' : 'the note is still up'))
  judge('the note comes down once the keychain has answered', gone, gone ? 'down' : 'still up after 15 s')
  const version = await remembered(first.profile)
  judge('the build is remembered, so its next launch says nothing', typeof version === 'string', String(version))
} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await first?.finish({ intro: `Build: ${packaged ?? 'whatever pnpm build last wrote to out/'}. A mock keychain told to ask (LOCUST_KEYCHAIN_ASKS=1, held 3 s).`, last: false })
}

let again
try {
  again = await startDrive({ name: 'keychain-note', port: 9571, workspace, seed, env, profilePath: first?.profile, keep: false, sendsNothing: true, stepFrom: 3, ...(packaged === undefined ? {} : { packaged }), ...(outPath === undefined ? {} : { outPath }) })
  await again.ready()
  let words
  await again.capture('the same build again: nothing said', async () => {
    words = await noteWithin(again, 12_000)
    return words ?? 'no note in 12 s'
  })
  judge("the same build's next launch says nothing", words === undefined, words ?? 'none')
} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await again?.finish({ intro: 'The same profile, launched again.', last: false })
}

let bare
try {
  bare = await startDrive({ name: 'keychain-note', port: 9571, workspace, seed, env, sendsNothing: true, stepFrom: 5, ...(packaged === undefined ? {} : { packaged }), ...(outPath === undefined ? {} : { outPath }) })
  await bare.ready()
  let words
  await bare.capture('no key kept: nothing said', async () => {
    words = await noteWithin(bare, 12_000)
    return words ?? 'no note in 12 s'
  })
  judge('a profile that keeps no key is never told macOS may ask', words === undefined, words ?? 'none')
  const version = await remembered(bare.profile)
  judge('and nothing is remembered, since nothing was opened', version === undefined, String(version))
} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await bare?.finish({ intro: 'A fresh profile with no key kept.', extra: verdicts.length === 0 ? '' : ['## Verdicts', '', ...verdicts.map((line) => `- ${line}`)].join('\n') })
}
