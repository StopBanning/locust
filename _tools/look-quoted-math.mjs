// A reply with a quote and the math inside it, drawn, to be looked at (0.713).
//
//   node _tools/look-quoted-math.mjs [--packaged <exe>] [--size WxH]
//
// The shape of the Codex answer a tester read as raw TeX on 0.712: prose with
// inline math, then "a version you could write down" as a quote whose
// equations open with `\[` on a line of their own. One conversation, written
// with the mission store's own writer, as the everyday profile is. Sends
// nothing; the frames are read.

import { mkdtemp } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const [width, height] = (arg('--size') ?? '1209x770').split('x').map(Number)

const REPLY = [
  'Each entry of a product is a row of the first matrix times a column of the second, so split the sum where the blocks meet.',
  '',
  'For $1 \\le i, j \\le n$ the first $n$ terms come from $A$ and $E$:',
  '',
  '\\[',
  '\\sum_{k=1}^{n} M_{ik}N_{kj} = \\sum_{k=1}^{n} A_{ik}E_{kj} = (AE)_{ij}.',
  '\\]',
  '',
  '**A version you could write down:**',
  '',
  '> Let \\(M=\\begin{bmatrix}A&B\\\\C&D\\end{bmatrix}\\) and \\(N=\\begin{bmatrix}E&F\\\\G&H\\end{bmatrix}\\). For \\(1\\le i,j\\le n\\),',
  '> \\[',
  '> (MN)_{ij}',
  '> =',
  '> \\sum_{k=1}^{n}A_{ik}E_{kj}',
  '> +',
  '> \\sum_{k=1}^{n}B_{ik}G_{kj}',
  '> =',
  '> (AE+BG)_{ij}.',
  '> \\]',
  '> The other three blocks follow the same way, so',
  '> \\[',
  '> MN=',
  '> \\begin{bmatrix}',
  '> AE+BG&AF+BH\\\\',
  '> CE+DG&CF+DH',
  '> \\end{bmatrix}.',
  '> \\]',
  '',
  'Check the units before you divide, and note that $AB \\neq BA$ in general.'
].join('\n')

const root = localPath(new URL('..', import.meta.url))
const adapters = await import(pathToFileURL(join(root, 'packages', 'runtime-adapters', 'dist', 'index.js')).href)
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-look-math-ws-')
const profilePath = await mkdtemp(join(tmpdir(), 'locust-look-math-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const missionId = 'mission_5e000000-0000-4000-8000-000000000001'
const runId = 'run_5e0001'
const at = new Date(Date.now() - 600_000).toISOString()
await ledger.createMission({
  missionId, runId, prompt: 'Explain how to do the block matrix problem, and give me a version I could write down.',
  runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: null,
  workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at
})
let tick = 0
const normalizer = adapters.createCodexEventNormalizer({ runId, missionId, requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: '0.156.1', now: () => new Date(Date.parse(at) + (tick++) * 9000) })
await ledger.appendEvents(missionId, [
  ...normalizer.accept({ sequence: 1, raw: JSON.stringify({ type: 'thread.started', thread_id: `thread_${missionId}` }) }),
  ...normalizer.accept({ sequence: 2, raw: JSON.stringify({ type: 'item.completed', item: { id: 'answer', type: 'agent_message', text: REPLY } }) }),
  ...normalizer.accept({ sequence: 3, raw: JSON.stringify({ type: 'turn.completed', usage: { input_tokens: 4100, cached_input_tokens: 0, output_tokens: 610 } }) }),
  ...normalizer.finish({ exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 3, cancelled: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, startedAt: at, finishedAt: new Date(Date.parse(at) + 27_000).toISOString() })
])

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: 'look-quoted-math',
  port: 9873,
  workspace,
  profilePath,
  sendsNothing: true,
  outPath: join(recordRoot('look-quoted-math-2026-10-09'), `${packaged === undefined ? 'local' : 'packaged'}-${String(width)}x${String(height)}`),
  seed: { schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false } }
})
try {
  await drive.ready()
  await drive.resize(width, height)
  await sleep(1500)
  await drive.capture('the conversation', () => drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('.lc-convrow button.lc-conv')].find((r) => /block matrix/i.test(r.innerText))?.click()
    await new Promise((r) => setTimeout(r, 1800))
    const thread = document.querySelector('.lc-thread')
    return JSON.stringify({
      quotes: thread?.querySelectorAll('blockquote.lc-quote').length ?? 0,
      displayed: thread?.querySelectorAll('.katex-display').length ?? 0,
      rawTex: /\\\\begin\\{bmatrix\\}|\\\\sum_/.test(thread?.innerText ?? '')
    })
  })()`))
  await drive.capture('the quote, scrolled into view', () => drive.evaluate(`(async () => {
    document.querySelector('blockquote.lc-quote')?.scrollIntoView({ block: 'center' })
    await new Promise((r) => setTimeout(r, 600))
    const quote = document.querySelector('blockquote.lc-quote')?.getBoundingClientRect()
    const inQuote = [...document.querySelectorAll('blockquote.lc-quote p, blockquote.lc-quote .katex-display, blockquote.lc-quote .lc-math')].map((n) => n.tagName + '.' + n.className.split(' ')[0] + ' ' + getComputedStyle(n).color)
    return quote === undefined ? 'no quote' : JSON.stringify({ top: Math.round(quote.top), height: Math.round(quote.height), width: Math.round(quote.width), colours: inQuote })
  })()`))
  say('captured')
} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. A reply with a quote holding displayed equations, at ${String(width)}x${String(height)}.`, extra: '' })
}
