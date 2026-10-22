// A new conversation whose message names a file inside a project starts in that project (0.814, named-folder.ts).
//
//   node _tools/drive-a-message-that-names-its-project-starts-there.mjs [--packaged <Locust.exe>]
//
// Colin, 2026-10-22, on handing work to his teammates: "its honestly a pain to have to switch it to some random
// fucking folder everytime so i just ignore you and send them the handoff". The window's folder is a new, empty folder
// inside ~/.config (one where agents keep their own settings, like .claude). A repository `garden` with a worktree
// `pea`, and an unrelated repository `orchard`, sit in a temporary folder. Then:
//   1. "Read <pea>\docs\BRIEF.md ..." starts in pea, with no folder question, and the conversation says so.
//   2. From pea, a new conversation naming a file in garden (the same repository) starts in garden.
//   3. From garden, a new conversation naming a file in orchard (another repository) stays in garden.
// Spends nothing: the free model, read only.

import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, readdir, readFile, rm, writeFile } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { join } from 'node:path'

import { FREE_ROUTE, openTeammateScript, say, sendAndWaitScript, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}
const git = (cwd, ...args) => execFileSync('git', ['-c', 'user.name=Locust drive', '-c', 'user.email=locust-drive', ...args], { cwd, stdio: 'pipe' })
const idOf = (path) => `ws_${createHash('sha256').update(path, 'utf8').digest('hex').slice(0, 32)}`

const base = await mkdtemp(join(tmpdir(), 'locust-drive-named-'))
const garden = join(base, 'garden')
const pea = join(base, 'garden-wt', 'pea')
const orchard = join(base, 'orchard')
await mkdir(join(garden, 'docs'), { recursive: true })
await writeFile(join(garden, 'docs', 'PLAN.md'), 'PLAN-WORD: BASIL-3\n')
git(garden, 'init', '-q')
git(garden, 'add', '-A')
git(garden, 'commit', '-q', '-m', 'A plan')
git(garden, 'worktree', 'add', '-q', '-b', 'exec/pea', pea)
await writeFile(join(pea, 'docs', 'BRIEF.md'), 'BRIEF-WORD: PEPPERMINT-4\n')
await mkdir(orchard, { recursive: true })
await writeFile(join(orchard, 'README.md'), 'ORCHARD-WORD: QUINCE-9\n')
git(orchard, 'init', '-q')

const settings = join(homedir(), '.config')
await mkdir(settings, { recursive: true })
const workspace = await mkdtemp(join(settings, 'locust-drive-named-ws-'))
const mate = (teammateId, name, hue, minute) => ({ teammateId, name, hue, role: 'Research & Briefs', createdAt: `2026-10-22T00:0${String(minute)}:00.000Z`, route: { ...FREE_ROUTE, mode: 'ask' } })
const drive = await startDrive({
  name: 'a-message-that-names-its-project-starts-there',
  port: 9851,
  workspace,
  launchElsewhere: true,
  ...(packaged === undefined ? {} : { packaged }),
  seed: {
    schemaVersion: 1,
    teammates: [mate('tm_ash', 'Ash', 'clay', 0), mate('tm_iris', 'Iris', 'violet', 1), mate('tm_cedar', 'Cedar', 'lime', 2)],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false }
  }
})
await drive.ready()
await drive.resize(1209, 770)

/** Each conversation's folder, as its record keeps it, oldest first. */
const folders = async () => {
  const dir = join(drive.profile, 'mission-ledger')
  const files = (await readdir(dir).catch(() => [])).filter((file) => file.endsWith('.jsonl'))
  const kept = []
  for (const file of files) {
    const text = await readFile(join(dir, file), 'utf8').catch(() => '')
    const id = /"workspaceId":"(ws_[0-9a-f]+)"/.exec(text)?.[1]
    const at = /"createdAt":"([^"]+)"/.exec(text)?.[1] ?? ''
    if (id !== undefined) kept.push({ id, at })
  }
  return kept.sort((a, b) => a.at.localeCompare(b.at)).map((entry) => entry.id)
}
const THREAD = `document.querySelector('.lc-thread')?.innerText.replace(/\\s+/g, ' ') ?? ''`
const CHIP = `document.querySelector('.lc-control--folder')?.innerText.replace(/\\s+/g, ' ').trim() ?? ''`
const QUESTION = `document.querySelector('.lc-folderask')?.innerText ?? ''`
const ask = async (name, message) => {
  const opened = await drive.evaluate(openTeammateScript(name))
  const sent = await drive.evaluate(sendAndWaitScript(message, { waitSeconds: 240 }))
  return { opened: String(opened), sent: String(sent), thread: String(await drive.evaluate(THREAD)), chip: String(await drive.evaluate(CHIP)), question: String(await drive.evaluate(QUESTION)) }
}
const wsName = workspace.split(/[\\/]/).pop()

try {
  const first = await drive.capture('a brief inside pea, sent from a settings folder', () => ask('Ash', `Read ${join(pea, 'docs', 'BRIEF.md')} and reply with the word after BRIEF-WORD, alone.`))
  check('no folder question: the message named its project', first.question === '', first.question.slice(0, 120))
  check('the conversation says where it went', first.thread.includes(`Working in pea, the project your message names, instead of ${wsName}.`), first.thread.slice(-400))
  check('it ran in pea, by its record', (await folders()).at(0) === idOf(pea), JSON.stringify(await folders()))
  check('and read the brief there', first.thread.includes('PEPPERMINT-4'), first.thread.slice(-200))
  check('the folder chip is pea now', /pea/.test(first.chip), first.chip)

  const second = await drive.capture('from pea, a file in garden, the same repository', () => ask('Iris', `Read ${join(garden, 'docs', 'PLAN.md')} and reply with the word after PLAN-WORD, alone.`))
  check('it says it moved to garden', second.thread.includes('Working in garden, the project your message names, instead of pea.'), second.thread.slice(-400))
  check('it ran in garden, by its record', (await folders()).at(1) === idOf(garden), JSON.stringify(await folders()))
  check('the folder chip is garden now', /garden/.test(second.chip), second.chip)

  const third = await drive.capture('from garden, a file in orchard, another repository', () => ask('Cedar', `What does ${join(orchard, 'README.md')} say? One line.`))
  check('it stays in garden and says nothing about folders', !third.thread.includes('the project your message names'), third.thread.slice(-300))
  check('it ran in garden, by its record', (await folders()).at(2) === idOf(garden), JSON.stringify(await folders()))
} catch (error) {
  failures += 1
  say(`  [FAIL] the drive stopped: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: 'A new conversation whose message names a file inside a project starts in that project (0.814).' })
  await rm(workspace, { recursive: true, force: true }).catch(() => undefined)
  await rm(base, { recursive: true, force: true }).catch(() => undefined)
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} FAILURE(S)`)
process.exit(failures === 0 ? 0 : 1)
