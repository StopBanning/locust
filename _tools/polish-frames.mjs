// Every screen and dialog a person opens in their first minutes, in both themes and at real window sizes, for a
// design pass (Colin, 2026-11-01: "its like youve never put eyes on the app ... world class apple quality").
//
//   node _tools/polish-frames.mjs --out <folder outside the repo> [--usage <usage-carried.jsonl>]
//        [--sizes 1440x900,1120x720,1920x1080] [--themes dark,light] [--scale 1] [--only home,usage,settings,dialogs,screens,thread,menus]
//        [--glass-check] [--fresh]
//
// Spends nothing: it opens screens and dialogs and closes them. `thread` writes a finished two-turn conversation with
// Wren into the ledger before launch -- two commands, a reply with headings, a list, a table and code, and a follow-up
// -- so the conversation screen is seen full without a model running. --usage puts a numbers-only usage
// record (the kind usage-carried.ts reads) in the throwaway profile, so Usage and Home's usage line draw as they do
// for a person with history. Frames land in <out>/<theme>-<width>x<height>/<name>.png, with a .txt of the screen's words.

import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

import { FREE_ROUTE, openTeammateScript, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const OUT = resolve(arg('--out') ?? 'polish-frames')
const usage = arg('--usage')
const sizes = (arg('--sizes') ?? '1440x900').split(',').map((size) => size.split('x').map(Number))
const themes = (arg('--themes') ?? 'dark').split(',')
const scale = Number(arg('--scale') ?? '1')
const only = arg('--only')?.split(',')
const want = (part) => only === undefined || only.includes(part)

const workspace = await scratchRepository('locust-polish-ws-')

const KICKOFF = [
  '## Storefront kickoff',
  '',
  'A small shop backend: carts, prices and checkout. The cart total doubles every item today, so that is the first fix.',
  '',
  '- **Fix the double count** in `src/cart.py` -- the subtotal is added twice.',
  '- **Cover it with a test** beside `tests/test_prices.py`, so it cannot come back.',
  '- **Ship a checkout preview** once the totals are right.',
  '',
  '| Owner | Task | Due |',
  '| --- | --- | --- |',
  '| Wren | Cart total fix | Mon 3 Nov |',
  '| Atlas | Checkout preview | Thu 6 Nov |',
  '',
  '```ts',
  'export function total(items: readonly Item[]): number {',
  '  return items.reduce((sum, item) => sum + item.price * item.qty, 0)',
  '}',
  '```',
  '',
  'Want me to start on the fix?'
].join('\n')

/** A finished two-turn conversation with Wren and seven shorter ones across the team, in a fresh profile's ledger. No model runs. */
async function seedConversation(folder) {
  const { createFileMissionLedger } = await import('../packages/mission-store/dist/index.js')
  const profile = await mkdtemp(join(tmpdir(), 'locust-polish-profile-'))
  const ledger = createFileMissionLedger({ rootDirectory: join(profile, 'mission-ledger') })
  const workspaceId = `ws_${createHash('sha256').update(folder).digest('hex').slice(0, 32)}`
  const turns = [
    {
      missionId: 'mission_polish_first',
      owner: 'tm_wren',
      prompt: 'Look at the files in this folder and tell me what this project is, then sketch a kickoff note.',
      tools: [
        { command: 'ls -la', title: 'Listed the files in the folder', output: 'README.md\nissue.md\nsrc\ntests' },
        { command: 'cat README.md issue.md', title: 'Read the README and the open issue', output: '# Acme Storefront ...' }
      ],
      answer: KICKOFF
    },
    {
      missionId: 'mission_polish_second',
      owner: 'tm_wren',
      follows: 'mission_polish_first',
      prompt: 'Yes, start on the fix. Keep it to the one function.',
      tools: [{ command: 'python -m pytest -q', title: 'Ran the tests', output: '3 passed in 0.12s' }],
      answer: 'Fixed. `total()` now adds each item once, and the three tests pass.\n\nThe change is one line in `src/cart.py`.'
    },
    { missionId: 'mission_polish_pay', owner: 'tm_atlas', prompt: 'Compare the three payment providers we could use for checkout.', tools: [], answer: 'Stripe is the quickest to wire in; Adyen costs less at volume; Square fits if the shop sells in person too.', at: '2026-10-31T17:20:00.000Z' },
    { missionId: 'mission_polish_readme', owner: 'tm_sable', prompt: 'Write the README section on running the tests.', tools: [], answer: 'Added a Testing section: install the dev requirements, then run `python -m pytest -q`.', at: '2026-10-31T15:05:00.000Z' },
    { missionId: 'mission_polish_weekly', owner: 'tm_quill', prompt: 'Every Monday, summarise what changed in the repo last week.', tools: [], answer: 'Saved as a routine for Mondays at 9am. The first summary lands next week.', at: '2026-10-31T11:40:00.000Z' },
    { missionId: 'mission_polish_tax', owner: 'tm_wren', prompt: 'Why does the tax test fail on Windows only?', tools: [], answer: 'The fixture file has CRLF line endings, so the parsed rate keeps a trailing carriage return.', at: '2026-10-30T16:10:00.000Z' },
    { missionId: 'mission_polish_brief', owner: 'tm_atlas', prompt: 'Draft a one-page brief on the cart redesign.', tools: [], answer: 'Drafted: goals, the three screens that change, and what we measure after launch.', at: '2026-10-30T10:25:00.000Z' },
    { missionId: 'mission_polish_links', owner: 'tm_sable', prompt: 'Check the docs for broken links.', tools: [], answer: 'Two broken links, both to the old API guide. Fixed them to point at the new one.', at: '2026-10-29T14:00:00.000Z' },
    { missionId: 'mission_polish_release', owner: 'tm_quill', prompt: 'Plan the release checklist for Friday.', tools: [], answer: 'Checklist ready: freeze Thursday noon, run the full suite, tag, then post the notes.', at: '2026-10-29T09:30:00.000Z' }
  ]
  for (const [index, turn] of turns.entries()) {
    const runId = `run_polish_${String(index)}`
    const at = turn.at ?? `2026-11-01T1${String(index)}:00:00.000Z`
    await ledger.createMission({
      missionId: turn.missionId, runId, prompt: turn.prompt, runtime: 'opencode', model: FREE_ROUTE.model,
      requestedRouteId: 'opencode', resolvedRouteId: 'opencode-account:default', cliVersion: null, workspaceId,
      sandbox: 'workspace-write', executionPolicyVersion: 1, createdAt: at,
      ...(turn.follows === undefined ? {} : { continuesFrom: { missionId: turn.follows, checkpointEpoch: 1, reason: 'follow-up' } })
    })
    let sequence = 0
    const event = (type, payload) => ({
      id: `event_polish_${String(index)}_${String(++sequence)}`, missionId: turn.missionId, runId, sequence, occurredAt: at,
      sourceAdapter: 'opencode', type, payload: { ...payload, evidence: { redacted: true } }
    })
    const events = []
    for (const [at2, tool] of turn.tools.entries()) {
      const itemId = `tool_${String(index)}_${String(at2)}`
      events.push(event('tool.started', { itemId, toolKind: 'command', name: 'bash', command: tool.command, title: tool.title, phase: 'started' }))
      events.push(event('tool.completed', { itemId, toolKind: 'command', name: 'bash', command: tool.command, title: tool.title, output: tool.output, exitCode: 0, phase: 'completed' }))
    }
    events.push(event('message.delta', { itemId: 'answer', operation: 'append', text: turn.answer, final: true }))
    events.push(event('run.completed', { process: { exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: events.length + 1, inputDeliveryFailed: false, outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: at, finishedAt: at } }))
    await ledger.appendEvents(turn.missionId, events)
  }
  await ledger.flush()
  return { profile, owners: Object.fromEntries(turns.map((turn) => [turn.missionId, turn.owner])) }
}
const seeded = want('thread') ? await seedConversation(workspace) : undefined
const seededProfile = seeded?.profile
const at = '2026-10-20T09:00:00.000Z'
const drive = await startDrive({
  name: 'polish-frames',
  port: 9461,
  workspace,
  outPath: join(OUT, 'drive'),
  sendsNothing: true,
  ...(seededProfile === undefined ? {} : { profilePath: seededProfile }),
  seed: {
    schemaVersion: 1,
    // --fresh: nobody on the team yet, as a person who just installed Locust sees it first.
    teammates: process.argv.includes('--fresh') ? [] : [
      { teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: at, route: FREE_ROUTE },
      { teammateId: 'tm_atlas', name: 'Atlas', hue: 'blue', role: 'Research & Briefs', createdAt: at, route: FREE_ROUTE },
      { teammateId: 'tm_sable', name: 'Sable', hue: 'clay', role: 'Docs & QA', createdAt: at, route: FREE_ROUTE },
      { teammateId: 'tm_quill', name: 'Quill', hue: 'violet', role: 'Ops & Scheduling', createdAt: at, route: FREE_ROUTE }
    ],
    missionOwners: seeded?.owners ?? {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false }
  },
  files: usage === undefined ? {} : { 'usage-carried.jsonl': await readFile(usage, 'utf8') }
})

const { evaluate, send } = drive
say(await drive.ready())

const key = (code, { ctrl = false, shift = false } = {}) =>
  evaluate(`window.dispatchEvent(new KeyboardEvent('keydown', { key: ${JSON.stringify(code)}, ctrlKey: ${String(ctrl)}, shiftKey: ${String(shift)}, bubbles: true })), true`)
const escape = async () => {
  await evaluate(`(() => { const el = document.activeElement ?? document.body; for (const target of [el, document, window]) target.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); return true })()`)
  await sleep(350)
}
const click = (script) => evaluate(`(() => { const el = ${script}; if (!el) return false; el.click(); return true })()`)
const byText = (selector, text) =>
  `[...document.querySelectorAll(${JSON.stringify(selector)})].find((el) => el.innerText.replace(/\\s+/g, ' ').trim() === ${JSON.stringify(text)})`
const palette = async (label) => {
  await key('k', { ctrl: true })
  await sleep(300)
  // A row's words are its label and its hint ("Usage every agent and model"): match the label alone.
  const done = await click(`${byText('.lc-palette__label', label)}?.closest('button')`)
  await sleep(700)
  if (!done) await escape()
  return done
}

let folder = OUT
const shot = async (name) => {
  await sleep(450)
  const picture = await send('Page.captureScreenshot', { format: 'png' })
  if (picture?.result?.data) await writeFile(join(folder, `${name}.png`), Buffer.from(picture.result.data, 'base64'))
  const words = await evaluate('document.body.innerText')
  await writeFile(join(folder, `${name}.txt`), String(words ?? ''), 'utf8')
  say(`  ${name}`)
}

for (const theme of themes) {
  for (const [width, height] of sizes) {
    folder = join(OUT, `${theme}-${String(width)}x${String(height)}`)
    await mkdir(folder, { recursive: true })
    say(`${theme} ${String(width)}x${String(height)}`)
    await drive.resize(width, height, scale)
    await evaluate(`document.documentElement.dataset.theme = ${JSON.stringify(theme)}, true`)
    // --glass-check: the Mac's glass page on any computer, magenta standing in for the glass, so a frame shows every
    // place that lets it through -- which must be the sidebar and the title bar only.
    if (process.argv.includes('--glass-check')) await evaluate(`document.documentElement.dataset.glass = 'mac', document.documentElement.style.background = '#ff00ff', true`)
    await key('1', { ctrl: true })
    await sleep(900)

    if (want('home')) {
      await click(`document.querySelector('[aria-label="Home"]')`)
      await sleep(1600)
      await shot('home')
      // With words in the box, so the send button is seen ready, not only empty.
      await evaluate(`(() => { const box = document.querySelector('.lc-composer__box textarea'); if (!box) return false; const set = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set; set.call(box, 'Plan the release checklist for Friday'); box.dispatchEvent(new Event('input', { bubbles: true })); return true })()`)
      await sleep(500)
      await shot('home-typed')
      await evaluate(`(() => { const box = document.querySelector('.lc-composer__box textarea'); if (!box) return false; const set = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set; set.call(box, ''); box.dispatchEvent(new Event('input', { bubbles: true })); return true })()`)
    }

    if (want('thread')) {
      await evaluate(openTeammateScript('Wren'))
      await sleep(1200)
      await shot('thread')
      await evaluate(`document.querySelector('.lc-thread')?.scrollTo(0, 0), true`)
      await shot('thread-top')
    }

    // The chat box's menus, on the open conversation: the model, the permission mode, the chat mode, the + menu.
    if (want('menus')) {
      await evaluate(openTeammateScript('Wren'))
      await sleep(900)
      for (const [name, selector] of [
        ['menu-model', '.command-dock .lc-control[aria-haspopup="listbox"]'],
        ['menu-permission', '.command-dock [aria-label^="Permission"]'],
        ['menu-chatmode', '.command-dock .lc-control--chatmode'],
        ['menu-plus', '.command-dock .lc-plusmenu__main']
      ]) {
        if (await click(`document.querySelector(${JSON.stringify(selector)})`)) {
          await sleep(700)
          await shot(name)
          await escape()
          await sleep(300)
        } else say(`  no ${name} control`)
      }
    }

    if (want('usage')) {
      if (await palette('Usage')) {
        await sleep(1500)
        // As a person who clicked Details sees it: the keyboard's focus ring is for a keyboard open.
        await evaluate('document.activeElement?.blur?.(), true')
        await shot('usage')
        await evaluate(`document.querySelector('.lc-usage__body')?.scrollTo(0, 9999), true`)
        await shot('usage-scrolled')
        await escape()
      } else say('  no Usage in the palette')
    }

    if (want('dialogs')) {
      await key('k', { ctrl: true })
      await shot('palette')
      await escape()
      if (await palette('New teammate')) { await shot('new-teammate'); await escape() }
      if (await palette('Keyboard shortcuts')) { await shot('keyboard'); await escape() }
    }

    if (want('screens')) {
      for (const [index, name] of [[2, 'team'], [4, 'rooms'], [5, 'memory']]) {
        await key(String(index), { ctrl: true })
        await sleep(900)
        await shot(`screen-${name}`)
      }
      await key('1', { ctrl: true })
      await sleep(900)
      await shot('screen-conversations')
      // Routines is a place in the sidebar, the Board a button in its foot.
      for (const label of ['Routines', 'Board']) {
        if (await click(byText('.lc-sidebar button', label))) {
          await sleep(1000)
          await shot(`screen-${label.toLowerCase()}`)
        } else say(`  no ${label} button`)
      }
      await key('1', { ctrl: true })
      await sleep(600)
    }

    if (want('settings')) {
      await key('3', { ctrl: true })
      await sleep(1200)
      const pages = await evaluate(`[...document.querySelectorAll('.lc-settings__navitem')].map((el) => el.innerText.replace(/\\s+/g, ' ').trim())`)
      for (const [index, page] of (pages ?? []).entries()) {
        await evaluate(`document.querySelectorAll('.lc-settings__navitem')[${String(index)}]?.click(), true`)
        await sleep(700)
        await evaluate(`document.querySelector('.lc-settings__pane')?.scrollTo(0, 0), true`)
        const slug = page.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '')
        await shot(`settings-${String(index + 1).padStart(2, '0')}-${slug}`)
        const tall = await evaluate(`(() => { const pane = document.querySelector('.lc-settings__pane'); return pane ? pane.scrollHeight > pane.clientHeight + 40 : false })()`)
        if (tall) {
          await evaluate(`(() => { const pane = document.querySelector('.lc-settings__pane'); pane.scrollTo(0, pane.clientHeight - 60); return true })()`)
          await shot(`settings-${String(index + 1).padStart(2, '0')}-${slug}-2`)
        }
      }
      await key('1', { ctrl: true })
      await sleep(600)
    }
  }
}

await drive.finish({ intro: 'Every screen and dialog, for the design pass. Sends nothing.' })
