// The shared half of every "drive the app like a person" script.
//
// A drive is not a smoke: nothing here asserts. It launches the BUILT app
// on a throwaway profile, does what a person does, and keeps what the
// screen showed at each step -- the text, a screenshot, and the renderer's
// error count -- under docs/user-session/<stamp>-<name>/. The judging is
// done afterwards by reading the record (Colin, 2026-09-05: "start taking
// screenshots when you drive the app and test so you can get a user
// experience"), so a hitch is a thing that was seen, not a thing the script
// was told to look for.
//
// Every drive picks its own debugging port and refuses to start when that
// port already answers: two drives on one port both bind it, and the second
// silently drives the first app.

// FIRST, for its side effect: it points tmpdir() outside AppData, which is
// where `~/.cursorignore` makes every Cursor run blind. See the file.
import './scratch-root.mjs'
import { holdCursorDefault } from './cursor-default-hold.mjs'

import { spawn, execFile, execFileSync } from 'node:child_process'
import { readFileSync, rmSync } from 'node:fs'
import { mkdir, mkdtemp, readFile, rm, stat, writeFile } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { delimiter, join } from 'node:path'
import { fileURLToPath } from 'node:url'

/**
 * End the app AND its helper processes.
 *
 * child.kill() ends only Electron's main process on Windows; the GPU,
 * renderer and utility processes it started can outlive it, holding the
 * build's files. Five of them, left by the update smoke of 0.309, made
 * electron-builder fail to package 0.310 (2026-09-24). The tree is ended
 * while its root is still there -- once the root is gone, taskkill cannot
 * find the orphans by it.
 */
function endTree(child) {
  if (process.platform === 'win32' && child.pid !== undefined && child.exitCode === null) {
    try {
      execFileSync('taskkill', ['/PID', String(child.pid), '/T', '/F'], { stdio: 'ignore', windowsHide: true })
      return
    } catch { /* already gone */ }
  }
  try { child.kill() } catch { /* gone */ }
}

/**
 * A file URL as a path on this machine. On Windows, as it always was (`/C:/x` less its slash); elsewhere a URL's
 * pathname IS the path, and slicing it made `/Users/x` the relative `Users/x` (the first Mac run, 2026-10-30).
 */
export const localPath = (url) => (process.platform === 'win32' ? url.pathname.slice(1) : fileURLToPath(url))

export const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = process.platform === 'win32'
  ? join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
  : process.platform === 'darwin'
    ? join(APP_DIR, 'node_modules', 'electron', 'dist', 'Electron.app', 'Contents', 'MacOS', 'Electron')
    : join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron')
// npm's own folder goes first on Windows; elsewhere the app builds its PATH from the login shell (shell-path.ts).
const NPM_DIR = process.platform === 'win32' ? join(homedir(), 'AppData', 'Roaming', 'npm') : undefined
// LOCUST_FREE_MODEL picks another of OpenCode's free models when this one is
// down (Muse Spark was, 9/22-9/24); `opencode models | grep free` lists them.
// Muse Spark left OpenCode's free list by 2026-10-18, and every drive on this
// route stopped before sending; DeepSeek V4.1 Flash wrote and handed over a
// file cleanly that day (drive-a-file-written-elsewhere).
export const FREE_ROUTE = { runtime: 'opencode', model: process.env.LOCUST_FREE_MODEL ?? 'opencode/deepseek-v4.1-flash-free', mode: 'accept-edits' }
/**
 * That model's row in the picker, as pickRouteScript takes it: the words of
 * its id in order, which is how the picker draws its name
 * (`opencode/muse-spark-1.3-contributor-free` is "Muse Spark 1.3 Contributor
 * Free"). It was Muse by name, so a drive moved onto the free route ran Muse
 * whatever LOCUST_FREE_MODEL said -- and on 2026-09-26 Muse and Ling were both
 * rate limited for hours while three other free models answered.
 */
export const FREE_ROW = '/' + FREE_ROUTE.model.replace(/^opencode\//, '').replace(/-free$/, '').split('-').map((word) => word.replace(/[.*+?^$()|[\]\\{}]/g, (character) => '\\' + character)).join('.*') + '/i'
/**
 * What to type to find that row: its own words, not "free". A search shows 12
 * rows a group (ROUTE_SEARCH_GROUP_LIMIT); OpenCode listed 10 free models on
 * 2026-10-08 and adds them on its own clock, so "free" alone stops reaching the
 * last of them once the list passes the cut. (That day's "the free route is not
 * there yet" was not the cut: the list itself came 14 s late, 0.709.)
 */
export const FREE_SEARCH = FREE_ROUTE.model.replace(/^opencode\//, '').replace(/-free$/, '').replace(/[-.]+/g, ' ')
/**
 * The folder a drive keeps its record in: docs/<folder> for a drive run by
 * hand, or <LOCUST_DRIVE_OUT>/<folder> when the sweep runs it, so a sweep
 * never writes into the repository.
 */
export const recordRoot = (folder) =>
  process.env.LOCUST_DRIVE_OUT === undefined ? localPath(new URL(`../docs/${folder}/`, import.meta.url)) : join(process.env.LOCUST_DRIVE_OUT, folder)
export const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
export const say = (line) => console.error(line)
export const reportRouteProbe = (route) => console.log(`route: ${String(route === '' ? 'no route control on this screen' : route)}`)
export const git = (args, cwd) => new Promise((resolve, reject) => {
  execFile('git', args, { cwd, windowsHide: true }, (error, stdout) => (error ? reject(error) : resolve(stdout)))
})

/**
 * DRIVES TAKE TURNS TO START.
 *
 * Four drives started together on this machine each launched an app that
 * probed seven runtimes at once -- some eighty CLI spawns -- and the probes
 * ran past their ten-second limit: two of four apps came up with five
 * runtimes of seven, and none could be put on the free route in two and a
 * half minutes, where one alone gets there in eight seconds (Yurt's beta
 * report and a measurement of it, 2026-09-23). Retrying the pick did not help;
 * the renderers were starved.
 *
 * So the launch, up to the end of `ready()`, holds a machine-wide turn: a
 * directory made atomically in the temp folder. The runs themselves still go
 * side by side -- only the start is one at a time. A turn older than three
 * minutes belongs to a drive that died holding it, and is taken over.
 */
const LAUNCH_TURN = join(tmpdir(), 'locust-drive-launch-turn')

async function takeLaunchTurn(name) {
  let waited = false
  for (;;) {
    try {
      await mkdir(LAUNCH_TURN)
      await writeFile(join(LAUNCH_TURN, 'holder'), `${String(process.pid)} ${name}`, 'utf8')
      if (waited) say(`${name}: my turn to start`)
      // A drive that dies or exits before giving the turn back must not
      // leave the others waiting out the three minutes.
      process.once('exit', () => {
        try {
          if (readFileSync(join(LAUNCH_TURN, 'holder'), 'utf8').startsWith(`${String(process.pid)} `)) rmSync(LAUNCH_TURN, { recursive: true, force: true })
        } catch { /* already given back */ }
      })
      return
    } catch {
      const held = await stat(LAUNCH_TURN).catch(() => undefined)
      if (held !== undefined && Date.now() - held.mtimeMs > 180_000) {
        await rm(LAUNCH_TURN, { recursive: true, force: true })
        continue
      }
      if (!waited) say(`${name}: another drive is starting; waiting for my turn`)
      waited = true
      await sleep(1000)
    }
  }
}

async function giveBackLaunchTurn() {
  const holder = await readFile(join(LAUNCH_TURN, 'holder'), 'utf8').catch(() => '')
  if (holder.startsWith(`${String(process.pid)} `)) await rm(LAUNCH_TURN, { recursive: true, force: true })
}

/**
 * A scratch git repository with a README and a LOCUST.md, committed.
 *
 * `brief` is that LOCUST.md, and Locust carries it to EVERY teammate started
 * in this folder -- it is not decoration. The default says only what the
 * folder is.
 *
 * It USED to say "Keep answers to one paragraph." That was there to keep
 * drive output short, and it kept confounding the drives instead: the cap
 * probe asked eight teammates to count to 250 and watched them answer "Your
 * 1-to-250 count conflicts with the one-paragraph rule" (2026-09-09); the
 * measure probe asked for three paragraphs and got a decision card asking
 * which instruction to follow (2026-09-10). Colin, the same day: "remove the
 * portion of keeps answers to one paragraph, thats a bit arbitrary." A
 * fixture that argues with the prompt is a silent confound, and one that
 * argues with the prompt by DEFAULT is a confound in every drive at once. A
 * drive that wants short answers can ask for them in its own prompt.
 */
export async function scratchRepository(prefix = 'locust-drive-ws-', brief = 'A scratch project for a user session.\n') {
  const workspace = await mkdtemp(join(tmpdir(), prefix))
  await git(['init', '-q', '-b', 'main'], workspace)
  await git(['config', 'user.email', 'drive@locust.test'], workspace)
  await git(['config', 'user.name', 'Locust drive'], workspace)
  await writeFile(join(workspace, 'README.md'), '# scratch\n\nA scratch project for a user session.\n', 'utf8')
  await writeFile(join(workspace, 'LOCUST.md'), brief, 'utf8')
  await git(['add', '.'], workspace)
  await git(['commit', '-q', '-m', 'first'], workspace)
  return workspace
}

/**
 * Launch the app and hand back the step recorder.
 *
 * `seed` is the teammates.json to start from (undefined = a fresh profile
 * and the first-launch screen). `env` is merged over the process
 * environment. `keep` leaves the profile and workspace on disk afterwards.
 */
/**
 * Drives that spend a PAID account, and therefore do not run by default.
 *
 * Colin, 2026-09-08: "u r why we have zero codex usage, why we cn run zero
 * tests on astra -- cause ur asking it to say pineapple." He is right. These
 * drives run on `codex / account-default`, which is his own Codex quota, and
 * they were being run dozens of times a night on one-word prompts to check a
 * button. The quota that paid for it was the quota an outside QA pass needed.
 *
 * A drive that spends real money is now opt-in: set LOCUST_SPEND=1 to allow
 * it. Everything else must run on a free model. This is a gate rather than a
 * note in a comment because a note does not stop anything.
 */
export function assertMaySpend(name) {
  if (process.env.LOCUST_SPEND === '1') return
  say(`refusing to run "${name}": it spends a paid account.`)
  say('This drive uses a real account quota. Re-run with LOCUST_SPEND=1 if you mean to spend it.')
  process.exit(1)
}

/*
 * The app's environment, held to free routes unless THIS drive said it spends and its runner said LOCUST_SPEND=1.
 *
 * It was lifted by LOCUST_SPEND=1 alone, so a free drive run under a LOCUST_SPEND=1 meant for another -- a sweep
 * started with it, the commands of a teammate in a window opened to spend (the overnight polish, 2026-10-12) --
 * opened a window that could start any paid route. A free drive's app now gets LOCUST_FREE_ONLY=1 and no
 * LOCUST_SPEND to read (main/free-routes.ts reads that first).
 */
export function spendGuarded(appEnv, spends, environment = process.env) {
  const guarded = { ...appEnv }
  if (spends && environment.LOCUST_SPEND === '1') delete guarded.LOCUST_FREE_ONLY
  else {
    guarded.LOCUST_FREE_ONLY = '1'
    delete guarded.LOCUST_SPEND
  }
  return guarded
}

export async function startDrive({ name, port, workspace, seed, files = {}, env = {}, keep = false, profilePath, outPath, stepFrom = 0, spends = false, sendsNothing = false, holdCursor = true, packaged, launchElsewhere = false, focused = false, extraArgs = [] }) {
  if (spends) assertMaySpend(name)
  try {
    const already = await fetch(`http://127.0.0.1:${String(port)}/json/list`, { signal: AbortSignal.timeout(1500) })
    if (already.ok) { say(`something is already debugging on port ${String(port)}`); process.exit(1) }
  } catch { /* free */ }

  const stamp = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19)
  // A relaunch on the same profile (a scheduled routine after a quit) keeps
  // writing into the same record, numbering its steps after the first run's.
  // LOCUST_DRIVE_OUT moves the default record somewhere else -- the sweep
  // (sweep-drives.mjs) keeps a whole run's captures out of the repository.
  const out = outPath ?? join(process.env.LOCUST_DRIVE_OUT ?? localPath(new URL('../docs/user-session/', import.meta.url)), `${stamp}-${name}`)
  await mkdir(out, { recursive: true })
  const profile = profilePath ?? await mkdtemp(join(tmpdir(), `locust-drive-${name}-`))
  if (seed !== undefined) {
    // The seed used to be skipped whenever a drive supplied its own
    // `profilePath` -- the two options were written for different reasons and
    // nobody had used them together. The result was silent: the app launched
    // with an EMPTY roster and the drive read that as the product's own
    // behaviour. Two drives on 2026-09-07 lost time to it, one of them
    // concluding the compact avatar rail was unimplemented when the rail had
    // simply been given nobody to draw.
    //
    // A drive must be able to assert its own premise, so the seed is written
    // wherever the profile is, and a profile that cannot take it says so
    // rather than starting a run whose setup is not what the script says.
    await mkdir(join(profile, 'mission-ledger'), { recursive: true }).catch(() => undefined)
    await writeFile(join(profile, 'teammates.json'), JSON.stringify(seed), 'utf8')
    const written = JSON.parse(await readFile(join(profile, 'teammates.json'), 'utf8'))
    const wanted = seed.teammates?.length ?? 0
    if ((written.teammates?.length ?? 0) !== wanted) {
      say(`seed did not land: wanted ${String(wanted)} teammates`)
      process.exit(1)
    }
    // NOTE: this proves the file was WRITTEN, nothing more -- it reads back
    // what it just wrote and compares it to itself, so it can only catch a
    // disk failure. Whether the APP accepts those records is a different
    // question and is checked after launch, in `ready`.
  }
  // Other profile files a drive wants in place BEFORE the app reads them
  // (memories.json, rooms.json, routines.json): written before launch, so
  // nothing the app writes at boot can race them.
  for (const [file, content] of Object.entries(files)) {
    await writeFile(join(profile, file), typeof content === 'string' ? content : JSON.stringify(content), 'utf8')
  }
  /*
   * The dev build by default, the PACKAGED binary when asked.
   *
   * `app.isPackaged` is false under `electron .`, so the version line, the
   * window title and the update section all read differently from what a
   * person who ran the installer sees. A drive that walks the ordinary path
   * has to walk it through the same bytes the installer lays down, or it is
   * reading a screen nobody has.
   *
   * Same profile handling, same capture machinery, same everything else --
   * only the argv differs, because the packaged exe IS the app and takes no
   * directory argument.
   */
  const launch = packaged === undefined ? [ELECTRON, [APP_DIR]] : [packaged, []]
  /*
   * FREE ROUTES ONLY, enforced by the app rather than promised by the drive.
   *
   * 2026-09-22: `drive-compact.mjs` said "spends nothing" in its header and
   * sent a turn on Cursor's Grok in its body. The app now refuses to start
   * any run that is not on a free route when this is set, so a drive that
   * picks a paid route by accident gets a refusal on screen instead of a
   * bill. Lifted only for a drive that says it spends, run with `LOCUST_SPEND=1`,
   * the same word `assertMaySpend` already asks for (spendGuarded, above) -- see
   * apps/desktop/src/main/free-routes.ts.
   */
  // LOCUST_DRIVE_PATH_FIRST: a folder found before npm's own -- the runtime canary's
  // isolated install of a new CLI release (_tools/runtime-canary.mjs).
  // Joined by the system's own separator: `;` on Windows as before, `:` elsewhere (the first Mac run).
  const path = [process.env.LOCUST_DRIVE_PATH_FIRST, NPM_DIR, process.env.PATH ?? ''].filter((part) => part !== undefined).join(delimiter)
  const appEnv = spendGuarded({ ...process.env, PATH: path, ...env }, spends)
  /*
   * Comparison copies under THIS drive's profile, not the person's ~/.locust/compare.
   *
   * Without this, every compare drive wrote cmp_* folders into the real home
   * compare root beside Colin's own (2026-10-05). The app reads LOCUST_COMPARE_ROOT
   * once; when unset it stays ~/.locust/compare. A drive that passed its own
   * LOCUST_COMPARE_ROOT in `env` keeps that.
   */
  if (appEnv.LOCUST_COMPARE_ROOT === undefined || String(appEnv.LOCUST_COMPARE_ROOT).trim() === '') {
    const driveCompare = join(profile, 'compare')
    await mkdir(driveCompare, { recursive: true })
    appEnv.LOCUST_COMPARE_ROOT = driveCompare
  }
  // Scout's fixes (0.828) likewise, under this drive's profile, not ~/.locust/scout-fixes.
  if (appEnv.LOCUST_SCOUT_FIX_ROOT === undefined || String(appEnv.LOCUST_SCOUT_FIX_ROOT).trim() === '') {
    const driveFixes = join(profile, 'scout-fixes')
    await mkdir(driveFixes, { recursive: true })
    appEnv.LOCUST_SCOUT_FIX_ROOT = driveFixes
  }
  /*
   * The person's Cursor default, held from before the launch and put back
   * after the app is gone (see cursor-default-hold.mjs). The app's own guard
   * cannot do it here: the drive ends the app before a Cursor run's process
   * ends, and deletes the profile that holds its note (0.443's drive left
   * Colin on Grok 4.6 High). An exit hook covers a drive that throws; a
   * drive killed outright is put right by the next one to start.
   */
  const cursorDefault = sendsNothing || !holdCursor
    ? { done: true, putBack: () => 'This drive does not use Cursor; its default was not read or changed.' }
    : holdCursorDefault({ say })
  process.once('exit', () => { if (!cursorDefault.done) cursorDefault.putBack() })
  // Held until `ready()` returns (or `finish`, for a drive that never asks).
  await takeLaunchTurn(name)
  /*
   * WHERE THE APP STANDS, AND WHERE IT WORKS, ARE NOT THE SAME FOLDER.
   *
   * A real launch never stands in the workspace: from the Start menu the
   * app's own folder is its install directory, and the workspace comes from
   * memory or Documents\Locust (main/workspace.ts). A drive launched IN the
   * workspace hands every child process that is started without a folder
   * the right one by accident -- so a check that work landed in the folder
   * could not fail (found 2026-09-26: OpenCode's server is started with no
   * folder of its own). `launchElsewhere` launches from the profile folder
   * and names the workspace the way the app reopens itself in one.
   */
  // A Mac drive never reads the person's keychain: a copy signed apart from the installed one makes macOS ask, and the
  // app waits on the answer with its window frozen (the first Mac run). Windows' DPAPI never asks.
  const keychain = process.platform === 'darwin' ? ['--use-mock-keychain'] : []
  const child = spawn(launch[0], [...launch[1], ...extraArgs, ...keychain, `--remote-debugging-port=${String(port)}`, `--user-data-dir=${profile}`, ...(launchElsewhere ? [`--workspace=${workspace}`] : [])], {
    cwd: launchElsewhere ? profile : workspace,
    env: appEnv,
    stdio: ['ignore', 'pipe', 'pipe']
  })
  const appOutput = []
  child.stdout.on('data', (d) => appOutput.push(String(d)))
  child.stderr.on('data', (d) => appOutput.push(String(d)))
  const record = []
  let step = stepFrom

  let page
  for (let i = 0; i < 80 && page === undefined; i += 1) {
    await sleep(500)
    if (child.exitCode !== null) {
      await giveBackLaunchTurn()
      throw new Error(`app exited ${String(child.exitCode)}\n${appOutput.join('').slice(-800)}`)
    }
    try {
      const list = await (await fetch(`http://127.0.0.1:${String(port)}/json/list`)).json()
      page = list.find((t) => t.type === 'page' && t.webSocketDebuggerUrl && !t.url.includes('#splash'))
    } catch { /* not up */ }
  }
  if (page === undefined) {
    endTree(child)
    await giveBackLaunchTurn()
    throw new Error('renderer never came up')
  }
  const socket = new WebSocket(page.webSocketDebuggerUrl)
  await new Promise((res, rej) => { socket.addEventListener('open', res, { once: true }); socket.addEventListener('error', rej, { once: true }) })
  let id = 0
  const pending = new Map()
  const consoleErrors = []
  socket.addEventListener('message', (event) => {
    const message = JSON.parse(event.data)
    if (message.method === 'Runtime.exceptionThrown') consoleErrors.push(message.params?.exceptionDetails?.text ?? 'exception')
    // Every argument, not the first: Electron's "sandboxed_renderer.bundle.js script failed to run" names the
    // preload's real error only in the second (2026-10-07, seen intermittently at launch).
    if (message.method === 'Runtime.consoleAPICalled' && message.params?.type === 'error') {
      const said = (message.params.args ?? []).map((arg) => String(arg?.value ?? arg?.description ?? '')).filter((text) => text.length > 0).join(' | ')
      consoleErrors.push(said.length > 0 ? said.slice(0, 2_000) : 'console.error')
    }
    const waiter = pending.get(message.id)
    if (waiter) { pending.delete(message.id); waiter(message) }
  })
  const send = (method, params = {}) => new Promise((resolve_) => {
    const next = ++id
    // Said, not silent: a wait longer than this inside one evaluate came back empty and looked like an answer
    // (polish-after-the-sweep's first run, 2026-10-12).
    const gaveUp = setTimeout(() => {
      if (!pending.delete(next)) return
      say(`  ${method} had no answer in 400 s; going on without one`)
      resolve_(undefined)
    }, 400_000)
    pending.set(next, (message) => { clearTimeout(gaveUp); resolve_(message) })
    socket.send(JSON.stringify({ id: next, method, params }))
  })
  const evaluate = async (expression) => {
    let message = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })
    // Right after launch the page can still be finishing its first
    // navigation, and an evaluate that lands in the old context dies with
    // "Execution context was destroyed". Seen at the first step of several
    // smokes on 2026-09-05; the app itself never reloads. One retry.
    if (/Execution context was destroyed/.test(String(message?.error?.message ?? ''))) {
      await sleep(1500)
      message = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })
    }
    const thrown = message?.result?.exceptionDetails
    if (thrown !== undefined) say(`  eval threw: ${thrown.exception?.description ?? ''}`.slice(0, 200))
    return message?.result?.result?.value
  }
  await send('Runtime.enable')

  /** One step: do it, then keep the screen's text and picture. */
  const capture = async (title, action) => {
    step += 1
    let note
    try {
      note = await action()
    } catch (error) {
      note = `threw: ${error instanceof Error ? error.message : String(error)}`
    }
    await sleep(600)
    const text = await evaluate(`document.body.innerText`)
    const shot = await send('Page.captureScreenshot', { format: 'png' })
    const file = `${String(step).padStart(2, '0')}-${title.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '')}`
    if (shot?.result?.data) await writeFile(join(out, `${file}.png`), Buffer.from(shot.result.data, 'base64'))
    // The note in full, ahead of the screen's text. SESSION.md's table cuts it
    // at 220 characters and it existed nowhere else, so a measurement whose
    // answer was in the tail cost a second run to read (2026-09-11).
    await writeFile(join(out, `${file}.txt`), `${String(note ?? '')}\n\n---\n\n${String(text ?? '')}`, 'utf8')
    const errors = consoleErrors.splice(0)
    // A COUNT of renderer errors is not actionable. This drive reported "2"
    // on a step and the two sentences existed nowhere on disk, so the only
    // way to read them was to spend another run (2026-09-11).
    if (errors.length > 0) await writeFile(join(out, `${file}.errors.txt`), errors.map(String).join('\n'), 'utf8')
    record.push({ step, title, note: note ?? '', errors })
    say(`${String(step).padStart(2, '0')}. ${title}${note ? ` -- ${String(note).slice(0, 160)}` : ''}`)
    return note
  }

  /** Wait until discovery has finished, so the first step is the real first screen. */
  const ready = async () => {
    /*
     * `focused`: the window behaves as the one a person is using (2026-10-06). A drive's window never has focus, and
     * Compare models then sometimes did not turn Compare on at all -- half the runs of every compare drive, an
     * automation artifact (with focus emulated, ten of ten engaged). Opt-in: drives about a window in the
     * background (faces holding still, the cost at rest) must stay unfocused.
     */
    if (focused) {
      await send('Emulation.setFocusEmulationEnabled', { enabled: true })
      await send('Page.bringToFront')
    }
    const settled = await evaluate(`(async () => {
      for (let i = 0; i < 240; i += 1) {
        const field = document.querySelector('form.command-dock textarea')
        // The placeholder while discovery runs reads "Looking for the AI agents on this machine..." since b2bf2c0e
        // ("Plain words", 2026-10-01, in 0.647.0): "runtime" became "AI agent" in every sentence a person reads. This
        // kept waiting for the old words, matched at once, and the drive began before the sweep had found any agent --
        // on a busy machine the composer then had no route, mode or effort control to measure.
        if (field && !/Checking local runtimes|Looking for the AI agents/.test(field.placeholder)) break
        await new Promise(r => setTimeout(r, 250))
        if (i === 239) return 'discovery never finished'
      }
      /*
       * And then until the COUNT stops moving.
       *
       * The placeholder changes when the first runtime is ready, not when the
       * sweep is done, and the composer's route is still settling behind it.
       * A probe that sent at that moment started its mission on whichever
       * runtime happened to be ready first: measured 2026-09-10, a teammate
       * routed to Codex CLI ran on OpenCode and failed with "OpenCode is not
       * ready", with the footer still reading "4 runtimes connected". The
       * drive reported it as the teammate saying nothing.
       */
      let seen = ''
      let stable = 0
      for (let i = 0; i < 240; i += 1) {
        const now = document.querySelector('.lc-connected')?.innerText ?? ''
        stable = now === seen ? stable + 1 : 0
        seen = now
        // Six quarter-seconds of no change: long enough that a sweep still
        // arriving is caught, short enough not to pad every drive.
        if (stable >= 6) return 'discovery finished; ' + seen
        await new Promise(r => setTimeout(r, 250))
      }
      return 'discovery never settled; ' + seen
    })()`)
    /*
     * And until the window is SHOWN (10/04). The main window is held behind the
     * splash until discovery is done, and a window not yet shown paints at one
     * frame a second. Everything above can be true of a window nobody can see
     * yet -- profile-four-runs and profile-first-open then measured that 1 Hz
     * as the app's own slowness (0.8-1.3 s "first clicks", two frame gaps of
     * exactly 1,001 ms, the renderer idle) and the first was reported before
     * the second explained it. Two frames 60 Hz apart say the window is out;
     * a window that never shows (a drive with no window) is let go after 15 s.
     */
    const shown = await evaluate(`(async () => {
      const from = performance.now()
      for (let i = 0; i < 60; i += 1) {
        const t0 = performance.now()
        await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)))
        if (performance.now() - t0 < 100) return 'window shown after ' + String(Math.round(performance.now() - from)) + ' ms'
        if (performance.now() - from > 15000) return 'window never came to 60 Hz in 15 s'
      }
      return 'window never came to 60 Hz'
    })()`)
    if (/never/.test(String(shown))) say(`  ${String(shown)}`)
    /*
     * And as if in front (0.611). Behind other windows every face and every
     * animation now holds (windowPresence.ts), and a drive's window often opens
     * behind whatever Colin is typing in. The page is told it is focused, so a
     * drive sees the app as a person looking at it does, wherever the window
     * landed. A drive that tests the rest sends the window's own blur, which
     * the app still hears (drive-bots-everywhere, probe-faces-rest-behind).
     */
    await send('Emulation.setFocusEmulationEnabled', { enabled: true })

    /*
     * Did the app ACCEPT the roster it was seeded with?
     *
     * The write-back check above cannot answer this: it compares the file to
     * itself. The app parses each record on load and DROPS any it cannot read
     * -- a hue outside `lime|blue|violet|clay`, a role outside its own list --
     * and says so only to the main-process console, which nothing here reads.
     *
     * So a drive seeded with three teammates could run with one, address two
     * chips that were never drawn, and report the results as though its own
     * premise had held. That cost a session an hour in 0.36.3, was written up,
     * and then cost another one today: `sky` and `amber` are not hues and
     * `Tests & Review` is not a role, so two of three teammates were dropped
     * and a twenty-minute drive measured the wrong thing.
     *
     * A drive must be able to assert its own premise. This is that assertion.
     */
    if (seed?.teammates?.length) {
      const names = seed.teammates.map((member) => member.name)
      /*
       * ASK THE APP, do not infer from a tooltip.
       *
       * This used to read every button's `title` and keep the ones starting
       * with "Message ". That worked only for as long as a teammate control
       * carried a native tooltip naming them -- and on 2026-09-15 the wide
       * sidebar's faces row dropped its `title` for a hover card, because
       * Windows was drawing "Message Wembley - Research & Briefs - Cursor
       * Agent / cursor-grok-4.6-medium - click to show only their
       * conversations" as one unbroken line.
       *
       * Every drive then failed with "the app did not accept 1 seeded
       * teammate(s)" about a roster the app had loaded perfectly. A premise
       * check that reads a tooltip is a premise check coupled to a
       * presentation detail, and it failed in the most misleading direction
       * available: it accused the product of dropping records.
       *
       * The bridge is the honest question. "Did the app accept this roster"
       * is a question about what the app LOADED, so ask it what it loaded.
       */
      const drawn = JSON.parse(await evaluate(`(async () => {
        const roster = await window.desktop.listTeammates()
        return JSON.stringify(roster?.ok === true ? roster.data.teammates.map((t) => t.name) : [])
      })()`))
      const missing = names.filter((name) => !drawn.includes(name))
      if (missing.length > 0) {
        say(`the app did not accept ${String(missing.length)} seeded teammate(s): ${missing.join(', ')}`)
        say('a record is dropped when its hue or role is not one the roster knows.')
        say('(this now asks the app what it loaded, so a missing name is the app dropping it.)')
        say(`hues: lime, blue, violet, clay -- roles: Code & Migrations, Research & Briefs, Ops & Scheduling, Docs & QA, Data & Reporting, Custom`)
        process.exit(1)
      }
    }

    /*
     * A DRIVE THAT DECLARED IT DOES NOT SPEND MUST NOT START ON A PAID ROUTE.
     *
     * `assertMaySpend` guards the drives that say `spends: true`. Nothing
     * guarded the other direction, and the gap is not theoretical: the
     * fold-opens-file drive on 2026-09-20 declared `spends: false`, seeded its
     * teammate with the free OpenCode route, and sent its one turn on **Codex
     * CLI / Account Default** -- Astra's quota -- because a teammate's seeded
     * route is not the COMPOSER's route on a new conversation, and nothing
     * ever compared the two. It passed every check it made. The route it ran
     * on was visible only in a screenshot.
     *
     * Seven of the twenty-six sending drives never picked a route at all, so
     * this was every one of their turns, not one accident.
     *
     * The repair belongs here rather than in each drive, because here is the
     * one function all of them call before anything can be sent. Not a
     * warning: a warning is a line in a log nobody reads while the quota is
     * already gone.
     */
    if (spends !== true) {
      /*
       * A SCREEN THAT CANNOT SEND AT ALL IS NOTHING TO GUARD -- the bare
       * machine, a first launch with nothing connected. There is no route,
       * so there is nothing that could spend.
       *
       * `control.disabled` was the whole test for that, and it is the wrong
       * signal. The chip stays ENABLED with nothing installed, deliberately
       * -- Composer.tsx: "the picker still opens, because that is where a
       * person goes to see what could be installed" -- and it reads "No
       * runtime" instead of naming one.
       *
       * So the first drive ever to reach a genuinely bare machine
       * (2026-09-21, once the LOCUST_HIDE_RUNTIMES seam was made to cover
       * Antigravity) was stopped by the guard written to exempt it, saying
       * the composer was "on" a route called No runtime.
       */
      const routeText = () => evaluate(`(() => {
        const control = [...document.querySelectorAll('.lc-control')].find(b => b.getAttribute('aria-haspopup') === 'listbox')
        if (!control || control.disabled) return ''
        const text = control.innerText.replace(/\\s+/g, ' ').trim()
        // Since 0.836 the chip shows the agent's mark, not its name; its title still names the exact route.
        return /^no (runtime|ai agent)$/i.test(text) ? '' : (text + ' ' + (control.title ?? '')).replace(/\\s+/g, ' ').trim()
      })()`)
      /*
       * The word, not the model id. The picker reads as NAMES since 0.196.0,
       * so the chip that used to say `opencode/muse-spark-1.3-contributor-free`
       * now says `Jev 1.13 Free` -- and a check written against the id read
       * the free route the app had just selected as a paid one, then exited.
       * Measured the first time this guard ran.
       */
      const isFree = (text) => /opencode/i.test(String(text)) && /\bfree\b/i.test(String(text))
      let route = await routeText()
      // A drive that sends nothing has no route to protect -- and on a
      // machine made to look bare there may be no free route to move to.
      // The app refuses a paid run in any scripted window regardless.
      if (!sendsNothing && route !== '' && !isFree(route)) {
        say(`this drive does not spend, and the composer is on "${String(route)}". Moving it to the free route.`)
        /*
         * Retried, because one pick waits 30 seconds for the free row and a
         * busy machine can take longer to list it: four drives started
         * together each ran OpenCode's model discovery at once and all four
         * gave up here, while each passed alone (Yurt's beta report,
         * 2026-09-23). Up to four picks, five seconds apart.
         */
        for (let attempt = 0; attempt < 4 && !isFree(route); attempt += 1) {
          // Its own name first; "free" on alternate tries, for a model the picker names otherwise.
          const said = await evaluate(pickRouteScript({ group: '/opencode/i', search: attempt % 2 === 0 ? FREE_SEARCH : 'free', row: FREE_ROW }))
          route = await routeText()
          if (!isFree(route)) say(`the free route is not there yet (${String(said).slice(0, 1500)})${attempt < 3 ? '; trying again' : ''}`)
          if (!isFree(route) && attempt < 3) await sleep(5000)
        }
        if (!isFree(route)) {
          say(`could not put a non-spending drive on a free route; it is on "${String(route)}".`)
          say('pass spends: true if this drive is meant to spend, or pick a free route before sending.')
          process.exit(1)
        }
      }
      reportRouteProbe(route)
    }
    // Discovered and on its route: the next drive may start.
    await giveBackLaunchTurn()
    return settled
  }

  /** Write SESSION.md with the step table and close the app. */
  const finish = async ({ intro, extra = '', last = true }) => {
    // A drive that never called `ready()` still gives its turn back.
    await giveBackLaunchTurn()
    const rows = record.map((r) => `| ${String(r.step)} | ${r.title} | ${String(r.note).replace(/\|/g, '/').replace(/\s+/g, ' ').slice(0, 220)} | ${r.errors.length === 0 ? '0' : `${String(r.errors.length)} — ${String(r.errors[0]).replace(/\|/g, '/').replace(/\s+/g, ' ').slice(0, 120)}`} |`)
    const session = join(out, 'SESSION.md')
    const { readFile } = await import('node:fs/promises')
    const existing = await readFile(session, 'utf8').catch(() => undefined)
    const table = ['| # | Step | What was seen | Renderer errors |', '|---|---|---|---|', ...rows]
    const tail = last ? ['', extra, '## Judgement', '', '(filled in after reading the captures)', ''] : ['']
    const body = existing === undefined
      ? [`# User session ${stamp} — ${name}`, '', intro, '', ...table, ...tail]
      : [existing.replace(/\n## Judgement[\s\S]*$/, '\n'), `## Continued: ${intro}`, '', ...table, ...tail]
    await writeFile(session, body.join('\n'), 'utf8')
    /*
     * What the MAIN process said, kept beside the screenshots.
     *
     * A drive could read the renderer and the disk but never the host's own
     * words, so a relay that refused to start a run was invisible: the screen
     * showed a room with two answers and no argument, and the only place the
     * reason existed was a stream nobody wrote down (2026-09-11).
     */
    await writeFile(join(out, 'main.log'), appOutput.join(''), 'utf8').catch(() => undefined)
    try { socket.close() } catch { /* gone */ }
    endTree(child)
    await sleep(1500)
    if (last) {
      const line = cursorDefault.putBack()
      await writeFile(session, `${(await readFile(session, 'utf8')).replace(/\n*$/, '\n')}\n${line}\n`, 'utf8').catch(() => undefined)
    }
    // LOCUST_DRIVE_KEEP=1 keeps any drive's profile, to read its ledger after.
    const kept = keep || process.env.LOCUST_DRIVE_KEEP === '1'
    if (!kept && last) await rm(profile, { recursive: true, force: true }).catch(() => undefined)
    if (last && kept) say(`profile kept: ${profile}`)
    if (last) say(`\nrecord: ${out}`)
    return { out, profile, step }
  }

  /**
   * Resize the window, so a claim about how the layout answers the window can
   * be measured at more than one size rather than asserted at one.
   *
   * `Emulation.setDeviceMetricsOverride` is what CDP gives a page; the OS
   * window around it does not move, which is fine -- everything measured here
   * is inside the page.
   */
  const resize = async (width, height) => {
    await send('Emulation.setDeviceMetricsOverride', {
      width,
      height,
      deviceScaleFactor: 1,
      mobile: false
    })
    return { width, height }
  }

  /**
   * Wait until the screen says something is true, instead of guessing how
   * long it takes.
   *
   * Every drive in this repository used to wait with a fixed `sleep`. That is
   * wrong in both directions at once: too short and the step measures a
   * screen that had not arrived yet, too long and it pads the wall clock of
   * every run that was already fine. Both were happening -- the flakes that
   * cost 2026-09-09 and 2026-09-15 were a selector arriving late, and the
   * sleeps together are most of the time a full pass takes.
   *
   * `expression` is evaluated IN THE PAGE and is polled from here rather than
   * looped in the page on purpose: a one-shot evaluate can be retried when
   * the execution context is destroyed mid-navigation, and a loop running
   * inside that context cannot.
   *
   * Returns the first truthy value. On timeout it THROWS, naming what was
   * awaited and what the expression last returned -- a wait that quietly
   * gives up is the fixed sleep again, wearing a better name.
   */
  const waitFor = async (expression, { timeoutMs = 20_000, everyMs = 150, what } = {}) => {
    const deadline = Date.now() + timeoutMs
    let last
    for (;;) {
      last = await evaluate(`(() => { try { return (${expression}) } catch (error) { return 'threw: ' + String(error && error.message) } })()`)
      if (last !== undefined && last !== null && last !== false && last !== '' && last !== 0) return last
      if (Date.now() >= deadline) {
        throw new Error(`waited ${String(timeoutMs)}ms for ${what ?? expression} -- last value ${JSON.stringify(last) ?? 'undefined'}`)
      }
      await sleep(everyMs)
    }
  }

  /**
   * The common case: wait until a selector matches, and hand back its text.
   * Text rather than `true` so the step's note says what actually arrived.
   */
  const waitForSelector = async (selector, options = {}) => waitFor(
    `(() => { const el = document.querySelector(${JSON.stringify(selector)}); return el ? (el.innerText || el.textContent || 'present') : false })()`,
    { what: selector, ...options }
  )

  /*
   * The OS process id, for the one thing CDP cannot do: move and photograph
   * the real window. `resize` above changes the PAGE and leaves the window
   * where it is, which is fine for anything measured inside the page and is
   * NOT a native-window sizing pass. A design review counting pixels needs
   * the window a person actually has.
   */
  return { evaluate, send, capture, ready, finish, profile, out, record, resize, waitFor, waitForSelector, pid: child.pid }
}

/**
 * A teammate's face in the sidebar rail, as a page-script expression: the ONE
 * way a harness finds a teammate to open.
 *
 * 108 drives found the teammate by a button titled "Message <name>". The rail
 * became faces (2026-09-21, Colin: "we can just make those clickable") and
 * the face has no title -- its hover card says it -- so every one of those
 * lookups found nothing, and the ones written `?.click()` went on in an
 * unassigned conversation without a word (Yurt's beta report, 2026-09-23:
 * "most drives die on open the teammate"). The face's label is
 * "<name> — open their conversation" (Sidebar.tsx), and clicking it opens the
 * teammate's hub, or selects them when they have none yet -- what the old
 * button did. The next change to the rail is a change here.
 */
export function teammateFace(name) {
  return `[...document.querySelectorAll('.lc-faces__one')].find((face) => (face.getAttribute('aria-label') ?? '').startsWith(${JSON.stringify(`${name} — `)}))`
}

/**
 * The team as the sidebar draws it, as a page-script expression: one entry per
 * face, `{ face, id, name, activity, innerText, conversations, conversation,
 * click(), querySelector() }`. `conversations` are the sidebar's rows wearing
 * this teammate's face, newest first; `conversation` is the newest -- what the
 * compact rows nested as `.lc-teammate__mission`.
 *
 * Drives read `.lc-teammate` rows for a teammate's name and state ("Wren ·
 * working"). Those rows are the COMPACT sidebar's; the ordinary one draws
 * faces and nothing else, so every such read found no rows -- 57 of them
 * across 22 harnesses, and drive-routine's "nothing seen to start" while the
 * routine ran (Yurt's beta report, 2026-09-23). A face's state is its bot's
 * `data-activity` (TeammateBot.tsx), worded as `faceLabel` words it
 * (faceState.ts), and `innerText` is "<name>\n<state>" so a read written
 * against a row still means the same thing.
 */
export function teammateRows() {
  return `[...document.querySelectorAll('.lc-faces__one')].map((face) => {
    const name = (face.getAttribute('aria-label') ?? '').split(' — ')[0]
    const id = face.querySelector('[data-teammate]')?.getAttribute('data-teammate') ?? ''
    const state = face.querySelector('[data-activity]')?.getAttribute('data-activity') ?? 'idle'
    const activity = ({ thinking: 'thinking', working: 'working', delegating: 'subagent working', responding: 'replying', waiting: 'waiting on you', receiving: 'listening', blocked: 'blocked', done: 'done', idle: 'idle' })[state] ?? state
    const conversations = [...document.querySelectorAll('.lc-conv')].filter((row) => id !== '' && row.querySelector('[data-teammate]')?.getAttribute('data-teammate') === id)
    return { face, id, name, activity, innerText: name + String.fromCharCode(10) + activity, conversations, conversation: conversations[0], click: () => face.click(), querySelector: (selector) => face.querySelector(selector) }
  })`
}

/**
 * The conversations in the sidebar, as a page-script expression: one entry per
 * row, `{ row, title, owner, active, running, click() }`. `owner` is the
 * teammate id on the row's face (`data-teammate`), or '' for nobody's.
 *
 * What the compact sidebar nests under each teammate (`.lc-teammate__mission`),
 * the ordinary one lists once, each row wearing its owner's face.
 */
export function conversationRows() {
  return `[...document.querySelectorAll('.lc-conv')].map((row) => ({
    row,
    title: row.getAttribute('title') ?? '',
    owner: row.querySelector('[data-teammate]')?.getAttribute('data-teammate') ?? '',
    active: row.getAttribute('aria-current') === 'true',
    running: row.querySelector('[data-orb]') !== null,
    click: () => row.click()
  }))`
}

/**
 * Open a teammate's conversation and wait until the composer is theirs
 * ("Message <name>…"). Says what went wrong otherwise, rather than carrying on.
 *
 * The rail draws every face up to five teammates, and four and a `+N` from
 * six (Sidebar.tsx, newest first), so past those it goes the way a person
 * would: the Team screen, and the card's Message button.
 */
export function openTeammateScript(name) {
  return `(async () => {
    const face = ${teammateFace(name)}
    if (face) {
      face.click()
    } else {
      // The Team button toggles: from the Team screen it would close it.
      if (!document.querySelector('.lc-rostergrid')) document.querySelector('.lc-faces__team')?.click()
      await new Promise((r) => setTimeout(r, 500))
      const card = [...document.querySelectorAll('.lc-rostercard')].find((one) => one.querySelector('.lc-rostercard__name')?.textContent.trim() === ${JSON.stringify(name)})
      const message = card?.querySelector('.lc-rostercard__message')
      if (!message) return 'no face for ' + ${JSON.stringify(name)} + ' in the rail and no card on the Team screen; faces: ' + [...document.querySelectorAll('.lc-faces__one')].map((one) => one.getAttribute('aria-label')).join(' | ')
      message.click()
    }
    for (let attempt = 0; attempt < 20; attempt += 1) {
      await new Promise((r) => setTimeout(r, 150))
      const said = document.querySelector('form.command-dock textarea')?.getAttribute('placeholder') ?? ''
      if (said.startsWith(${JSON.stringify(`Message ${name}`)})) return 'opened ' + ${JSON.stringify(name)}
    }
    return 'clicked ' + ${JSON.stringify(name)} + ' but the composer says: ' + (document.querySelector('form.command-dock textarea')?.getAttribute('placeholder') ?? 'no composer')
  })()`
}

/**
 * Pick a route from the composer's picker the way a person does: open it,
 * type a search, click the first enabled row under the runtime's group.
 * Returns what the controls read afterwards, or why nothing was picked.
 */
export function pickRouteScript({ group, search, row }) {
  return `(async () => {
    const control = [...document.querySelectorAll('.lc-control')].find(b => b.getAttribute('aria-haspopup') === 'listbox')
    if (!control) return 'no route control'
    if (control.disabled) return 'route control disabled'
    /*
     * Open it only if it is CLOSED. This used to click unconditionally, which
     * on an already-open picker closes it -- and the selection then does not
     * apply, while the helper still returns its success line naming whatever
     * route was there before.
     *
     * Measured 2026-09-09, both builds: called as the first thing to touch the
     * picker it moves the chip Codex -> OpenCode -> Cursor first try; called
     * after anything else had opened the picker, it reported success and left
     * the chip untouched. That difference cost two walkthroughs and a false
     * alarm about sixty drives being in doubt -- they were not, because they
     * all happen to call this first.
     *
     * One line, so no drive has to know that rule.
     */
    if (!document.querySelector('.lc-picker')) control.click()
    let target
    let notice = null
    for (let attempt = 0; attempt < 60 && !target; attempt += 1) {
      const picker = document.querySelector('.lc-picker')
      if (!picker) { control.click(); await new Promise(r => setTimeout(r, 500)); continue }
      notice = picker.querySelector('.lc-picker__notice')?.innerText ?? null
      const box = picker.querySelector('.lc-picker__input')
      // Typed once, then the list is watched. Typed on every attempt, a
      // renderer too busy to filter within the half second between them was
      // re-filtered forever and never showed a row (four apps at once,
      // 2026-09-23).
      if (box && ${JSON.stringify(search ?? '')} && box.value !== ${JSON.stringify(search ?? '')}) {
        const setInput = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set
        setInput.call(box, ${JSON.stringify(search ?? '')})
        box.dispatchEvent(new Event('input', { bubbles: true }))
        await new Promise(r => setTimeout(r, 500))
      }
      let current = ''
      for (const node of picker.querySelector('.lc-picker__list').children) {
        const header = node.querySelector('.lc-picker__group')
        if (header) current = header.innerText
        const candidate = node.querySelector('.lc-picker__row')
        if (candidate && !candidate.disabled && ${group}.test(current) && ${row ?? '/./'}.test(candidate.innerText)) { target = candidate; break }
      }
      if (!target) await new Promise(r => setTimeout(r, 500))
    }
    // The whole list as it stood, headers and disabled rows marked: six rows did not say why a pick found nothing.
    if (!target) {
      const entries = [...(document.querySelector('.lc-picker__list')?.children ?? [])].map((node) => {
        const header = node.querySelector('.lc-picker__group')
        const r = node.querySelector('.lc-picker__row')
        return [
          header ? '[' + header.innerText.replace(/\\s+/g, ' ').trim() + ']' : '',
          r ? (r.disabled ? '(disabled) ' : '') + r.innerText.replace(/\\s+/g, ' ').trim().slice(0, 50) : ''
        ].filter(Boolean).join(' ')
      }).filter(Boolean)
      const typed = document.querySelector('.lc-picker__input')?.value
      return 'no matching route; typed ' + JSON.stringify(typed ?? null) + (notice ? '; notice: ' + notice : '') + '; ' + String(entries.length) + ' entries: ' + entries.join(' | ')
    }
    target.click()
    await new Promise(r => setTimeout(r, 500))
    return (notice ? 'picker said: ' + notice + ' || ' : '') + [...document.querySelectorAll('.lc-control')].map(c => c.innerText.replace(/\\s+/g, ' ').trim()).filter(Boolean).join(' · ')
  })()`
}

/** Type a message, press send, and wait for the run to end (or not, within the bound). */
/**
 * Open every step line in the thread, so each step's row is on screen (0.491+).
 *
 * A turn's steps used to sit in ONE fold, `.lc-activity`, and drives opened it
 * to read the rows. Since 0.491 a turn is what was said and, between, one
 * line per group of steps (`.lc-steps__line`); `.lc-activity` is now only the
 * turn's files card at its foot. A drive that reads rows opens these first.
 * Resolves to how many rows are then on screen.
 */
export function openAllStepsScript(scope = '.lc-thread') {
  return `(async () => {
    for (const line of document.querySelectorAll(${JSON.stringify(`${scope} .lc-steps__line[aria-expanded="false"]`)})) line.click()
    await new Promise((r) => setTimeout(r, 400))
    return document.querySelectorAll(${JSON.stringify(`${scope} .lc-steps__list .lc-filerow`)}).length
  })()`
}

// One evaluate has no answer after 400 s (startDrive's send), so a waitSeconds past about 390 comes back empty:
// wait longer from the drive itself, a short look at a time (polish-after-the-sweep).
export function sendAndWaitScript(text, { waitSeconds = 360, settle = true } = {}) {
  return `(async () => {
    const field = document.querySelector('form.command-dock textarea')
    const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
    setter.call(field, ${JSON.stringify(text)})
    field.dispatchEvent(new Event('input', { bubbles: true }))
    let sent = false
    for (let i = 0; i < 120 && !sent; i += 1) {
      await new Promise(r => setTimeout(r, 250))
      const button = document.querySelector('button[aria-label="Send"]')
      if (button && !button.disabled) { button.click(); sent = true }
    }
    if (!sent) return 'no send'
    if (!${settle ? 'true' : 'false'}) return 'sent'
    for (let i = 0; i < ${String(waitSeconds * 2)}; i += 1) {
      await new Promise(r => setTimeout(r, 500))
      if (i > 4 && !document.querySelector('button[aria-label^="Stop the running"]')) break
    }
    await new Promise(r => setTimeout(r, 800))
    return 'finished: ' + (document.querySelector('.lc-thread')?.innerText.replace(/\\s+/g, ' ').slice(-300) ?? '')
  })()`
}

/**
 * COMPARE, the way a person sets it up (2026-10-06): the chat mode chip, then Compare. It opens a picker to tick
 * models ("Pick two or three models"), or -- with models already in the box -- goes straight to a chip per column.
 * Resolves to JSON `{ how: 'picker' | 'columns' | <why not> }`. Lifted from drive-compare-answers, the one compare
 * drive whose picking still worked once the picker named rows by display name and drew only the rows in view.
 */
export function enterCompareScript() {
  return `(async () => {
    const chip = document.querySelector('.lc-control--chatmode')
    if (!chip) return JSON.stringify({ how: 'no chat mode chip' })
    chip.click()
    await new Promise((r) => setTimeout(r, 400))
    const item = [...document.querySelectorAll('[role="menu"][aria-label="Direct or compare"] [role="menuitemradio"], [role="menu"][aria-label="Direct or compare"] button')].find((b) => /^Compare/.test(b.innerText.trim()))
    if (!item) return JSON.stringify({ how: 'no Compare item' })
    item.click()
    for (let i = 0; i < 40; i += 1) {
      await new Promise((r) => setTimeout(r, 150))
      if (document.querySelector('.lc-picker__input')) return JSON.stringify({ how: 'picker' })
      if (document.querySelector('.lc-slotgroup')) return JSON.stringify({ how: 'columns' })
    }
    return JSON.stringify({ how: 'neither a picker nor columns' })
  })()`
}

/**
 * One model for column `index`: in the open picker, or through that column's own chip. `pick` is `{ search, row }`:
 * what is typed into the search, and a regular expression over the row's label. Resolves to JSON
 * `{ picked, label?, why? }`. Use `comparePick('nemotron-3-ultra-free')` to make a pick from a free model's id.
 */
export function comparePickScript(index, pick) {
  return `(async () => {
    if (document.querySelector('.lc-picker__input') === null) {
      // That column's chip, or, past the columns there are, "+" -- as a person adds the next model.
      const chip = document.querySelectorAll('.lc-slotgroup')[${String(index)}]?.querySelector('.lc-control--slot')
        ?? document.querySelector('.lc-control--addslot')
      if (!chip) return JSON.stringify({ picked: false, why: 'no column chip and no add button' })
      chip.click()
      for (let i = 0; i < 20 && !document.querySelector('.lc-picker__input'); i += 1) await new Promise((r) => setTimeout(r, 150))
    }
    const box = document.querySelector('.lc-picker__input')
    if (!box) return JSON.stringify({ picked: false, why: 'no search box' })
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(box, ${JSON.stringify(pick.search)})
    box.dispatchEvent(new Event('input', { bubbles: true }))
    const wanted = new RegExp(${JSON.stringify(pick.row)}, 'i')
    let row
    for (let i = 0; i < 20 && !row; i += 1) {
      await new Promise((r) => setTimeout(r, 200))
      const rows = [...document.querySelectorAll('.lc-picker__row:not(.is-recent)')].filter((one) => !one.disabled && wanted.test(one.querySelector('.lc-picker__label')?.textContent ?? ''))
      // A free pick takes the row that says Free: another agent may list the same model by name, paid.
      row = (${JSON.stringify(pick.free === true)} ? rows.find((one) => /\\bfree\\b/i.test(one.querySelector('.lc-picker__label')?.textContent ?? '')) : undefined) ?? rows[0]
    }
    if (!row) return JSON.stringify({ picked: false, why: 'no row' })
    const label = row.querySelector('.lc-picker__label')?.textContent.trim() ?? ''
    if (row.getAttribute('aria-pressed') !== 'true') row.click()
    await new Promise((r) => setTimeout(r, 500))
    return JSON.stringify({ picked: true, label })
  })()`
}

/** A free OpenCode model's id as a pick: searched by its first word, matched by its words in order ("Mimo V2.6 Flash"). */
export function comparePick(id) {
  const words = id.replace(/^opencode\//, '').replace(/-free$/, '').split('-')
  return { search: words[0], row: '^' + words.map((word) => word.replace(/[.*+?^$()|[\]\\{}]/g, (c) => '\\' + c)).join('.*'), free: /-free$/.test(id) }
}

/** Done in the picker, if it is open, and the columns' chips as they read after. Resolves to JSON `{ chips }`. */
export function compareDoneScript() {
  return `(async () => {
    const done = [...document.querySelectorAll('.lc-picker__foot--compare button')].find((b) => b.textContent.trim() === 'Done')
    if (done && !done.disabled) done.click()
    for (let i = 0; i < 20 && !document.querySelector('.lc-slotgroup'); i += 1) await new Promise((r) => setTimeout(r, 150))
    await new Promise((r) => setTimeout(r, 400))
    return JSON.stringify({ chips: [...document.querySelectorAll('.lc-slotgroup .lc-control--slot')].map((c) => c.innerText.replace(/\\s+/g, ' ').trim()) })
  })()`
}
