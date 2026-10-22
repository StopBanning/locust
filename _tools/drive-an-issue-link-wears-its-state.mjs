// An issue link wears its state (0.816), on three settled public issues of cli/cli: #136 open, #2 closed as
// completed, #382 closed as not planned. Read only on GitHub (`gh issue view`); nothing is written there.
//
//   node _tools/drive-an-issue-link-wears-its-state.mjs [--packaged <exe>]
//
// The person's message names the three, and Wren, on the free model, answers one word. Beside each link in the
// person's bubble is a chip: Open in green, Closed in violet, Not planned in grey -- never a closed pull request's
// red -- and each hover says its standing, then the issue's title as gh gives it. At three window sizes.

import { execFileSync } from 'node:child_process'
import { join } from 'node:path'

import { FREE_ROUTE, recordRoot, say, scratchRepository, sendAndWaitScript, sleep, startDrive, teammateFace } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const gh = (args) => execFileSync('gh', args, { encoding: 'utf8', windowsHide: true, timeout: 60_000 }).trim()
const ISSUES = [
  { number: 136, state: 'open', word: 'Open', standing: 'Open', token: '--lc-green-text' },
  { number: 2, state: 'closed-completed', word: 'Closed', standing: 'Closed as completed', token: '--lc-violet-text' },
  { number: 382, state: 'closed-not-planned', word: 'Not planned', standing: 'Closed as not planned', token: '--lc-text-muted' }
].map((issue) => ({ ...issue, title: gh(['issue', 'view', String(issue.number), '--repo', 'cli/cli', '--json', 'title', '--jq', '.title']).replace(/\s+/g, ' ') }))
say(ISSUES.map((issue) => `#${String(issue.number)} ${issue.title}`).join(' | '))

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 400)}`}`)
}
const workspace = await scratchRepository('locust-issue-links-')
const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: 'an-issue-link-wears-its-state',
  port: 9941,
  workspace,
  outPath: join(recordRoot('an-issue-link-wears-its-state-2026-10-22'), packaged === undefined ? 'local' : 'packaged'),
  seed: {
    schemaVersion: 1,
    teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-09-05T05:00:00.000Z', route: { ...FREE_ROUTE, mode: 'ask' } }],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false }
  }
})
// Every chip in the person's bubble, with the link it sits beside and the colour it is drawn in.
const CHIPS = `(async () => {
  const colorOf = (token) => {
    const probe = document.createElement('span')
    probe.style.color = 'var(' + token + ')'
    document.body.append(probe)
    const value = getComputedStyle(probe).color
    probe.remove()
    return value
  }
  for (let i = 0; i < 40 && document.querySelectorAll('.lc-prlink[data-issue-state]').length < ${String(ISSUES.length)}; i += 1) await new Promise((r) => setTimeout(r, 500))
  const red = colorOf('--lc-red-text')
  return JSON.stringify([...document.querySelectorAll('.lc-prlink[data-issue-state]')].map((chip) => ({
    href: chip.previousElementSibling?.getAttribute('title') ?? '',
    state: chip.getAttribute('data-issue-state'),
    word: chip.innerText.trim(),
    hover: chip.getAttribute('title'),
    spoken: chip.getAttribute('aria-label'),
    color: getComputedStyle(chip).color,
    red,
    tokens: ${JSON.stringify(Object.fromEntries(ISSUES.map((issue) => [issue.state, issue.token])))},
    wanted: colorOf(${JSON.stringify(Object.fromEntries(ISSUES.map((issue) => [issue.state, issue.token])))}[chip.getAttribute('data-issue-state')] ?? '--none')
  })))
})()`
try {
  await drive.ready()
  await drive.resize(1440, 900)
  await sleep(1500)
  await drive.evaluate(`(async () => { ${teammateFace('Wren')}.click(); await new Promise(r => setTimeout(r, 600)) })()`)
  const links = ISSUES.map((issue) => `https://github.com/cli/cli/issues/${String(issue.number)}`).join(' and ')
  const sent = String(await drive.evaluate(sendAndWaitScript(`Reply with the single word ok and run nothing. For later: ${links}`, { waitSeconds: 240 })))
  check('the message went, and its turn ended', sent.startsWith('finished'), sent)

  // 1. Three links, three chips, each in GitHub's words and colours.
  const chips = JSON.parse(String(await drive.capture('three issue links, three chips', () => drive.evaluate(CHIPS))))
  for (const issue of ISSUES) {
    const chip = chips.find((one) => one.href.endsWith(`/issues/${String(issue.number)}`))
    const hover = `${issue.standing} · ${issue.title}`
    check(`#${String(issue.number)} wears ${issue.word}, its standing and title on hover`,
      chip !== undefined && chip.state === issue.state && chip.word === issue.word && chip.hover === hover && chip.spoken === hover,
      JSON.stringify(chip))
    check(`#${String(issue.number)} is drawn in ${issue.token}, not a closed pull request's red`,
      chip !== undefined && chip.color === chip.wanted && chip.color !== chip.red, JSON.stringify(chip && { color: chip.color, wanted: chip.wanted, red: chip.red }))
  }
  check('only the three issue links wear a chip', chips.length === ISSUES.length, chips.map((chip) => chip.href).join(', '))

  // 2. The same chips, whole, at two more sizes.
  for (const [width, height] of [[1120, 720], [860, 640]]) {
    await drive.resize(width, height)
    await sleep(700)
    const at = JSON.parse(String(await drive.capture(`the chips at ${String(width)} x ${String(height)}`, () => drive.evaluate(`(async () => {
      const chips = [...document.querySelectorAll('.lc-prlink[data-issue-state]')]
      chips.at(-1)?.scrollIntoView({ block: 'center' })
      await new Promise((r) => setTimeout(r, 300))
      return JSON.stringify(chips.map((chip) => { const box = chip.getBoundingClientRect(); return { word: chip.innerText.trim(), width: Math.round(box.width), height: Math.round(box.height), wraps: chip.scrollWidth > chip.clientWidth + 1 } }))
    })()`))))
    check(`at ${String(width)} x ${String(height)}, each chip is one line and whole`,
      at.length === ISSUES.length && at.every((chip) => chip.width > 0 && chip.height > 0 && chip.height < 30 && !chip.wraps), JSON.stringify(at))
  }
  await drive.resize(1440, 900)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. cli/cli #136, #2 and #382 named in one message to Wren.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
