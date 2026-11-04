// THE LOCUST-TEAMS DEMO IN LIQUID GLASS (2026-11-04): film-team-card.mjs's steps, in the look a Mac person picks.
// Liquid glass over Locust's own Rainbow (so no frame can show the desktop), plush faces, Dark. A page capture shows
// none of the native glass, so every frame is macOS's own capture of this one window (screencapture -l), taken
// wherever the window is: nothing is brought to the front, and no other app's window is ever in a frame.
//
//   node _tools/film-team-card-glass.mjs [--card <path to a .team.png>] [--take <name>]
//
// Writes frames/*.jpg, take.json (marks, each frame's time) and take.mp4 to
// ~/work/scratch/film/teams-glass-<name>-<time>/record/. The README's GIF is cut from take.mp4.
import { execFile, execFileSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { mkdir, writeFile } from 'node:fs/promises'
import { homedir } from 'node:os'
import { join } from 'node:path'
import { promisify } from 'node:util'
import { say, startDrive } from './drive-lib.mjs'

if (process.platform !== 'darwin') throw new Error('Liquid glass is a Mac look: this film runs on a Mac.')
const run = promisify(execFile)
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
const arg = (name, otherwise) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : otherwise)
const TAKE = arg('--take', 'next')
const CARD = arg('--card', join(homedir(), 'work', 'locust-teams', 'teams', 'solo-founder.team.png'))
if (!existsSync(CARD)) throw new Error(`no team card at ${CARD}`)

const stamp = new Date().toISOString().slice(0, 19).replace(/[:T]/g, '-')
const filmRoot = join(homedir(), 'work', 'scratch', 'film', `teams-glass-${TAKE}-${stamp}`)
const workspace = join(filmRoot, 'weekend-app')
await mkdir(workspace, { recursive: true })
await writeFile(join(workspace, 'README.md'), '# Weekend App\n\nAn idea: a small web app for planning a weekend with friends. Pick a place, split the costs, vote on the plan.\nNobody has used it yet.\n', 'utf8')
execFileSync('git', ['init', '-q'], { cwd: workspace })
execFileSync('git', ['add', '.'], { cwd: workspace })
execFileSync('git', ['-c', 'user.name=Film', '-c', 'user.email=film@locust.lol', 'commit', '-qm', 'Weekend App'], { cwd: workspace })

/** This film's window: the largest ordinary window of the app's own process. */
const windowOf = (pid) => Number(execFileSync('osascript', ['-l', 'JavaScript', '-e', [
  'ObjC.import("CoreGraphics");',
  'var list = ObjC.castRefToObject($.CGWindowListCopyWindowInfo($.kCGWindowListOptionAll, 0));',
  'var best = 0, area = 0;',
  'for (var i = 0; i < list.count; i++) { var d = list.objectAtIndex(i);',
  `  if (ObjC.unwrap(d.objectForKey("kCGWindowOwnerPID")) !== ${String(pid)} || ObjC.unwrap(d.objectForKey("kCGWindowLayer")) !== 0) continue;`,
  '  var b = d.objectForKey("kCGWindowBounds"); var a = ObjC.unwrap(b.objectForKey("Width")) * ObjC.unwrap(b.objectForKey("Height"));',
  '  if (a > area) { area = a; best = ObjC.unwrap(d.objectForKey("kCGWindowNumber")); } }',
  'best'
].join(' ')]).toString().trim())

const drive = await startDrive({
  name: `teams-glass-${TAKE}`, port: 9906, workspace, holdCursor: false, keep: true, outPath: join(filmRoot, 'record'),
  env: { LOCUST_TEAM_CARD_PATH: CARD },
  seed: { schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false, plush: true } }
})
const until = async (expression, seconds = 60) => {
  for (let n = 0; n < seconds * 4; n += 1) { if (await drive.evaluate(expression)) return true; await pause(250) }
  return false
}

/** Frames as fast as macOS hands them over, each stamped with when it was asked for. */
const frames = []
let filming = false
let filmed
const film = (id, dir) => {
  filming = true
  filmed = (async () => {
    while (filming) {
      const at = Date.now() / 1000
      const file = join(dir, `${String(frames.length).padStart(5, '0')}.jpg`)
      try {
        await run('screencapture', ['-x', '-o', '-t', 'jpg', `-l${String(id)}`, file])
        frames.push({ file, at })
      } catch { /* one missed frame */ }
    }
  })()
}

const marks = {}
const mark = (name) => { marks[name] = Date.now() / 1000; say(`mark ${name}`) }
let failed = false
try {
  await drive.ready()
  // Rainbow first, then the glass: no frame, filmed or not, ever has the desktop under it.
  say(`ground: ${String(await drive.evaluate(`window.desktop.setGlassGround('rainbow').then((s) => s.glassGround)`))}`)
  say(`look: ${String(await drive.evaluate(`window.desktop.setLook('liquid').then((s) => s.look + ' offered ' + s.liquidOffered)`))}`)
  await pause(2500)
  const id = windowOf(drive.pid)
  if (!id) throw new Error('no window for the app')
  say(`window ${String(id)}: ${String(await drive.evaluate('innerWidth + "x" + innerHeight + " @" + devicePixelRatio'))}`)
  await mkdir(join(drive.out, 'frames'), { recursive: true })
  film(id, join(drive.out, 'frames'))
  mark('home')
  await pause(3000)

  mark('import')
  const pressed = String(await drive.evaluate(`(() => {
    const button = [...document.querySelectorAll('button')].find((one) => one.innerText.trim() === 'Team from an image')
    if (!button) return 'no Team from an image'
    button.click()
    return 'pressed'
  })()`))
  if (pressed !== 'pressed') throw new Error(pressed)
  if (!(await until(`/from the team card/.test(document.querySelector('.lc-teamnotice')?.textContent ?? '')`, 15))) throw new Error('no notice after the import')
  mark('added')
  await pause(4000)

  mark('scope')
  await drive.evaluate(`[...document.querySelectorAll('.lc-faces__one')].find((one) => /Scope/.test(one.getAttribute('aria-label') ?? ''))?.click()`)
  if (!(await until(`!!document.querySelector('.lc-starter')`, 15))) throw new Error('Scope opened with no starter')
  mark('starters')
  await pause(2500)

  mark('starter')
  await drive.evaluate(`document.querySelector('.lc-starter')?.click()`)
  const started = Date.now()
  await pause(3000)
  while (Date.now() - started < 150_000 && await drive.evaluate(`!!document.querySelector('button[aria-label^="Stop the running"]')`)) await pause(400)
  mark('answered')
  const said = String(await drive.evaluate(`document.querySelector('.lc-thread')?.innerText.replace(/\\s+/g, ' ').slice(-600) ?? ''`))
  say(`answer: ${said}`)
  if (/Internal error|stopped with an error|not available/i.test(said)) { failed = true; say('THE TURN FAILED: retake') }
  await pause(1500)
  mark('read')
  await drive.evaluate(`[...document.querySelectorAll('.lc-thread p')].find((one) => one.innerText.length > 80)?.scrollIntoView({ behavior: 'smooth', block: 'start' })`)
  await pause(5000)
  mark('end')
} catch (error) {
  failed = true
  say(`film failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  filming = false
  await filmed
  const seconds = frames.length > 1 ? frames.at(-1).at - frames[0].at : 0
  say(`${String(frames.length)} frames over ${seconds.toFixed(1)} s (${(frames.length / Math.max(seconds, 1)).toFixed(1)} a second)`)
  await writeFile(join(drive.out, 'take.json'), `${JSON.stringify({ card: CARD, marks, first: frames[0]?.at, frames: frames.map((f) => f.at) }, null, 2)}\n`, 'utf8')
  if (frames.length > 1) {
    // Each frame held until the next was asked for, so the take runs in real time however unevenly frames came.
    const list = frames.map((f, i) => `file '${f.file}'\nduration ${((frames[i + 1]?.at ?? f.at + 0.1) - f.at).toFixed(3)}`).join('\n')
    await writeFile(join(drive.out, 'frames.txt'), `${list}\nfile '${frames.at(-1).file}'\n`, 'utf8')
    execFileSync('ffmpeg', ['-y', '-loglevel', 'error', '-f', 'concat', '-safe', '0', '-i', join(drive.out, 'frames.txt'), '-vf', 'scale=trunc(iw/2)*2:trunc(ih/2)*2,fps=30', '-pix_fmt', 'yuv420p', '-c:v', 'libx264', '-crf', '16', join(drive.out, 'take.mp4')])
    say(`take: ${join(drive.out, 'take.mp4')}`)
  }
  await drive.finish({ intro: `The locust-teams demo in Liquid glass, take ${TAKE}: Rainbow, plush, ${CARD}.`, extra: failed ? 'The take failed.' : 'The take finished.' })
}
process.exit(failed ? 1 : 0)
