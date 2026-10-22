// An OpenCode edit in Approve each: is the change on the card, and does a
// decline read as declined? (Fresh-profile beta report of 0.345, findings 1, 3.)
//
//   node _tools/drive-opencode-approve-edit.mjs [--packaged <exe>] [--tag <label>]
//
// The report: the card said "Change files" and a long absolute path, with no
// proposed content; a declined edit's row read "edit failed" beside "1
// refused". Wren on the free model, Approve each, asked to edit one line: the
// first card is read and declined, then the same edit is asked again and
// approved.
//
// Free model only.

import { readFile } from 'node:fs/promises'
import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import { FREE_ROUTE, openTeammateScript, say, scratchRepository, startDrive, recordRoot } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag')
const outPath = tag === undefined ? undefined : join(recordRoot('beta-fixes-2026-09-24'), `opencode-approve-edit-${tag}`)
if (outPath !== undefined) await mkdir(outPath, { recursive: true })

const workspace = await scratchRepository('locust-drive-oc-edit-ws-')
await writeFile(join(workspace, 'notes.txt'), 'Status: draft\n', 'utf8')
const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  ...(outPath === undefined ? {} : { outPath }),
  name: 'opencode-approve-edit',
  port: 9323,
  workspace,
  seed: {
    schemaVersion: 1,
    teammates: [
      { teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-09-25T05:00:00.000Z', route: { runtime: 'opencode', model: FREE_ROUTE.model, mode: 'accept-edits' } }
    ],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false }
  }
})

const MODE = `(async () => {
  const control = [...document.querySelectorAll('.lc-control')].find((b) => /^(Ask|Edit|Accept edits|Plan|Approve|Auto)\\b/.test(b.innerText))
  control.click(); await new Promise((r) => setTimeout(r, 400))
  const approve = [...document.querySelectorAll('[role=menuitemradio]')].find((b) => /^Approve each/.test(b.innerText.trim()))
  if (!approve || approve.disabled) { control.click(); return 'APPROVE EACH NOT AVAILABLE' }
  approve.click(); await new Promise((r) => setTimeout(r, 400))
  return 'mode: ' + control.innerText.split(/\\s+/).join(' ').trim()
})()`

const ask = (text, answer) => `(async () => {
  const field = document.querySelector('form.command-dock textarea')
  Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set.call(field, ${JSON.stringify(text)})
  field.dispatchEvent(new Event('input', { bubbles: true }))
  for (let i = 0; i < 120; i += 1) {
    await new Promise((r) => setTimeout(r, 250))
    const button = document.querySelector('button[aria-label="Send"]')
    if (button && !button.disabled) { button.click(); break }
  }
  const cards = []
  for (let i = 0; i < 900; i += 1) {
    await new Promise((r) => setTimeout(r, 400))
    const approval = document.querySelector('[role=group][aria-label="Approval required"]')
    if (approval) {
      await new Promise((r) => setTimeout(r, 400))
      const seen = approval.innerText.split(/\\s+/).join(' ')
      cards.push((/Status: ready/.test(seen) ? '[SHOWS THE PROPOSED LINE] ' : '[NO PROPOSED CONTENT] ') + seen.slice(0, 160))
      const button = ${answer === 'approve'
        ? `[...approval.querySelectorAll('button')].find((b) => /^Approve once/.test(b.innerText.trim()))`
        : `approval.querySelector('button.lc-denybutton')`}
      if (!button) break
      button.click()
      await new Promise((r) => setTimeout(r, 900))
      continue
    }
    if (i > 8 && !document.querySelector('button[aria-label^="Stop the running"]')) break
  }
  await new Promise((r) => setTimeout(r, 900))
  const fold = document.querySelector('.lc-activity')
  if (fold && fold.getAttribute('aria-expanded') !== 'true') { fold.click(); await new Promise((r) => setTimeout(r, 400)) }
  return 'cards: ' + (cards.length === 0 ? 'none' : cards.join(' ;; ')) + ' || fold: ' + (fold?.innerText ?? 'none').split(/\\s+/).join(' ').slice(0, 260)
})()`

const EDIT = 'Use your edit tool to change the line Status: draft in notes.txt so it reads Status: ready. Do not use the shell. If the edit is declined, stop and say so in one sentence.'

try {
  await drive.capture('Approve each on the free model', async () => {
    await drive.ready()
    await drive.evaluate(openTeammateScript('Wren'))
    return drive.evaluate(MODE)
  })
  await drive.capture('the edit asked, the card read, and declined', () => drive.evaluate(ask(EDIT, 'deny')))
  await drive.capture('notes.txt after the decline', async () => `notes.txt: ${JSON.stringify((await readFile(join(workspace, 'notes.txt'), 'utf8')).trim())}`)
  await drive.capture('the same edit asked again, and approved', () => drive.evaluate(ask(EDIT.replace('If the edit is declined, stop and say so in one sentence.', 'It is allowed this time.'), 'approve')))
  await drive.capture('notes.txt after the approval', async () => `notes.txt: ${JSON.stringify((await readFile(join(workspace, 'notes.txt'), 'utf8')).trim())}`)
} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'whatever pnpm build last wrote to out/'}. Wren on OpenCode / ${FREE_ROUTE.model}, Approve each; one line of notes.txt, declined then approved.` })
}
