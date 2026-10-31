// A Kilo follow-up is answered (0.845): two messages to a teammate on Kilo's Auto Free model, the second a reply in
// the same conversation. Before 0.845 every run had a Kilo data folder of its own, so the reply's Kilo could not load
// the session it continued and the turn failed with "Internal error: OpenCode service failure" (found filming the
// launch clip, 2026-10-31). Free model only: nothing is spent.
//
//   node _tools/drive-a-kilo-follow-up-is-answered.mjs
//
// The checks: both turns are answered and neither failed; the reply loaded the first turn's session (its record
// names it); and both ran in one folder in the profile, kilo-conversations/<first turn>, which holds Kilo's database.

import { access, readdir, readFile } from 'node:fs/promises'
import { join } from 'node:path'

import { openTeammateScript, say, scratchRepository, sendAndWaitScript, startDrive } from './drive-lib.mjs'

const workspace = await scratchRepository('locust-drive-kilo-follow-up-')
const drive = await startDrive({
  name: 'kilo-follow-up',
  port: 9349,
  workspace,
  holdCursor: false,
  seed: {
    schemaVersion: 1,
    teammates: [{ teammateId: 'tm_kit', name: 'Kit', hue: 'clay', role: 'Custom', roleTitle: 'Writer', createdAt: '2026-10-31T12:00:00.000Z', route: { runtime: 'kilo', model: 'kilo/kilo-auto/free', mode: 'ask' } }],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' }
  }
})

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 500)}`}`)
}
const THREAD = `(document.querySelector('.lc-thread')?.innerText ?? '').replace(/\\s+/g, ' ')`

try {
  await drive.capture('launch', () => drive.ready())
  const opened = String(await drive.evaluate(openTeammateScript('Kit')))
  check('Kit opens', !/^no face/.test(opened), opened)
  await drive.waitFor(`(document.querySelector('form.command-dock textarea')?.getAttribute('placeholder') ?? '').startsWith('Message Kit')`, { timeoutMs: 20_000 })

  const first = String(await drive.capture('the first message, answered', () => drive.evaluate(sendAndWaitScript('Name one fruit that is yellow. Answer with the fruit only.', { waitSeconds: 120 }))))
  check('the first turn is answered', /^finished: /.test(first) && !/Internal error|stopped with an error/i.test(first), first)

  const second = String(await drive.capture('the reply, answered', () => drive.evaluate(sendAndWaitScript('Now name one fruit that is red. Answer with the fruit only.', { waitSeconds: 120 }))))
  const after = second.slice(second.lastIndexOf('fruit that is red'))
  check('the reply is answered, not failed', /^finished: /.test(second) && !/Internal error|stopped with an error|service failure/i.test(after) && after.replace(/Answer with the fruit only\./, '').trim().length > 3, after)

  // The record: the reply continues the first turn and names the session it loaded.
  const ledger = join(drive.profile, 'mission-ledger')
  const missions = []
  for (const file of (await readdir(ledger)).filter((name) => name.endsWith('.jsonl'))) {
    const created = JSON.parse((await readFile(join(ledger, file), 'utf8')).split('\n')[0])
    missions.push(created.metadata)
  }
  const opening = missions.find((one) => one.continuesFrom === undefined)
  const reply = missions.find((one) => one.continuesFrom !== undefined)
  check('the reply continues the first turn and names the session it loaded', opening !== undefined && reply?.continuesFrom?.missionId === opening.missionId && typeof reply.continuesFrom.runtimeThreadId === 'string', JSON.stringify(reply?.continuesFrom ?? null))
  const folder = join(drive.profile, 'kilo-conversations', opening?.missionId ?? 'none')
  const held = await access(join(folder, 'kilo', 'kilo.db')).then(() => true, () => false)
  check('both turns ran in the first turn\'s folder in the profile, which holds Kilo\'s database', held, folder)
  say(`thread: ${String(await drive.evaluate(THREAD)).slice(-300)}`)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: 'Kit on Kilo Auto Free, in Ask: a message and a reply in the same conversation.', extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
