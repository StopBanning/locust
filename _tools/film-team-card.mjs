// THE LOCUST-TEAMS DEMO (2026-11-04): the real app, filmed while this drive does what the locust-teams README says.
// A Locust with nobody in it and a project open; Home's "Team from an image" takes the Solo founder card (its PNG
// from the locust-teams repo, given by LOCUST_TEAM_CARD_PATH, since a drive cannot press a native dialog); the three
// teammates arrive; Scope opens and its starter is pressed; the answer comes on the card's own free OpenCode model,
// in Ask mode. Nothing is spent.
//
//   node _tools/film-team-card.mjs [--card <path to a .team.png>] [--folder <new folder>] [--take <name>]
//
// Writes the frames and take.json (the moments) to ~/work/scratch/film/teams-take-<name>-<time>/record/, and the
// stitched take as take.mp4 beside them; the README's GIF is cut from that.
import { execFileSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { mkdir, writeFile } from 'node:fs/promises'
import { homedir } from 'node:os'
import { join } from 'node:path'
import { say, startDrive } from './drive-lib.mjs'
import { startFilming, stitch } from './film-lib.mjs'

const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
const arg = (name, otherwise) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : otherwise)
const TAKE = arg('--take', 'next')
const CARD = arg('--card', join(homedir(), 'work', 'locust-teams', 'teams', 'solo-founder.team.png'))
const WINDOW = [1280, 800]
const SCALE = 2
if (!existsSync(CARD)) throw new Error(`no team card at ${CARD}`)

const stamp = new Date().toISOString().slice(0, 19).replace(/[:T]/g, '-')
const filmRoot = join(homedir(), 'work', 'scratch', 'film', `teams-take-${TAKE}-${stamp}`)
const FOLDER = arg('--folder', undefined)
if (FOLDER !== undefined && existsSync(FOLDER)) throw new Error(`${FOLDER} is there already: --folder must be a new folder`)
const workspace = FOLDER ?? join(filmRoot, 'weekend-app')
await mkdir(workspace, { recursive: true })
await writeFile(join(workspace, 'README.md'), '# Weekend App\n\nAn idea: a small web app for planning a weekend with friends. Pick a place, split the costs, vote on the plan.\nNobody has used it yet.\n', 'utf8')
execFileSync('git', ['init', '-q'], { cwd: workspace })
execFileSync('git', ['add', '.'], { cwd: workspace })
execFileSync('git', ['-c', 'user.name=Film', '-c', 'user.email=film@locust.lol', 'commit', '-qm', 'Weekend App'], { cwd: workspace })

const drive = await startDrive({
  name: `teams-take-${TAKE}`, port: 9904, workspace, holdCursor: false, keep: true, outPath: join(filmRoot, 'record'),
  env: { LOCUST_TEAM_CARD_PATH: CARD }, extraArgs: [`--force-device-scale-factor=${String(SCALE)}`],
  seed: { schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false } }
})
const until = async (expression, seconds = 60) => {
  for (let n = 0; n < seconds * 4; n += 1) { if (await drive.evaluate(expression)) return true; await pause(250) }
  return false
}

const marks = {}
const mark = (name) => { marks[name] = Date.now() / 1000; say(`mark ${name}`) }
let film
let failed = false
try {
  await drive.ready()
  await drive.resize(...WINDOW, SCALE)
  const painted = String(await drive.evaluate('devicePixelRatio + " " + innerWidth + "x" + innerHeight'))
  say(`window: ${painted}`)
  if (!painted.startsWith(`${String(SCALE)} `)) throw new Error(`the window paints at ${painted}, not at a scale of ${String(SCALE)}`)
  await pause(2000)

  film = await startFilming(drive, join(drive.out, 'frames'))
  mark('home')
  await pause(2500)

  mark('import')
  const pressed = String(await drive.evaluate(`(() => {
    const button = [...document.querySelectorAll('button')].find((one) => one.innerText.trim() === 'Team from an image')
    if (!button) return 'no Team from an image'
    button.click()
    return 'pressed'
  })()`))
  if (pressed !== 'pressed') throw new Error(pressed)
  if (!(await until(`/from the team card/.test(document.querySelector('.lc-teamnotice')?.textContent ?? '')`, 15))) throw new Error('no notice after the import')
  say(`notice: ${String(await drive.evaluate(`document.querySelector('.lc-teamnotice')?.textContent ?? ''`))}`)
  mark('added')
  await pause(3500)

  mark('scope')
  await drive.evaluate(`[...document.querySelectorAll('.lc-faces__one')].find((one) => /Scope/.test(one.getAttribute('aria-label') ?? ''))?.click()`)
  if (!(await until(`!!document.querySelector('.lc-starter')`, 15))) throw new Error('Scope opened with no starter')
  mark('starters')
  await pause(2200)

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
  // Back up to where the answer starts, as a person would to read it, and stay there for the cut's last shot.
  mark('read')
  await drive.evaluate(`[...document.querySelectorAll('.lc-thread p')].find((one) => one.innerText.length > 80)?.scrollIntoView({ behavior: 'smooth', block: 'start' })`)
  await pause(4500)
} catch (error) {
  failed = true
  say(`film failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  const frames = film === undefined ? [] : await film.stop()
  await writeFile(join(drive.out, 'take.json'), `${JSON.stringify({ window: WINDOW, scale: SCALE, card: CARD, marks, first: frames[0]?.at }, null, 2)}\n`, 'utf8')
  if (frames.length > 0) say(`take: ${await stitch(frames, join(drive.out, 'take.mp4'))} (${String(frames.length)} frames)`)
  await drive.finish({ intro: `The locust-teams demo, take ${TAKE}: an empty Locust, Team from an image (${CARD}), Scope's starter.`, extra: failed ? 'The take failed.' : 'The take finished.' })
}
process.exit(failed ? 1 : 0)
