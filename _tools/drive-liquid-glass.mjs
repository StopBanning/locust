// Liquid glass (0.855): the Mac window as macOS composites it, over a known desktop.
//
//   node _tools/drive-liquid-glass.mjs [--tag <name>] [--desk bright|dark-desktop|rainbow]
//
// Glass shows what is behind it, and a drive's page capture shows none of it, so each frame is macOS's own capture of
// this one window (screencapture -l): never another app's window, and nothing is brought to the front -- the window
// is captured wherever it is, behind whatever the person is using. What is behind the glass is a picture the
// development build puts under it inside the window (LOCUST_GLASS_BACKDROP, main/mac-glass.ts): a bright macOS
// wallpaper in one run, near-black, as a dark desktop is, in another. In each: Liquid glass on, Home, a conversation,
// a reply in it, the palette, Settings, the light theme, then back to Standard. One message, on the free model, in the
// bright run only: the thread's bubbles and cards have to be seen on the glass.
//
// A development build only: a packaged build takes no backdrop.
import { execFileSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { mkdir, mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { FREE_ROUTE, recordRoot, say, scratchRepository, sendAndWaitScript, sleep, startDrive, teammateFace } from './drive-lib.mjs'

if (process.platform !== 'darwin') {
  say('Liquid glass is a Mac look: this drive runs on a Mac.')
  process.exit(1)
}
const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const tag = arg('--tag')
// --desk <name>: one pass only (bright, dark-desktop or rainbow).
const onlyDesk = arg('--desk')

const pictures = await mkdtemp(join(tmpdir(), 'locust-glass-backdrop-'))
const bright = join(pictures, 'sonoma.jpg')
const wallpaper = '/System/Library/Desktop Pictures/Sonoma.heic'
if (existsSync(wallpaper)) execFileSync('sips', ['-s', 'format', 'jpeg', wallpaper, '--out', bright], { stdio: 'ignore' })

/** This drive's window: the largest ordinary window of the app's own process. */
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

const verdicts = []
const judge = (name, pass, seen) => {
  verdicts.push(`${name}: ${pass ? 'PASS' : 'FAIL'} (${seen})`)
  say(`${pass ? '[PASS]' : '[FAIL]'} ${name} -- ${seen}`)
}

const at = '2026-11-02T09:00:00.000Z'
const seed = {
  schemaVersion: 1,
  teammates: [
    { teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: at, route: FREE_ROUTE },
    { teammateId: 'tm_atlas', name: 'Atlas', hue: 'blue', role: 'Research & Briefs', createdAt: at, route: FREE_ROUTE },
    { teammateId: 'tm_sable', name: 'Sable', hue: 'violet', role: 'Docs & QA', createdAt: at, route: FREE_ROUTE }
  ],
  missionOwners: {},
  settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false }
}

// Rainbow (0.859): no drive picture -- Locust's own Rainbow fills the window under the glass, so the capture shows only
// Locust. It is chosen before the glass goes on and taken off only after the last frame, so no frame shows the desktop.
for (const [desk, picture, port] of [['bright', existsSync(bright) ? bright : 'dark', 9571], ['dark-desktop', 'dark', 9573], ['rainbow', null, 9575]]) {
  if (onlyDesk !== undefined && onlyDesk !== desk) continue
  const outPath = tag === undefined ? undefined : join(recordRoot('beta-fixes-2026-09-24'), `liquid-glass-${tag}-${desk}`)
  if (outPath !== undefined) await mkdir(outPath, { recursive: true })
  const drive = await startDrive({
    name: `liquid-glass-${desk}`,
    port,
    workspace: await scratchRepository('locust-drive-liquid-glass-ws-'),
    sendsNothing: desk !== 'bright',
    env: picture === null ? {} : { LOCUST_GLASS_BACKDROP: picture },
    seed,
    ...(outPath === undefined ? {} : { outPath })
  })
  const { evaluate } = drive
  const snap = async (name) => {
    await sleep(700)
    const file = join(drive.out, `${name}.png`)
    execFileSync('screencapture', ['-x', '-o', `-l${String(windowOf(drive.pid))}`, file])
    say(`  frame ${name}`)
  }
  const click = (script) => evaluate(`(() => { const el = ${script}; if (!el) return false; el.click(); return true })()`)
  const startsWith = (selector, text) =>
    `[...document.querySelectorAll(${JSON.stringify(selector)})].find((el) => el.innerText.replace(/\\s+/g, ' ').trim().startsWith(${JSON.stringify(text)}))`
  const key = (code) => evaluate(`window.dispatchEvent(new KeyboardEvent('keydown', { key: ${JSON.stringify(code)}, ctrlKey: true, bubbles: true })), true`)
  const home = async () => { await click(`document.querySelector('[aria-label="Home"]')`); await sleep(1200) }
  const conversation = async () => { await click(teammateFace('Wren')); await sleep(1200) }
  const settings = async () => {
    await key('3')
    await sleep(900)
    await click(startsWith('.lc-settings__navitem', 'Appearance'))
    await sleep(900)
  }
  // An agent's usage card, held open over what is under it (0.859): frosted, so nothing under it reads through.
  const agentCard = async (name) => {
    const seen = String(await evaluate(`(() => { const c = document.querySelector('.lc-agentcard'); if (!c) return 'none'; c.style.display = 'grid'; return getComputedStyle(c).backdropFilter })()`))
    if (seen === 'none') return say(`  ${name}: no agent card on this screen`)
    judge(`${name}: the agent card is frosted`, /blur/.test(seen), seen)
    await snap(name)
    await evaluate(`(() => { const c = document.querySelector('.lc-agentcard'); if (c) c.style.display = ''; return true })()`)
  }
  const look = async (choice) => String(await evaluate(`window.desktop.setLook(${JSON.stringify(choice)}).then((s) => s.look + ' offered ' + s.liquidOffered)`))
  const theme = async (choice) => String(await evaluate(`window.desktop.setTheme(${JSON.stringify(choice)}).then((s) => s.resolved)`))
  const drawn = async () => String(await evaluate(`document.documentElement.dataset.look ?? 'standard'`))
  try {
    say(String(await drive.ready()))
    await sleep(800)
    await home()
    if (desk === 'rainbow') {
      const ground = String(await evaluate(`window.desktop.setGlassGround('rainbow').then((s) => s.glassGround)`))
      judge('rainbow: Behind the glass is Rainbow before the glass goes on', ground === 'rainbow', ground)
    }
    const set = await look('liquid')
    await sleep(800)
    judge(`${desk}: Liquid glass is offered and drawn`, /liquid offered true/.test(set) && (await drawn()) === 'liquid', `${set}; page ${await drawn()}`)
    await snap(`${desk}-1-liquid-home`)
    await agentCard(`${desk}-1b-liquid-agentcard`)
    await conversation()
    await snap(`${desk}-2-liquid-conversation`)
    if (desk === 'bright') {
      say(`  ${String(await evaluate(sendAndWaitScript('In two short paragraphs, say what a git worktree is and when one helps. Then a three-item bullet list of commands. Do not read or change any files.', { waitSeconds: 240 }))).slice(0, 160)}`)
      await snap(`${desk}-2b-liquid-reply`)
      await agentCard(`${desk}-2d-liquid-agentcard`)
    }
    await key('k')
    await sleep(700)
    await snap(`${desk}-2c-liquid-palette`)
    await evaluate(`(() => { for (const t of [document.activeElement ?? document.body, document, window]) t.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); return true })()`)
    await sleep(400)
    await settings()
    if (desk === 'rainbow') {
      await click(startsWith('.lc-settings__navitem', 'Appearance'))
      await sleep(500)
      const row = String(await evaluate(`[...document.querySelectorAll('.lc-settingline--sub .lc-settings__heading')].map((h) => h.textContent).join('|')`))
      judge('rainbow: Appearance shows Behind the glass under Look', row.includes('Behind the glass'), row)
    }
    await snap(`${desk}-3-liquid-settings`)
    await theme('light')
    await home()
    await snap(`${desk}-4-liquid-light-home`)
    await conversation()
    await snap(`${desk}-5-liquid-light-conversation`)
    await theme('dark')
    // The compact rail, chosen in Settings: the panel narrows with the sidebar, and the traffic lights stay inside it.
    await settings()
    await click(startsWith('.lc-segmented .lc-button', 'Rail'))
    await sleep(1000)
    say(`  rail: ${String(await evaluate(`!!document.querySelector('.lc-shell.is-compact')`))}`)
    await home()
    await snap(`${desk}-6-liquid-rail`)
    await settings()
    await click(startsWith('.lc-segmented .lc-button', 'Auto'))
    if (desk === 'rainbow') {
      // Off the glass first, so taking the Rainbow away never shows the desktop to a capture.
      await look('standard')
      await evaluate(`window.desktop.setGlassGround('desktop')`)
    }
    await look('standard')
    await home()
    judge(`${desk}: back to Standard`, (await drawn()) === 'standard', await drawn())
  } catch (error) {
    say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
  } finally {
    await drive.finish({
      intro: `Build: whatever pnpm build last wrote to out/. Liquid glass over ${desk === 'bright' ? 'a bright macOS wallpaper' : 'a near-black desktop'} put under the glass; macOS's capture of this window alone.`,
      extra: verdicts.length === 0 ? '' : ['## Verdicts', '', ...verdicts.map((line) => `- ${line}`)].join('\n')
    })
  }
}
