// Scout prepares the fix, and says what it saw (0.828, Releases 3 and 4 of "Scout comes to you first").
//
//   node _tools/drive-scout-prepares-the-fix.mjs [--packaged <exe>] [--tag <name>]
//
// Free models only: Scout and Ada both read and write with the free model, so nothing is spent. The folder is a git
// repository with a remote, whose check is `node check.js`, failing until answer.txt says 42. Then:
//   1. Settings > General: "Prepare fixes before I look" and "What Scout saw", at three window sizes.
//   2. Fixes on, by Ada, at most two a day. Look now: the check's card says Ada is preparing a fix.
//   3. The fix is made in a copy under the drive's profile: the copy has no remote, a push from it fails, and the
//      guard refuses git push and gh there. The card says the fix is ready, the check passes.
//   4. The person changes another file meanwhile. Apply: answer.txt says 42 in the folder, the person's own change
//      is still there, and the card says Applied.
//   5. What Scout saw: the looks, newest first, the check's card raised, and the filter for what it left out.
//   6. Asked in Scout's own conversation why it did not say something, Scout answers from the record, in Ask.

import { spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readdir, readFile, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import { APP_DIR, FREE_ROUTE, git, openTeammateScript, recordRoot, say, scratchRepository, sendAndWaitScript, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 400)}`}`)
}

const workspace = await scratchRepository('locust-drive-scout-fix-ws-')
await writeFile(join(workspace, 'check.js'), [
  "const said = require('fs').existsSync('answer.txt') ? require('fs').readFileSync('answer.txt', 'utf8').trim() : 'nothing'",
  "if (said !== '42') { console.log('CHECK FAILED: answer.txt must say 42, and it says ' + said); process.exit(1) }",
  "console.log('ok: answer.txt says 42')",
  ''
].join('\n'), 'utf8')
await writeFile(join(workspace, 'answer.txt'), '41\n', 'utf8')
await writeFile(join(workspace, 'notes.md'), '# Notes\n\nmine\n', 'utf8')
await git(['add', '-A'], workspace)
await git(['commit', '-q', '-m', 'an answer and its check'], workspace)
await git(['remote', 'add', 'origin', 'https://github.com/locust-drive/not-a-real-repo.git'], workspace)
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `scout-fix-${tag}`,
  port: 9874,
  workspace,
  outPath: join(recordRoot('scout-prepares-the-fix-2026-10-23'), tag),
  seed: {
    schemaVersion: 1,
    teammates: [{ teammateId: 'tm_ada', name: 'Ada', hue: 'violet', role: 'Code & Migrations', createdAt: '2026-10-23T05:00:00.000Z', route: FREE_ROUTE }],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false, checkCommands: { [workspaceId]: 'node check.js' } }
  }
})
const fixRoot = join(drive.profile, 'scout-fixes')

const state = async () => JSON.parse(String(await drive.evaluate('window.desktop.scout.state().then((s) => JSON.stringify(s))')))
const SECTION = `document.querySelector('[data-setting="scout"]')?.innerText.replace(/\\s+/g, ' ') ?? ''`
const OPEN_SETTINGS = `(async () => {
  if (!document.querySelector('.lc-settings')) document.querySelector('button[title="Settings (Ctrl 3)"]')?.click()
  for (let i = 0; i < 40 && !document.querySelector('.lc-settings'); i += 1) await new Promise((r) => setTimeout(r, 100))
  ;[...document.querySelectorAll('.lc-settings__navitem, button')].find((b) => b.innerText.trim() === 'General')?.click()
  for (let i = 0; i < 40 && !document.querySelector('[data-setting="scout"]'); i += 1) await new Promise((r) => setTimeout(r, 100))
  document.querySelector('[data-setting="scout"]')?.scrollIntoView({ block: 'start' })
  return document.querySelector('[data-setting="scout"]') ? 'open' : 'no Scout section'
})()`
const CLOSE_SETTINGS = `(() => { if (document.querySelector('.lc-settings')) document.querySelector('button[title="Settings (Ctrl 3)"]')?.click(); return 'closed' })()`
const CHECK_CARD = `(() => {
  const card = [...document.querySelectorAll('[data-scout-card]')].find((one) => (one.querySelector('.lc-scoutcard__said')?.innerText ?? '').startsWith('The check node check.js fails'))
  card?.scrollIntoView({ block: 'center' })
  return card ? card.innerText.replace(/\\s+/g, ' ') : 'no check card'
})()`
const lookAndWait = async () => {
  const before = (await state()).lastLookAt
  await drive.evaluate('window.desktop.scout.look()')
  for (let i = 0; i < 120; i += 1) {
    const now = await state()
    if (now.lastLookAt !== before && !now.cards.some((card) => card.reading === true)) return now
    await sleep(500)
  }
  return state()
}
const fixOf = async () => (await state()).cards.find((card) => card.key.endsWith(':fails'))?.fix
/** The hook itself, run in `cwd`, asked about a command: its answer, as a runtime reads it. */
const guardSays = (cwd, command) => new Promise((resolve) => {
  const guard = join(APP_DIR, 'resources', 'locust-command-guard.mjs')
  const child = spawn(process.execPath, [guard], { cwd, stdio: ['pipe', 'pipe', 'ignore'], windowsHide: true })
  let out = ''
  child.stdout.on('data', (chunk) => { out += String(chunk) })
  child.on('close', () => { resolve(out) })
  child.stdin.end(JSON.stringify({ hook_event_name: 'PreToolUse', tool_name: 'Bash', tool_input: { command } }))
})

try {
  await drive.ready()
  await drive.resize(1440, 900)
  const turned = JSON.parse(String(await drive.evaluate(`window.desktop.scout.turnOn(${JSON.stringify({ runtime: FREE_ROUTE.runtime, model: FREE_ROUTE.model })}).then((s) => JSON.stringify(s))`)))
  check('Scout is on, on the free model', turned?.settings?.on === true, JSON.stringify(turned).slice(0, 200))
  await drive.evaluate(`window.desktop.scout.set({ watches: { github: false, comments: false, teammates: true, check: false } })`)
  await lookAndWait()

  // 1. The two new lines, at three sizes.
  const shown = String(await drive.capture('settings: prepare fixes, and what Scout saw', async () => {
    say(`  ${String(await drive.evaluate(OPEN_SETTINGS))}`)
    await drive.evaluate(`document.querySelector('[aria-label="Prepare fixes before I look"]')?.scrollIntoView({ block: 'center' })`)
    return drive.evaluate(SECTION)
  }))
  check('Prepare fixes before I look is there, off, and asks for the check first', shown.includes('Prepare fixes before I look') && shown.includes('Turn on This folder’s check first.'), shown.slice(0, 300))
  check('What Scout saw is there', shown.includes('What Scout saw') && shown.includes('Show what Scout saw'), shown.slice(0, 300))
  await drive.resize(1120, 720)
  await drive.capture('settings at 1120 by 720', () => drive.evaluate(`${OPEN_SETTINGS}.then(() => document.querySelector('[aria-label="Prepare fixes before I look"]')?.scrollIntoView({ block: 'center' }))`).then(() => drive.evaluate(SECTION)))
  await drive.resize(860, 640)
  await drive.capture('settings at 860 wide', () => drive.evaluate(`${OPEN_SETTINGS}.then(() => document.querySelector('[aria-label="Prepare fixes before I look"]')?.scrollIntoView({ block: 'center' }))`).then(() => drive.evaluate(SECTION)))
  await drive.resize(1440, 900)

  // 2. Fixes on, through Settings as the person would: the check first, then the switch.
  await drive.evaluate(`window.desktop.scout.set({ watches: { github: false, comments: false, teammates: true, check: true } })`)
  await drive.evaluate(OPEN_SETTINGS)
  const pressed = String(await drive.evaluate(`(async () => {
    for (let i = 0; i < 20 && document.querySelector('[aria-label="Prepare fixes before I look"]')?.disabled; i += 1) await new Promise((r) => setTimeout(r, 150))
    const sw = document.querySelector('[aria-label="Prepare fixes before I look"]')
    if (!sw) return 'no switch'
    if (sw.disabled) return 'switch disabled'
    sw.click()
    return 'pressed'
  })()`))
  for (let i = 0; i < 20 && (await state()).settings.fixes?.on !== true; i += 1) await sleep(250)
  const fixes = (await state()).settings.fixes
  check('turned on in Settings, Ada prepares them, at most two a day', pressed === 'pressed' && fixes?.on === true && fixes.teammateId === 'tm_ada' && fixes.perDay === 2, `${pressed}; ${JSON.stringify(fixes)}`)
  await drive.capture('settings: fixes on, by Ada', async () => {
    await drive.evaluate(`document.querySelector('[aria-label="Prepare fixes before I look"]')?.scrollIntoView({ block: 'center' })`)
    return drive.evaluate(SECTION)
  })
  await drive.evaluate(CLOSE_SETTINGS)
  await lookAndWait()
  let fix = await fixOf()
  for (let i = 0; i < 20 && fix === undefined; i += 1) { await sleep(250); fix = await fixOf() }
  check('the check’s card says Ada is preparing a fix', fix?.state === 'preparing' && fix.name === 'Ada', JSON.stringify(fix))
  const preparing = String(await drive.capture('Home: Ada is preparing a fix', async () => { await sleep(600); return drive.evaluate(CHECK_CARD) }))
  check('and says so on the card', preparing.includes('Ada is preparing a fix in a copy of the folder…'), preparing)

  // 3. The copy, while the fix is made: its own repository, no remote, nothing pushes from it.
  const fixId = fix?.fixId
  const copy = fixId === undefined ? undefined : join(fixRoot, fixId)
  if (copy !== undefined) {
    for (let i = 0; i < 40 && !(await readdir(copy).then(() => true, () => false)); i += 1) await sleep(250)
    const remotes = await git(['remote'], copy).catch((error) => `failed: ${String(error)}`)
    check('the copy is under the drive’s own profile, with no remote', remotes.trim() === '', remotes)
    const pushed = await git(['push', 'origin', 'HEAD'], copy).then(() => 'pushed', (error) => `refused: ${String(error).slice(0, 160)}`)
    check('a push from the copy fails', pushed.startsWith('refused'), pushed)
    const denied = await guardSays(copy, 'git push origin main')
    const deniedGh = await guardSays(copy, 'gh pr create --fill')
    check('the guard refuses git push and gh in the copy', denied.includes('"permissionDecision":"deny"') && deniedGh.includes('"permissionDecision":"deny"'), `${denied.slice(0, 160)} | ${deniedGh.slice(0, 160)}`)
    check('and lets git push be in the person’s own folder', (await guardSays(workspace, 'git push origin main')) === '')
  }

  // 4. The person keeps working on another file while Ada prepares it.
  await writeFile(join(workspace, 'notes.md'), '# Notes\n\nmine, changed while the fix was made\n', 'utf8')
  for (let i = 0; i < 960 && (await fixOf())?.state === 'preparing'; i += 1) await sleep(500)
  fix = await fixOf()
  check('the fix is ready, and the check passes in the copy', fix?.state === 'ready' && fix.files.includes('answer.txt'), JSON.stringify(fix))
  const ready = String(await drive.capture('Home: the fix is ready', async () => { await sleep(600); return drive.evaluate(CHECK_CARD) }))
  check('the card says the fix is ready, with Review, Apply and Skip', /Fix ready: \d+ files? changed/.test(ready) && ready.includes('the check passes.') && ready.includes('Review') && ready.includes('Apply') && ready.includes('Skip'), ready)
  check('the folder is untouched before Apply', (await readFile(join(workspace, 'answer.txt'), 'utf8')).trim() === '41')
  if (fix?.state === 'ready') {
    const applied = String(await drive.evaluate(`(async () => {
      const card = [...document.querySelectorAll('[data-scout-card]')].find((one) => one.querySelector('[data-scout-fix="ready"]'))
      const apply = [...(card?.querySelectorAll('button') ?? [])].find((b) => b.innerText.trim() === 'Apply')
      if (!apply) return 'no Apply'
      apply.click()
      for (let i = 0; i < 40 && !card.querySelector('[data-scout-fix="applied"]'); i += 1) await new Promise((r) => setTimeout(r, 250))
      return card.querySelector('[data-scout-fix]')?.innerText.replace(/\\s+/g, ' ') ?? 'no fix line'
    })()`))
    check('Apply says it is in the folder, beside your own changes', applied.startsWith('Applied:') && applied.includes('beside your own changes'), applied)
    check('answer.txt says 42 in the folder', (await readFile(join(workspace, 'answer.txt'), 'utf8')).trim() === '42')
    check('the person’s own change to notes.md is still there', (await readFile(join(workspace, 'notes.md'), 'utf8')).includes('mine, changed while the fix was made'))
    await drive.capture('Home: the fix applied', async () => { await sleep(500); return drive.evaluate(`[...document.querySelectorAll('[data-scout-fix]')].map((one) => one.innerText).join(' | ')`) })
    check('the copy is gone once applied', !(await readdir(fixRoot).catch(() => [])).includes(fixId))
  }
  const today = (await state()).today
  check('today counts the fix', today?.fixes === 1, JSON.stringify(today))

  // 5. What Scout saw.
  const saw = String(await drive.capture('settings: what Scout saw', async () => {
    await drive.evaluate(`${OPEN_SETTINGS}.then(() => '')`)
    await drive.evaluate(`(async () => {
      const show = [...document.querySelectorAll('[data-setting="scout"] button')].find((b) => b.innerText.trim() === 'Show what Scout saw')
      show?.click()
      for (let i = 0; i < 40 && !document.querySelector('.lc-scoutlog, .lc-scoutlog + *'); i += 1) await new Promise((r) => setTimeout(r, 100))
      document.querySelector('.lc-scoutlog')?.scrollIntoView({ block: 'center' })
    })()`)
    await sleep(400)
    return drive.evaluate(`document.querySelector('.lc-scoutlog')?.innerText.replace(/\\s+/g, ' ') ?? 'no log'`)
  }))
  check('the log names each look by its time, newest first', /^The look at \w{3} \w{3} \d+, \d+:\d\d [AP]M/.test(saw), saw.slice(0, 200))
  check('it says the check’s card was raised', saw.includes('Raised on Home: The check node check.js fails'), saw.slice(0, 600))
  check('and the watches it did not read', saw.includes('Not read, its watch is off: GitHub, your /locust comments.'), saw.slice(0, 600))
  const filtered = String(await drive.capture('what Scout saw: only what it left out', async () => {
    await drive.evaluate(`(() => { const box = [...document.querySelectorAll('.lc-scoutlog__filter input')][0]; box?.click(); return box ? 'ticked' : 'no filter' })()`)
    await sleep(400)
    return drive.evaluate(`document.querySelector('.lc-scoutlog')?.innerText.replace(/\\s+/g, ' ') ?? document.querySelector('[data-setting="scout"]')?.innerText.slice(0, 400)`)
  }))
  check('the filter keeps the looks that left something out', !filtered.includes('Raised on Home: The check node check.js fails') || filtered.includes('Not read'), filtered.slice(0, 300))
  await drive.evaluate(CLOSE_SETTINGS)

  // 6. Asked in Scout's own conversation.
  // Scout's teammate is named Scout when nobody on the team is yet (scout.ts makeTeammate).
  const opened = String(await drive.evaluate(openTeammateScript('Scout')))
  check('Scout’s own conversation opens', opened.startsWith('opened'), opened)
  const answered = String(await drive.capture('Scout answers from what it saw', async () => drive.evaluate(sendAndWaitScript('Why didn’t you tell me about issue #42?', { waitSeconds: 240 }))))
  say(`  ${answered.slice(0, 300)}`)
  const conversation = String(await drive.evaluate(`document.querySelector('.lc-thread')?.innerText.replace(/\\s+/g, ' ') ?? ''`))
  check('Scout answers', /finished: /.test(answered) && conversation.length > 60, answered.slice(0, 200))
  check('from the record: issue #42 is not in it, said plainly', /not (in|see|find|have|record)|no (record|mention)|never (saw|seen|read)|didn.t see|did not see/i.test(conversation), conversation.slice(-500))
  const asked = String(await drive.capture('Scout names the look', async () => drive.evaluate(sendAndWaitScript('When did you see the check failing?', { waitSeconds: 240 }))))
  const named = String(await drive.evaluate(`document.querySelector('.lc-thread')?.innerText.replace(/\\s+/g, ' ') ?? ''`))
  check('and names the look its answer comes from', /look at \w{3} \w{3} \d+, \d+:\d\d [AP]M/i.test(named.slice(-1200)), `${asked.slice(0, 120)} | ${named.slice(-500)}`)
  // Its turns ran in Ask: the record of the newest says so.
  const ledger = join(drive.profile, 'mission-ledger')
  const newest = (await readdir(ledger)).filter((file) => file.endsWith('.jsonl')).map((file) => join(ledger, file))
  const records = await Promise.all(newest.map(async (file) => readFile(file, 'utf8').catch(() => '')))
  const scoutTurns = records.filter((text) => text.includes('Why didn’t you tell me about issue #42?') || text.includes('Why didn\\u2019t you tell me about issue #42?'))
  check('the question ran in Ask', scoutTurns.length > 0 && scoutTurns.every((text) => /"mode":"ask"/.test(text)), `${String(scoutTurns.length)} record(s)`)
} catch (error) {
  failures += 1
  say(`  [FAIL] the drive stopped: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. Scout and Ada on ${FREE_ROUTE.model}; the folder's check is \`node check.js\`, failing until answer.txt says 42.` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} FAILURE(S)`)
process.exit(failures === 0 ? 0 : 1)
