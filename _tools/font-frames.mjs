// Every screen, photographed twice: in the face the app ships now, and in the
// one Colin picked. Frames for him to judge BEFORE the change goes out.
//
//   node _tools/font-frames.mjs --label after
//       builds a conversation on the free OpenCode route, walks every screen
//       on out/, and keeps the profile and folder for the second run
//   node _tools/font-frames.mjs --label before --packaged <Locust.exe> --reuse <dir>
//       the same profile and folder on a build without the change, so both
//       sets show the same conversation, the same teammates, the same memory
//
// Colin, 2026-09-22, choosing direction B from the type trials: "Alright
// I'll run with your font suggestion". The trials were drawn from the app's
// screens; these ARE the app's screens, which is the difference that matters
// before anything ships.
//
// Spends nothing: the one message goes to the free OpenCode route, checked on
// the chip before it is sent, and the second run sends nothing at all.

import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { localPath, FREE_ROUTE, pickRouteScript, say, scratchRepository, sendAndWaitScript, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const label = arg('--label')
if (label !== 'before' && label !== 'after') throw new Error('--label before|after')
const packaged = arg('--packaged')
const reuse = arg('--reuse')

const OUT = join(localPath(new URL('../docs/font-frames-2026-09-22/', import.meta.url)), label)
await mkdir(OUT, { recursive: true })

const T0 = '2026-09-20T15:00:00.000Z'
let workspace
let profile
if (reuse === undefined) {
  workspace = await scratchRepository('locust-font-frames-ws-')
  // Something real to read, so the reply has content worth setting in type.
  await writeFile(
    join(workspace, 'README.md'),
    "# Harbor\n\nA small tide-table app: it reads a station's predictions and draws the next two days as a curve, with the next high and low called out.\n",
    'utf8'
  )
  profile = await mkdtemp(join(tmpdir(), 'locust-font-frames-'))
  // The second run finds the folder through the profile it is handed.
  await writeFile(join(profile, 'font-frames.json'), JSON.stringify({ workspace, profile }), 'utf8')
} else {
  ;({ workspace, profile } = JSON.parse(await readFile(join(reuse, 'font-frames.json'), 'utf8')))
}
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`

// The same size every frame is taken at, so a pair differs only in the face.
await writeFile(join(profile, 'window.json'), JSON.stringify({ x: 0, y: 0, width: 1477, height: 920, maximized: false }), 'utf8')

const drive = await startDrive({
  name: `font-frames-${label}`,
  port: label === 'after' ? 9351 : 9352,
  workspace,
  profilePath: profile,
  keep: true,
  sendsNothing: reuse !== undefined,
  ...(packaged === undefined ? {} : { packaged }),
  seed: {
    schemaVersion: 1,
    teammates: [
      { teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: T0, route: FREE_ROUTE },
      { teammateId: 'tm_atlas', name: 'Atlas', hue: 'blue', role: 'Research & Briefs', createdAt: T0, route: FREE_ROUTE },
      { teammateId: 'tm_juno', name: 'Juno', hue: 'violet', role: 'Docs & QA', createdAt: T0, route: FREE_ROUTE }
    ],
    missionOwners: reuse === undefined ? {} : JSON.parse(await readFile(join(profile, 'teammates.json'), 'utf8')).missionOwners ?? {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'auto', autoMode: false }
  },
  files: {
    'memories.json': {
      schemaVersion: 1,
      memories: [
        { memoryId: 'mem_units', text: 'Tide heights are in feet above MLLW, never metres.', scope: 'workspace', workspaceId, workspaceName: 'harbor', by: { name: 'you' }, createdAt: T0, status: 'kept', enabled: true },
        { memoryId: 'mem_tone', text: 'Keep release notes to what a person would notice.', scope: 'global', by: { name: 'you' }, createdAt: T0, status: 'kept', enabled: true }
      ]
    }
  }
})

const notes = []
const shoot = async (number, title) => {
  await sleep(900)
  const file = `${String(number).padStart(2, '0')}-${title}.png`
  const shot = await drive.send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false })
  const data = shot?.result?.data
  if (typeof data !== 'string') throw new Error(`no screenshot for ${title}`)
  await writeFile(join(OUT, file), Buffer.from(data, 'base64'))
  notes.push(file)
  say(`frame ${file}`)
}
const evaluate = (expression) => drive.evaluate(expression)
const key = (digit) => evaluate(`(async () => {
  window.dispatchEvent(new KeyboardEvent('keydown', { key: '${digit}', ctrlKey: true, bubbles: true }))
  await new Promise(r => setTimeout(r, 900))
  return document.querySelector('.lc-screen__title')?.innerText ?? '(no screen title)'
})()`)
const click = (pattern, selector = 'button, a, [role="menuitem"]') => evaluate(`(async () => {
  const flat = (el) => el.innerText.replace(/\\s+/g, ' ').trim()
  const hit = [...document.querySelectorAll(${JSON.stringify(selector)})].find((el) => ${pattern}.test(flat(el)) || ${pattern}.test(el.getAttribute('title') ?? ''))
  if (!hit) return 'nothing matched ' + ${JSON.stringify(String(pattern))}
  hit.click()
  await new Promise(r => setTimeout(r, 900))
  return 'clicked ' + flat(hit).slice(0, 60)
})()`)
const scrollToHeading = (text) => evaluate(`(async () => {
  const h = [...document.querySelectorAll('h2, h3')].find((el) => el.innerText.trim().toLowerCase() === ${JSON.stringify(text.toLowerCase())})
  if (!h) return 'no heading ' + ${JSON.stringify(text)}
  h.scrollIntoView({ block: 'start' })
  await new Promise(r => setTimeout(r, 500))
  return 'at ' + h.innerText
})()`)
const chip = () => evaluate(`(() => {
  const control = [...document.querySelectorAll('.lc-control')].find(b => b.getAttribute('aria-haspopup') === 'listbox')
  return control ? control.innerText.replace(/\\s+/g, ' ').trim() : ''
})()`)

const PROMPT = 'Read README.md, then tell me in three short bullet points what this project is and what you would build first.'

try {
  await drive.ready()
  say(`window: ${String(await evaluate('window.innerWidth + "x" + window.innerHeight'))}`)

  if (reuse === undefined) {
    say(await click('/^Message Wren/', 'button'))
    say(await evaluate(pickRouteScript({ group: '/opencode/i', search: 'free', row: '/free/i' })))
    const route = await chip()
    if (!/opencode/i.test(route) || !/free/i.test(route)) throw new Error(`refusing to send: the composer is on "${route}"`)
    say(`route: ${route}`)
    say(await evaluate(sendAndWaitScript(PROMPT, { waitSeconds: 300 })))
  } else {
    // The conversation the first run made, from the sidebar.
    say(await click('/Read README\\.md/', '.lc-convrow button'))
  }

  await shoot(1, 'conversation')
  // Home is the wordmark.
  say(await click('/^Home$/', 'button.lc-brand__lockup'))
  await shoot(2, 'home')
  say(await evaluate(`(async () => {
    const control = [...document.querySelectorAll('.lc-control')].find(b => b.getAttribute('aria-haspopup') === 'listbox')
    control?.click()
    await new Promise(r => setTimeout(r, 900))
    return document.querySelector('.lc-picker') ? 'picker open' : 'no picker'
  })()`))
  await shoot(3, 'model-picker')
  await evaluate(`document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))`)
  await sleep(400)

  say(`missions: ${await key('1')}`)
  await shoot(4, 'missions')
  say(`team: ${await key('2')}`)
  await shoot(5, 'team')
  say(`rooms: ${await key('4')}`)
  await shoot(6, 'rooms')
  say(`memory: ${await key('5')}`)
  await shoot(7, 'memory')
  say(await click('/^Routines$/'))
  await shoot(8, 'routines')
  // A routine is made by saving a finished conversation.
  say(await click('/^Save$/'))
  await shoot(9, 'new-routine-dialog')
  await evaluate(`document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))`)
  await sleep(400)
  say(`settings: ${await key('3')}`)
  await shoot(10, 'settings-top')
  // Settings is paged by category, not one long scroll.
  say(await click('/^Teammates$/'))
  await shoot(11, 'settings-how-teammates-work')
  say(await click('/^Appearance$/'))
  await shoot(12, 'settings-appearance')
  say(await click('/^(AI agents|Runtimes)$/'))
  await shoot(13, 'settings-runtimes')
} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await writeFile(join(OUT, 'frames.json'), JSON.stringify({ label, packaged: packaged ?? 'out/', frames: notes }, null, 2), 'utf8')
  const { profile: kept } = await drive.finish({ intro: `Font frames, ${label}. ${packaged === undefined ? 'Build: out/.' : `Build: ${packaged}.`}` })
  say(`\nreuse with: --reuse ${kept}`)
}
