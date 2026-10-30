// The everyday profile the fresh-eyes drives share (areas 8 on).
//
// Sixteen conversations over six weeks, four teammates and two with nobody
// on them; one conversation is three turns long, and only its SECOND turn
// says "signup". Written with the mission store's own writer and Codex's own
// normalizer, as drive-old-conversation does, so every record is one this
// build accepts. Sends nothing.

import { mkdtemp } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, scratchRepository } from './drive-lib.mjs'

const hex24 = (n) => (String(n) + 'a'.repeat(24)).slice(0, 24)
export const TEAM = [
  { name: 'Wren', hue: 'lime', role: 'Code & Migrations' },
  { name: 'Atlas', hue: 'blue', role: 'Research & Briefs' },
  { name: 'Marlow', hue: 'violet', role: 'Ops & Scheduling' },
  { name: 'Quill', hue: 'clay', role: 'Docs & QA' }
].map((t, i) => ({ teammateId: `tm_${hex24(i + 1)}`, ...t, createdAt: '2026-08-10T09:00:00.000Z' }))

// [owner, hours ago, turns (prompt, reply)...]
export const CONVERSATIONS = [
  ['Wren', 0.1, ['Fix the login redirect loop on the account page', 'The loop came from the session check running before the cookie was set. Moved it after.'], ['Also check the signup form does the same thing', 'It did. Same fix applied to the signup form.'], ['Good, ship it', 'Committed as "Stop the login and signup redirect loops".']],
  ['Atlas', 1, ['Summarize the three vendor quotes for the new CRM and recommend one', 'HubSpot Starter is cheapest for five seats; I recommend it.']],
  ['Marlow', 2, ["Draft next week's shift schedule around Maria's time off", 'Drafted. Tuesday and Wednesday are covered by moving the afternoon shift.']],
  ['Wren', 3, ['Why is the build 40 seconds slower since Tuesday?', 'Source maps were turned on for production builds in Tuesday’s change.']],
  ['Quill', 5, ['Proofread the README and fix anything unclear', 'Fixed six sentences; the install section now lists Node first.']],
  ['Atlas', 20, ['What changed in the Q3 sales tax rules for Florida?', 'Nothing for your categories; the change was to commercial rent.']],
  ['Wren', 26, ['Rename the billing module to payments across the repo', 'Renamed in 41 files. Tests pass.']],
  ['Marlow', 30, ['Reconcile the September invoices against the bank export', 'Two invoices have no matching deposit: #1043 and #1051.']],
  ['Wren', 50, ['Write tests for the invoice PDF export', 'Added eight tests; one found a rounding bug in the totals.']],
  ['Quill', 72, ['Turn the release notes into a customer email', 'Drafted a 120-word email with the three changes customers will notice.']],
  ['Atlas', 96, ['Compare Notion, Coda and Airtable for a 5-person team', 'Notion for docs, Airtable for anything that is really a table.']],
  ['Atlas', 144, ['Read every customer interview note in the interviews folder, pull out each complaint about onboarding, group them by theme, and tell me which three themes come up most often with a quote for each', 'Top three: too many setup steps, unclear pricing, and no sample data.']],
  ['Marlow', 216, ['Set up the Monday morning numbers summary', 'Saved as a routine that runs Mondays at 8.']],
  [undefined, 288, ['Quick question about git rebase', 'Rebase replays your commits on top of the other branch.']],
  ['Quill', 360, ['Make the pricing page copy shorter', 'Cut it from 310 words to 140.']],
  [undefined, 960, ['What does this folder do?', 'It is a small web shop with a Node backend.']]
]

/**
 * A scratch workspace and a profile whose ledger holds the everyday list.
 * Returns what `startDrive` needs and the teammates.json seed.
 */
export async function seedEverydayLedger(prefix) {
  const root = localPath(new URL('..', import.meta.url))
  const adapters = await import(pathToFileURL(join(root, 'packages', 'runtime-adapters', 'dist', 'index.js')).href)
  const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
  const workspace = await scratchRepository(`locust-drive-${prefix}-ws-`)
  const profilePath = await mkdtemp(join(tmpdir(), `locust-drive-${prefix}-profile-`))
  const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
  const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
  const who = Object.fromEntries(TEAM.map((t) => [t.name, t.teammateId]))
  const now = Date.now()
  const missionOwners = {}
  let made = 0
  for (const [c, [owner, hoursAgo, ...turns]] of CONVERSATIONS.entries()) {
    let previous
    for (const [t, [prompt, reply]] of turns.entries()) {
      const missionId = `mission_5e000000-0000-4000-8000-${String(c).padStart(4, '0')}${String(t).padStart(8, '0')}`
      const runId = `run_5e${String(c).padStart(4, '0')}${String(t)}`
      // Turns of one conversation are two minutes apart, the newest last.
      const at = new Date(now - hoursAgo * 3_600_000 - (turns.length - 1 - t) * 120_000).toISOString()
      await ledger.createMission({
        missionId, runId, prompt,
        runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: null,
        workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at,
        ...(previous === undefined ? {} : { continuesFrom: { missionId: previous, checkpointEpoch: 1, reason: 'follow-up' } })
      })
      // A real run takes time and reports tokens: 20s to about a minute and
      // a half, and a few thousand tokens in, so the time and usage columns
      // read as a person's would.
      const seconds = 20 + ((c * 7 + t * 13) % 70)
      const ended = new Date(Date.parse(at) + seconds * 1000).toISOString()
      let tick = 0
      const normalizer = adapters.createCodexEventNormalizer({ runId, missionId, requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: '0.156.1', now: () => new Date(Date.parse(at) + Math.min(seconds, (tick++) * (seconds / 3)) * 1000) })
      await ledger.appendEvents(missionId, [
        ...normalizer.accept({ sequence: 1, raw: JSON.stringify({ type: 'thread.started', thread_id: `thread_${missionId}` }) }),
        ...normalizer.accept({ sequence: 2, raw: JSON.stringify({ type: 'item.completed', item: { id: 'answer', type: 'agent_message', text: reply } }) }),
        ...normalizer.accept({ sequence: 3, raw: JSON.stringify({ type: 'turn.completed', usage: { input_tokens: 3000 + c * 911, cached_input_tokens: 0, output_tokens: 200 + t * 90 + c * 17 } }) }),
        ...normalizer.finish({ exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 3, cancelled: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, startedAt: at, finishedAt: ended })
      ])
      if (owner !== undefined) missionOwners[missionId] = who[owner]
      previous = missionId
      made += 1
    }
  }
  return {
    workspace,
    profilePath,
    missions: made,
    conversations: CONVERSATIONS.length,
    seed: { schemaVersion: 1, teammates: TEAM, missionOwners, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false } }
  }
}
