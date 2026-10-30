// "Ask about this" sits below the words chosen, not over the line above (0.523).
//
//   node _tools/drive-ask-about-sits-below.mjs [--packaged <exe>] [--tag <name>]
//
// Sol: the button covered the line above the selection -- the line being
// read. ChatGPT and Claude put theirs below. A seeded reply of three lines;
// the middle one is dragged across with the real mouse. The button must sit
// below the selection and cover no word of the reply. Sends nothing.

import { createHash } from 'node:crypto'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, recordRoot, say, scratchRepository, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'

const root = localPath(new URL('..', import.meta.url))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-ask-below-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-ask-below-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const ASH = { teammateId: 'tm_ash000000000000000000', name: 'Ash', hue: 'clay', role: 'Custom', roleTitle: 'Helper', createdAt: '2026-09-05T05:00:00.000Z' }
const missionId = 'mission_5e000000-0000-4000-8000-000a00000000'
const runId = 'run_5e000a'
const at = new Date(Date.now() - 600_000).toISOString()
await ledger.createMission({
  missionId, runId, prompt: 'Describe the town in three short paragraphs.',
  runtime: 'opencode', model: 'opencode/mimo-v2.6-flash-free', requestedRouteId: 'opencode', resolvedRouteId: 'opencode-account:default', cliVersion: null,
  workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at
})
const event = (sequence, type, payload) => ({ id: `event_${String(sequence)}`, runId, missionId, sequence, occurredAt: at, sourceAdapter: 'opencode', type, payload: { ...payload, evidence: { redacted: true } } })
await ledger.appendEvents(missionId, [
  event(1, 'message.delta', { itemId: 'answer', operation: 'append', text: 'The river is wide.\n\nThe bridge is old.\n\nThe town is quiet.', final: true }),
  event(2, 'run.completed', { usage: { inputTokens: 900, outputTokens: 20 }, process: { exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 2, inputDeliveryFailed: false, outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: at, finishedAt: at } })
])
await ledger.flush?.()

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `ask-below-${tag}`,
  port: 9846,
  workspace,
  profilePath,
  sendsNothing: true,
  outPath: join(recordRoot('ask-about-sits-below-2026-10-01'), tag),
  seed: { schemaVersion: 1, teammates: [ASH], missionOwners: { [missionId]: ASH.teammateId }, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 400)}`}`)
}
const mouse = (type, x, y, extra = {}) => drive.send('Input.dispatchMouseEvent', { type, x, y, button: 'left', clickCount: 1, ...extra })

try {
  await drive.capture('launch', () => drive.ready())
  await drive.resize(1440, 900)
  const place = JSON.parse(String(await drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('.lc-convrow button.lc-conv')].find((r) => /Describe the town/.test(r.innerText))?.click()
    await new Promise((r) => setTimeout(r, 1500))
    const body = [...document.querySelectorAll('main .lc-agentline__body')].at(-1)
    if (!body) return JSON.stringify({ found: false })
    const walker = document.createTreeWalker(body, NodeFilter.SHOW_TEXT)
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      const index = node.textContent.indexOf('The bridge is old')
      if (index < 0) continue
      const range = document.createRange()
      range.setStart(node, index)
      range.setEnd(node, index + 'The bridge is old'.length)
      const box = range.getBoundingClientRect()
      return JSON.stringify({ found: true, x1: box.left + 1, x2: box.right - 1, y: box.top + box.height / 2, top: box.top, bottom: box.bottom })
    }
    return JSON.stringify({ found: false })
  })()`)))
  check('the seeded reply holds the line to select', place.found === true, JSON.stringify(place))
  await mouse('mouseMoved', place.x1, place.y, { button: 'none', clickCount: 0 })
  await mouse('mousePressed', place.x1, place.y)
  await mouse('mouseMoved', place.x2, place.y, { buttons: 1 })
  await mouse('mouseReleased', place.x2, place.y)
  const button = JSON.parse(String(await drive.capture('selected: the button', () => drive.evaluate(`(async () => {
    await new Promise((r) => setTimeout(r, 400))
    const b = document.querySelector('.lc-selectionask')
    const box = b?.getBoundingClientRect()
    // Every line of the reply the button would cover.
    const body = [...document.querySelectorAll('main .lc-agentline__body')].at(-1)
    // The other two lines' own text boxes, measured by range: a reply can be one element.
    const lineBox = (words) => {
      const walker = document.createTreeWalker(body, NodeFilter.SHOW_TEXT)
      for (let node = walker.nextNode(); node; node = walker.nextNode()) {
        const index = node.textContent.indexOf(words)
        if (index < 0) continue
        const range = document.createRange()
        range.setStart(node, index)
        range.setEnd(node, index + words.length)
        return range.getBoundingClientRect()
      }
      return undefined
    }
    const covered = box === undefined ? [] : ['The river is wide', 'The town is quiet'].filter((words) => {
      const r = lineBox(words)
      return r !== undefined && r.bottom > box.top && r.top < box.bottom && r.right > box.left && r.left < box.right
    })
    return JSON.stringify({ selected: window.getSelection()?.toString() ?? '', top: box?.top ?? null, covered })
  })()`))))
  check('the words are selected and the button shows', /The bridge is old/.test(button.selected) && button.top !== null, JSON.stringify(button))
  check('it sits beside them, on their line', button.top !== null && button.top < place.bottom && button.top + 30 > place.top, JSON.stringify({ button: button.top, selection: [place.top, place.bottom] }))
  check('and covers neither the line above nor the line below', button.covered.length === 0, JSON.stringify(button.covered))
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. A seeded reply of three lines; the middle one selected.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
