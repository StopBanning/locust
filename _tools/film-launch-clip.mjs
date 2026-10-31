// THE LAUNCH CLIP, TAKE BY TAKE (2026-10-31): the real app, filmed while this drive does what a person does.
// Take 1: Home with a team on four agents; the trending skills kept for every agent; a message to Kit, its answer
// streaming in on a free model (no spending). Writes the raw take and its frames; the cut and captions come after.
import { execFileSync } from 'node:child_process'
import { mkdir, writeFile } from 'node:fs/promises'
import { homedir } from 'node:os'
import { join } from 'node:path'
import { say, startDrive } from './drive-lib.mjs'
import { startFilming, stitch } from './film-lib.mjs'

const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
const teammate = (id, name, hue, roleTitle, runtime, model, mode = 'ask') => ({ teammateId: id, name, hue, role: 'Custom', roleTitle, createdAt: '2026-10-31T12:00:00.000Z', route: { runtime, model, mode } })
const seed = {
  schemaVersion: 1,
  teammates: [
    teammate('tm_wrenclip0000000000000', 'Wren', 'lime', 'Builder', 'claude', 'opus'),
    teammate('tm_atlasclip000000000000', 'Atlas', 'blue', 'Reviewer', 'codex', 'gpt-6.1-sol'),
    teammate('tm_irisclip0000000000000', 'Iris', 'violet', 'Researcher', 'antigravity', 'gemini-4-argon'),
    teammate('tm_kitclip00000000000000', 'Kit', 'clay', 'Writer', 'kilo', 'kilo/kilo-auto/free')
  ],
  missionOwners: {},
  settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' }
}

// A project folder with a name worth reading in the title bar.
const workspace = join(homedir(), 'locust-scratch', 'film', `weekend-app-${String(Date.now()).slice(-5)}`, 'weekend-app')
await mkdir(workspace, { recursive: true })
await writeFile(join(workspace, 'index.html'), '<!doctype html>\n<title>Weekend App</title>\n<h1>Weekend App</h1>\n<p>Plan a weekend with friends: pick a place, split the costs, vote on the plan.</p>\n<button>Start a plan</button>\n', 'utf8')
await writeFile(join(workspace, 'README.md'), '# Weekend App\n\nA small web app for planning a weekend with friends.\n', 'utf8')
execFileSync('git', ['init', '-q'], { cwd: workspace })
execFileSync('git', ['add', '.'], { cwd: workspace })
execFileSync('git', ['-c', 'user.name=Film', '-c', 'user.email=film@locust.lol', 'commit', '-qm', 'Weekend App'], { cwd: workspace })

const drive = await startDrive({ name: 'launch-clip-take1', port: 9902, workspace, seed, holdCursor: false, keep: true })
const section = `document.querySelector('[data-setting="skills-from-github"]')`
const until = async (expression, seconds = 60) => {
  for (let n = 0; n < seconds * 4; n += 1) { if (await drive.evaluate(expression)) return true; await pause(250) }
  return false
}
try {
  await drive.ready()
  await drive.resize?.(1440, 900)
  await pause(2500)
  const film = await startFilming(drive, join(drive.out, 'frames'))
  const marks = {}
  // 1. Home: the team, each on its own agent.
  marks.home = film.mark()
  await pause(5000)
  // 2. The trending skills, kept for every agent.
  marks.skills = film.mark()
  await drive.evaluate(`[...document.querySelectorAll('.lc-sidebar__nav button')].find(b => /Settings/.test(b.innerText))?.click()`)
  await until(`!!document.querySelector('.lc-settings__navitem')`)
  await drive.evaluate(`[...document.querySelectorAll('.lc-settings__navitem')].find(b => b.innerText.trim() === 'Teammates')?.click()`)
  await until(`!!${section}`)
  await drive.evaluate(`[...${section}.querySelectorAll('.lc-skilllib__name')].find(el => el.innerText === 'New this week')?.scrollIntoView({ behavior: 'smooth', block: 'start' })`)
  await pause(1800)
  await drive.evaluate(`[...${section}.querySelectorAll('.lc-skilllib__tile')].find(tile => tile.innerText.includes('t3dotgg/tstack')).querySelector('.lc-skilllib__pack').click()`)
  await until(`!!${section}.querySelector('.lc-skilllib__actions .lc-primarybutton')`, 90)
  await drive.evaluate(`${section}.querySelector('.lc-skilllib__preview')?.scrollIntoView({ behavior: 'smooth', block: 'start' })`)
  await pause(2000)
  await drive.evaluate(`${section}.querySelector('.lc-skilllib__actions')?.scrollIntoView({ behavior: 'smooth', block: 'center' })`)
  await pause(1000)
  await drive.evaluate(`${section}.querySelector('.lc-skilllib__actions .lc-primarybutton').click()`)
  await until(`${section}.querySelectorAll('.lc-skilllib__kept').length === 1`, 240)
  await drive.evaluate(`${section}.querySelector('.lc-skilllib__kept')?.scrollIntoView({ behavior: 'smooth', block: 'center' })`)
  await pause(2500)
  // 3. A message to Kit; the answer streams in.
  marks.message = film.mark()
  await drive.evaluate(`[...document.querySelectorAll('.lc-faces__one')].find((one) => /Kit/.test(one.getAttribute('aria-label') ?? ''))?.click()`)
  await until(`(document.querySelector('form.command-dock textarea')?.getAttribute('placeholder') ?? '').startsWith('Message Kit')`, 20)
  await pause(600)
  const ask = 'Read index.html and suggest one thing to build next, in two sentences.'
  await drive.evaluate(`(async () => {
    const field = document.querySelector('form.command-dock textarea')
    field.focus()
    const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
    const text = ${JSON.stringify(ask)}
    for (let i = 1; i <= text.length; i += 1) {
      setter.call(field, text.slice(0, i))
      field.dispatchEvent(new Event('input', { bubbles: true }))
      await new Promise((r) => setTimeout(r, 28))
    }
    return true
  })()`)
  await pause(500)
  await drive.evaluate(`document.querySelector('button[aria-label="Send"]')?.click()`)
  const started = Date.now()
  await pause(3000)
  while (Date.now() - started < 60_000 && await drive.evaluate(`!!document.querySelector('button[aria-label^="Stop the running"]')`)) await pause(500)
  await pause(2500)
  marks.end = film.mark()
  const frames = await film.stop()
  await writeFile(join(drive.out, 'marks.json'), `${JSON.stringify(marks, null, 2)}\n`, 'utf8')
  say(`frames ${String(frames.length)}, ${(frames[frames.length - 1].at - frames[0].at).toFixed(1)} s; marks ${JSON.stringify(marks)}`)
  await stitch(frames, join(drive.out, 'take1.mp4'))
  say(`take: ${join(drive.out, 'take1.mp4')}`)
  say(`answer: ${await drive.evaluate(`document.querySelector('.lc-thread')?.innerText.replace(/\\s+/g, ' ').slice(-400) ?? ''`)}`)
} catch (error) { say(error instanceof Error ? error.stack ?? error.message : String(error)) }
finally { await drive.finish({ intro: 'Launch clip, take 1' }) }
