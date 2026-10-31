// A first try of film-lib: Home with a team of three on three agents, then Settings. No model calls.
import { join } from 'node:path'
import { say, scratchRepository, startDrive } from './drive-lib.mjs'
import { startFilming, stitch } from './film-lib.mjs'

const teammate = (id, name, hue, roleTitle, runtime) => ({ teammateId: id, name, hue, role: 'Custom', roleTitle, createdAt: '2026-10-31T12:00:00.000Z', route: { runtime, model: 'account-default', mode: 'ask' } })
const seed = {
  schemaVersion: 1,
  teammates: [teammate('tm_wrenfilm0000000000000', 'Wren', 'lime', 'Builder', 'claude-code'), teammate('tm_atlasfilm000000000000', 'Atlas', 'blue', 'Reviewer', 'codex'), teammate('tm_iris0film000000000000', 'Iris', 'violet', 'Researcher', 'opencode')],
  missionOwners: {},
  settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' }
}
const drive = await startDrive({ name: 'film-try', port: 9900, workspace: await scratchRepository('locust-film-try-'), seed, sendsNothing: true, holdCursor: false })
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
try {
  await drive.ready()
  await drive.resize?.(1440, 900)
  await pause(1500)
  const film = await startFilming(drive, join(drive.out, 'frames'))
  await pause(5000)
  await drive.evaluate(`[...document.querySelectorAll('.lc-sidebar__nav button')].find(b => /Settings/.test(b.innerText))?.click()`)
  await pause(2500)
  await drive.evaluate(`document.querySelector('[data-setting="your-account"]')?.scrollIntoView({ behavior: 'smooth', block: 'start' })`)
  await pause(3000)
  const frames = await film.stop()
  say(`frames: ${String(frames.length)} over ${(frames[frames.length - 1].at - frames[0].at).toFixed(1)} s`)
  await stitch(frames, join(drive.out, 'film-try.mp4'))
  say(`film: ${join(drive.out, 'film-try.mp4')}`)
} catch (error) { say(error instanceof Error ? error.stack ?? error.message : String(error)) }
finally { await drive.finish({ intro: 'film-lib first try' }) }
