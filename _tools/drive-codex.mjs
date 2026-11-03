// A full session on Codex CLI: an edit, a follow-up that builds on it, the
// diff in the fold, the conversation in the sidebar, the archive screen.
//
//   node _tools/drive-codex.mjs [--model <codex model id>]
//
// --model (0.864): Wren on a named model rather than the account's default, so the follow-up reopens a thread
// recorded on it -- where Codex warned "resuming with <the person's default>" until the thread was reopened on the
// turn's own model. The drive then checks the conversation shows no such warning.
//
// Wren on Codex CLI (account-default), Accept edits. Two turns of one
// conversation. Spends two short Codex runs on Colin's account.

import { readFile } from 'node:fs/promises'
import { join } from 'node:path'

import { say, scratchRepository, sendAndWaitScript, startDrive, teammateFace } from './drive-lib.mjs'

const modelArg = process.argv.includes('--model') ? process.argv[process.argv.indexOf('--model') + 1] : undefined
const workspace = await scratchRepository('locust-drive-codex-ws-')
const drive = await startDrive({
  spends: true,
  name: 'codex',
  port: 9304,
  workspace,
  seed: {
    schemaVersion: 1,
    teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-09-05T05:00:00.000Z', route: { runtime: 'codex', model: modelArg ?? 'account-default', mode: 'accept-edits' } }],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' }
  }
})
const fold = `(async () => {
  const fold = document.querySelector('.lc-activity')
  if (!fold) return 'no activity fold'
  // The is-open class is never set on the card. ActivityCard sets
  // aria-expanded on the button and nothing else, so this guard never
  // guarded anything. Harmless while earlier folds were closed anyway;
  // since 0.49.0 a finished turn's fold opens itself and this CLOSED it.
  if (fold.getAttribute('aria-expanded') !== 'true') fold.click()
  await new Promise(r => setTimeout(r, 300))
  return fold.innerText.replace(/\\s+/g, ' ').slice(0, 60) + ' || ' + [...document.querySelectorAll('.lc-filerow')].map(r => r.innerText.replace(/\\s+/g, ' ').trim()).join(' | ').slice(0, 300)
})()`

try {
  await drive.capture('launch: Wren on Codex CLI, Accept edits', () => drive.ready())
  await drive.capture('turn 1: create NOTES.md', async () => {
    await drive.evaluate(`(async () => { ${teammateFace('Wren')}.click(); await new Promise(r => setTimeout(r, 500)) })()`)
    return drive.evaluate(sendAndWaitScript('Create a file named NOTES.md containing exactly two lines: "# Notes" and "First entry." Do not run shell commands. Reply with the single word DONE when it exists.'))
  })
  await drive.capture('the fold after turn 1', () => drive.evaluate(fold))
  await drive.capture('turn 2: a follow-up that builds on turn 1', () => drive.evaluate(sendAndWaitScript('Append one more line to NOTES.md: "Second entry." Keep the first two lines. Do not run shell commands. Reply with the single word DONE.')))
  await drive.capture('turn 2 says nothing about resuming on another model', () => drive.evaluate(`(() => {
    const said = document.querySelector('.lc-thread')?.innerText ?? ''
    return /resuming with/i.test(said) ? 'WARNED: ' + said.slice(said.search(/recorded with model/i), said.search(/recorded with model/i) + 160) : 'no resuming warning'
  })()`))
  await drive.capture('the fold after turn 2, and the sidebar', () => drive.evaluate(`(async () => {
    const folds = [...document.querySelectorAll('.lc-activity')]
    const last = folds[folds.length - 1]
    // The is-open class is never set on the card. ActivityCard sets
    // aria-expanded on the button and nothing else, so this guard never
    // guarded anything. Harmless while earlier folds were closed anyway;
    // since 0.49.0 a finished turn's fold opens itself and this CLOSED it.
    if (last && last.getAttribute('aria-expanded') !== 'true') last.click()
    await new Promise(r => setTimeout(r, 300))
    return folds.map(f => f.innerText.replace(/\\s+/g, ' ').slice(0, 50)).join(' / ') + ' || sidebar: ' + document.querySelector('.lc-sidebar').innerText.replace(/\\s+/g, ' ').slice(0, 200)
  })()`))
  await drive.capture('the header: cost and turns', () => drive.evaluate(`document.querySelector('.lc-workroom__header')?.innerText.replace(/\\s+/g, ' ').slice(0, 220) ?? ''`))
  await drive.capture('All missions (Ctrl 1): the archive', () => drive.evaluate(`(async () => {
    document.querySelector('button[title="All missions (Ctrl 1)"]').click()
    await new Promise(r => setTimeout(r, 800))
    return (document.querySelector('.lc-screen__title')?.innerText ?? '') + ' · ' + (document.querySelector('.lc-screen__meta')?.innerText ?? '') + ' || filters: ' + [...document.querySelectorAll('.lc-filter')].map(f => f.innerText.trim()).join('/') + ' || rows: ' + [...document.querySelectorAll('.lc-missionrow')].map(r => r.innerText.replace(/\\s+/g, ' ').slice(0, 120)).join(' | ')
  })()`))
  await drive.capture('filter Completed, then open the first row', () => drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('.lc-filter')].find(f => f.innerText.trim() === 'Completed')?.click()
    await new Promise(r => setTimeout(r, 400))
    const rows = document.querySelectorAll('.lc-missionrow').length
    document.querySelector('.lc-missionrow')?.click()
    await new Promise(r => setTimeout(r, 800))
    return 'completed rows: ' + rows + ' || opened: ' + (document.querySelector('.lc-workroom__header')?.innerText.replace(/\\s+/g, ' ').slice(0, 160) ?? 'no workroom')
  })()`))
  const notes = await readFile(join(workspace, 'NOTES.md'), 'utf8').catch(() => 'absent')
  drive.record.push({ step: drive.record.length + 1, title: 'disk after the session', note: `NOTES.md: ${JSON.stringify(notes)}`, errors: [] })
} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: 'Build: whatever `pnpm build` last wrote to out/. Wren on Codex CLI, Accept edits; two turns of one conversation, then the archive screen.' })
}
