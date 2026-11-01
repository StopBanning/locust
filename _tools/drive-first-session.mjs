// A new person's first two minutes, photographed (0.358).
//
//   node _tools/drive-first-session.mjs [--packaged <exe>] [--tag <name>]
//        [--team "Write & design"] [--mate Iris] [--starter 1 | --ask "..."]
//        [--plain]
//
// `--plain` works in a folder with no git in it -- which is what someone who
// is not a coder has (0.365): the host must see a file a command made there
// too, and the Artifacts tab must list it.
//
// Locust is for "quite literally ANY ai user" (Colin, 2026-09-26), so this is
// someone who is not a coder: nobody on the team, nothing remembered. They
// start with a team from Home, open one of its teammates, and either press
// one of that teammate's suggestions (--starter N, the real button) or type
// a question (--ask). Then they read the answer, look at what happened, and
// look at any file it made. Every screen is captured at 1440x900 for a
// person to judge.
//
// Defaults are the first run of it: Research & money, Sable, a money
// question. Two things it checks, both found by that run on 0.358: a
// teammate outside Build software offers no code, and the Activity panel
// under a run that searched the web does not say the network was denied.
//
// A starter that asks another teammate for something (Moss's third: "Ask
// Quill for a short piece...") is waited out until the WHOLE team is quiet,
// not just the conversation on screen: the reply comes back after Quill's
// own run.

import { mkdir, mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { say, scratchRepository, sendAndWaitScript, sleep, startDrive, teammateFace, teammateRows, recordRoot } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag')
const team = arg('--team') ?? 'Research & money'
const mate = arg('--mate') ?? 'Sable'
const starter = arg('--starter') === undefined ? undefined : Number(arg('--starter'))
const plain = process.argv.includes('--plain')
const ask = arg('--ask') ?? (starter === undefined ? 'I have $12,000 saved and want to use it within three years. How should I think about where to keep it?' : undefined)
const slug = `${team === 'Research & money' && mate === 'Sable' && starter === undefined ? '' : `-${mate.toLowerCase()}${starter === undefined ? '' : `-starter${String(starter)}`}`}${plain ? '-plain' : ''}`
const outPath = tag === undefined ? undefined : join(recordRoot('beta-fixes-2026-09-24'), `first-session${slug}-${tag}`)
if (outPath !== undefined) await mkdir(outPath, { recursive: true })

const workspace = plain ? await mkdtemp(join(tmpdir(), 'locust-drive-first-session-plain-')) : await scratchRepository('locust-drive-first-session-ws-')
if (plain) await writeFile(join(workspace, 'notes.md'), ['# My notes', '', 'A plain folder, no git.', ''].join('\n'), 'utf8')
/*
 * `--project`: a tiny shop with one real bug and a test that shows it, so a
 * coder's first "fix one small thing" has something true to find -- a
 * total that skips the first price. Committed, so the fix is a diff.
 */
if (process.argv.includes('--project')) {
  await mkdir(join(workspace, 'src'), { recursive: true })
  await mkdir(join(workspace, 'test'), { recursive: true })
  await writeFile(join(workspace, 'package.json'), JSON.stringify({ name: 'shop-total', private: true, type: 'module', scripts: { test: 'node --test' } }, null, 2) + '\n', 'utf8')
  await writeFile(join(workspace, 'src', 'total.js'), [
    '// Adds up a basket of prices, in cents.',
    'export function total(prices) {',
    '  let sum = 0',
    '  for (let i = 1; i < prices.length; i += 1) sum += prices[i]',
    '  return sum',
    '}',
    ''
  ].join('\n'), 'utf8')
  await writeFile(join(workspace, 'test', 'total.test.js'), [
    "import { test } from 'node:test'",
    "import assert from 'node:assert/strict'",
    '',
    "import { total } from '../src/total.js'",
    '',
    "test('adds every price in the basket', () => {",
    '  assert.equal(total([100, 250, 50]), 400)',
    '})',
    ''
  ].join('\n'), 'utf8')
  const { git } = await import('./drive-lib.mjs')
  await git(['add', '.'], workspace)
  await git(['commit', '-q', '-m', 'shop total'], workspace)
}
const drive = await startDrive({
  name: 'first-session',
  port: 9617,
  workspace,
  ...(packaged === undefined ? {} : { packaged }),
  ...(outPath === undefined ? {} : { outPath }),
  // No settings at all: a new profile's own defaults. The seed used to say
  // relayHopCap 2 (copied from the old relay drive), and Rook's first
  // starter stopped after two automatic replies with no plan -- a cap no
  // new person has; the default is 12 (DEFAULT_RELAY_HOP_CAP).
  seed: { schemaVersion: 1, teammates: [], missionOwners: {} }
})
const verdicts = []

const text = (selector) => `(document.querySelector(${JSON.stringify(selector)})?.innerText ?? '').replace(/\\s+/g, ' ').slice(0, 220)`
const CODE_WORDS = /\b(code|codebase|repo|repository|pull request|README)\b/i

/** Until nothing on the team is working, and has not been for four seconds. */
const quietScript = (waitSeconds) => `(async () => {
  let calm = 0
  for (let i = 0; i < ${String(waitSeconds * 2)}; i += 1) {
    await new Promise((r) => setTimeout(r, 500))
    const busy = document.querySelector('button[aria-label^="Stop the running"]') !== null
      || ${teammateRows()}.some((row) => /working|running|starting|replying|listening|thinking|waiting(?! on you)/i.test(row.innerText))
    calm = busy ? 0 : calm + 1
    if (i > 6 && calm >= 8) break
  }
  return 'quiet: ' + (document.querySelector('.lc-sidebar')?.innerText ?? '').replace(/\\s+/g, ' ').slice(0, 220)
})()`

try {
  await drive.capture('launch: a new person, nobody on the team', () => drive.ready())
  await drive.resize(1440, 900)
  await sleep(1200)
  await drive.capture('Home, first thing', () => drive.evaluate(text('main')))
  await drive.capture(`press ${team}`, () => drive.evaluate(`(async () => {
    const card = [...document.querySelectorAll('.lc-teamtemplate')].find((one) => one.textContent.includes(${JSON.stringify(team)}))
    if (!card) return 'no ${team.replace(/'/g, '')} card'
    card.click()
    for (let i = 0; i < 40 && !document.querySelector('.lc-hometeam:not(.lc-teamtemplates)'); i += 1) await new Promise((r) => setTimeout(r, 150))
    await new Promise((r) => setTimeout(r, 800))
    return ${text('.lc-hometeam')}
  })()`))
  await drive.capture(`open ${mate} from Home`, () => drive.evaluate(`(async () => {
    const card = [...document.querySelectorAll('.lc-hometeam__card')].find((one) => one.textContent.includes(${JSON.stringify(mate)}))
    if (!card) return 'no ${mate} card'
    card.click()
    await new Promise((r) => setTimeout(r, 1300))
    return document.querySelector('form.command-dock textarea')?.getAttribute('placeholder') ?? 'no composer'
  })()`))
  const offered = await drive.evaluate(`JSON.stringify({ starters: [...document.querySelectorAll('.lc-starter')].map((one) => one.textContent), line: document.querySelector('.lc-empty__inner p')?.textContent ?? '' })`)
  const { starters, line } = JSON.parse(offered)
  say(`${mate} offers: ${starters.join(' | ')} || ${line}`)
  if (team !== 'Build software') {
    verdicts.push(`${mate} offers no code: ${starters.length === 3 && !starters.some((one) => CODE_WORDS.test(one)) && !/codebase|reads this workspace/.test(line) ? 'PASS' : 'FAIL'}`)
  }

  const asked = starter === undefined ? ask : starters[starter - 1]
  if (asked === undefined) throw new Error(`no starter ${String(starter)}`)
  say(`asking: ${asked}`)
  await drive.capture(starter === undefined ? `ask ${mate}` : `press starter ${String(starter)}: ${asked.slice(0, 60)}`, () =>
    starter === undefined
      ? drive.evaluate(sendAndWaitScript(asked, { settle: false }))
      : drive.evaluate(`(async () => {
          const button = [...document.querySelectorAll('.lc-starter')][${String(starter - 1)}]
          if (!button || button.disabled) return 'no starter button'
          button.click()
          await new Promise((r) => setTimeout(r, 1500))
          return ${text('.lc-thread')}
        })()`)
  )
  // The notes an exchange leaves belong to the turn that asked, and that
  // turn's view gives way to the next -- so they are photographed while the
  // team is still working, if any appear (0.366).
  await drive.capture('while the team works: the exchange notes', () => drive.evaluate(`(async () => {
    for (let i = 0; i < 480; i += 1) {
      const notes = [...document.querySelectorAll('.lc-diagnostic')]
      if (notes.length > 0) {
        await new Promise((r) => setTimeout(r, 1500))
        const now = [...document.querySelectorAll('.lc-diagnostic')]
        now.at(-1)?.scrollIntoView({ block: 'center' })
        document.scrollingElement.scrollTop = 0
        await new Promise((r) => setTimeout(r, 400))
        return now.map((note) => (note.className.includes('amber') ? 'AMBER ' : note.className.includes('muted') ? 'quiet ' : '') + note.textContent.trim()).join(' | ').slice(0, 700)
      }
      const busy = document.querySelector('button[aria-label^="Stop the running"]') !== null
        || ${teammateRows()}.some((row) => /working|running|starting|replying|listening|thinking|waiting(?! on you)/i.test(row.innerText))
      if (i > 20 && !busy) return 'no notes while it worked'
      await new Promise((r) => setTimeout(r, 500))
    }
    return 'no notes in time'
  })()`))
  // A free model whose provider is busy (C9), or that has said nothing for two minutes (0.846), is met as a person
  // meets it: its Stop and switch, then Send, up to twice. 2026-11-01: this connection's Deepseek allowance was spent,
  // and the run sat on "Free usage exceeded" to its end with nothing written.
  for (let switched = 0; switched < 2; switched += 1) {
    const offer = await drive.evaluate(`(async () => {
      for (let i = 0; i < 300; i += 1) {
        const button = [...document.querySelectorAll('button')].find((b) => /^Stop and switch to /.test(b.innerText.trim()))
        const running = document.querySelector('button[aria-label^="Stop the running"]') !== null
        if (button && running) { const words = button.innerText.trim(); button.click(); return words }
        if (!running && i > 10) return ''
        await new Promise((r) => setTimeout(r, 500))
      }
      return ''
    })()`)
    if (offer === '') break
    await sleep(1500)
    await drive.capture(`${String(offer)}, then Send`, () => drive.evaluate(`(async () => {
      const box = document.querySelector('form.command-dock textarea')
      for (let i = 0; i < 120; i += 1) {
        await new Promise((r) => setTimeout(r, 250))
        const send = document.querySelector('button[aria-label="Send"]')
        if (send && !send.disabled && box && box.value.length > 0) { const words = box.value.slice(0, 80); send.click(); return 'sent: ' + words }
      }
      return 'Send never enabled'
    })()`))
  }
  const quiet = await drive.capture('the whole team, once quiet', () => drive.evaluate(quietScript(480)))
  // A teammate that ends by asking reads "waiting on you": stopped, not busy (0.695 sweep: the wait outlived
  // the evaluate, and `quiet` came back undefined).
  say(String(quiet ?? 'no answer in time').slice(0, 200))
  await drive.capture(`${mate}'s conversation, at its end`, () => drive.evaluate(`(async () => {
    const face = ${teammateFace(mate)}
    face?.click()
    await new Promise((r) => setTimeout(r, 1200))
    const scroller = document.querySelector('.lc-thread')?.closest('[class*="scroll"]') ?? document.querySelector('.lc-thread')
    scroller?.scrollTo?.({ top: 1e9 })
    await new Promise((r) => setTimeout(r, 600))
    return (document.querySelector('.lc-thread')?.innerText ?? '').replace(/\\s+/g, ' ').slice(-400)
  })()`))
  // What the exchange said on the way (0.366): progress quiet, problems amber.
  await drive.capture('the notes the exchange left', () => drive.evaluate(`(async () => {
    const notes = [...document.querySelectorAll('.lc-thread .lc-diagnostic')]
    if (notes.length === 0) return 'no notes'
    const first = notes[0]
    let scroller = first.parentElement
    while (scroller && scroller !== document.body && !/(auto|scroll)/.test(getComputedStyle(scroller).overflowY)) scroller = scroller.parentElement
    if (scroller && scroller !== document.body) scroller.scrollTop += first.getBoundingClientRect().top - scroller.getBoundingClientRect().top - 220
    await new Promise((r) => setTimeout(r, 500))
    return notes.map((note) => (note.className.includes('amber') ? 'AMBER ' : note.className.includes('muted') ? 'quiet ' : '') + note.textContent.trim()).join(' | ').slice(0, 600)
  })()`))
  // The window's own root must never scroll -- everything scrolls inside it.
  // (A scrollIntoView in the 0.363 drive moved the whole window 11px.)
  const rootOverflow = await drive.evaluate(`JSON.stringify({ y: Math.round(document.scrollingElement.scrollHeight - document.scrollingElement.clientHeight), x: Math.round(document.scrollingElement.scrollWidth - document.scrollingElement.clientWidth), top: document.scrollingElement.scrollTop })`)
  say(`root overflow in a conversation: ${rootOverflow}`)
  verdicts.push(`the window root does not scroll: ${/"y":0,"x":0/.test(rootOverflow) ? 'PASS' : 'FAIL'} (${rootOverflow})`)
  // A new document shows in the conversation as the page it is (0.363), once its row is open. The steps fold and its
  // file rows start closed now (the 0.845 walkthrough found no preview), so this opens them as a person does.
  await drive.capture('the document it wrote, in the conversation', () => drive.evaluate(`(async () => {
    let preview = document.querySelector('.lc-docpreview')
    if (!preview) {
      for (const fold of document.querySelectorAll('.lc-steps__line[aria-expanded="false"], .lc-activity[aria-expanded="false"]')) fold.click()
      await new Promise((r) => setTimeout(r, 400))
      ;[...document.querySelectorAll('.lc-filerow[aria-expanded="false"]')].find((row) => /\\.(md|txt|html?)$/i.test(row.querySelector('.lc-filerow__name')?.textContent.trim() ?? ''))?.click()
      await new Promise((r) => setTimeout(r, 600))
      preview = document.querySelector('.lc-docpreview')
    }
    if (!preview) return 'no document preview'
    // The thread's own scroller only: scrollIntoView moves every scrollable
    // ancestor, the window's root included.
    const card = preview.closest('.lc-card, .lc-steps')
    let scroller = card?.parentElement
    while (scroller && scroller !== document.body && !/(auto|scroll)/.test(getComputedStyle(scroller).overflowY)) scroller = scroller.parentElement
    if (card && scroller && scroller !== document.body) scroller.scrollTop += card.getBoundingClientRect().top - scroller.getBoundingClientRect().top - 12
    await new Promise((r) => setTimeout(r, 600))
    return (preview.innerText ?? '').replace(/\\s+/g, ' ').slice(0, 220)
  })()`))
  // What a person does with a file their teammate made: open it. The pill
  // under the answer, or the fold's own open control when there is no pill.
  await drive.capture('open the file it made', () => drive.evaluate(`(async () => {
    const open = document.querySelector('.lc-handedfile__open') ?? document.querySelector('.lc-filerow__view')
    if (!open) return 'no file to open'
    open.click()
    await new Promise((r) => setTimeout(r, 1500))
    return ${text('.lc-viewer')}
  })()`))
  await drive.evaluate(`(() => { document.querySelector('.lc-viewer__close')?.click(); return 'closed' })()`)
  await sleep(600)
  await drive.capture('what happened (Activity)', () => drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('button')].find((b) => (b.getAttribute('aria-label') || b.textContent.trim()) === 'Activity')?.click()
    await new Promise((r) => setTimeout(r, 900))
    return ${text('.lc-inspector')}
  })()`))
  const panel = await drive.evaluate(`(document.querySelector('.lc-inspector')?.innerText ?? '').replace(/\\s+/g, ' ')`)
  if (/Searched the web/.test(panel)) {
    verdicts.push(`the panel says the web is allowed (it searched): ${/search the web and open web pages/.test(panel) && !/beyond the model's own/.test(panel) ? 'PASS' : 'FAIL'}`)
  }
  await drive.capture('the files it made (Artifacts)', () => drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('.lc-inspector button[role="tab"]')].find((b) => b.textContent.trim() === 'Artifacts')?.click()
    await new Promise((r) => setTimeout(r, 700))
    return ${text('.lc-inspector')}
  })()`))
  await drive.capture('back on Home, with a team and a conversation', () => drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('.lc-inspector__close')].forEach((b) => b.click())
    document.querySelector('.lc-brand__lockup')?.click()
    await new Promise((r) => setTimeout(r, 1200))
    return ${text('main')}
  })()`))
  await drive.capture('the sidebar: who has a conversation now', () => drive.evaluate(`(document.querySelector('.lc-sidebar')?.innerText ?? '').replace(/\\s+/g, ' ').slice(0, 300)`))
  say(verdicts.join(' | '))
} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'whatever pnpm build last wrote to out/'}. A new person, nobody on the team: ${team}, then ${mate}${starter === undefined ? `, asked: ${ask ?? ''}` : `'s starter ${String(starter)}`}, on the free model. Screens for a person to judge. Verdicts: ${verdicts.join('; ')}` })
}
