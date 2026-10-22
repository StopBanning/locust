// Skills from GitHub (0.710), end to end in Settings > Teammates, against the real anthropics/skills.
//
//   node _tools/drive-skills-from-github.mjs [--packaged <exe>]
//
// Sends nothing to any model. Reads GitHub without signing in: three API calls a look (of the 60 an hour
// GitHub allows), plus raw files. Looks at the repository, checks that nothing is ticked for the person and
// that a skill carrying scripts names them, keeps two skills, reads what landed in the profile, looks
// again (the kept ones ticked and tagged), and removes them.
import { readdir, readFile } from 'node:fs/promises'
import { join } from 'node:path'
import { say, scratchRepository, startDrive } from './drive-lib.mjs'

const REPO = process.env.LOCUST_SKILLS_REPO ?? 'anthropics/skills'
const WITH_SCRIPTS = process.env.LOCUST_SKILL_WITH_SCRIPTS ?? 'pdf'
const PLAIN = process.env.LOCUST_SKILL_PLAIN ?? 'brand-guidelines'
const packaged = process.argv.includes('--packaged') ? process.argv[process.argv.indexOf('--packaged') + 1] : undefined

const drive = await startDrive({
  name: 'skills-from-github',
  port: 9873,
  workspace: await scratchRepository('locust-skills-github-ws-'),
  sendsNothing: true,
  ...(packaged === undefined ? {} : { packaged })
})

let failures = 0
const check = (what, ok, detail = '') => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail ? ` -- ${detail}` : ''}`)
}
const wait = async (expression, seconds = 60) => {
  for (let i = 0; i < seconds * 4; i += 1) {
    if (await drive.evaluate(expression)) return true
    await new Promise((resolve) => setTimeout(resolve, 250))
  }
  throw new Error(`Timed out: ${expression}`)
}
const SECTION = `document.querySelector('[data-setting="skills-from-github"]')`
/** Each skill row of the look: name, ticked, disabled, and its "Runs:" line. */
const ROWS = `[...${SECTION}.querySelectorAll('.lc-skilllib__skill')].map((li) => ({
  name: li.querySelector('.lc-skilllib__skillname')?.firstChild?.textContent?.trim(),
  ticked: li.querySelector('input').checked,
  disabled: li.querySelector('input').disabled,
  tag: li.querySelector('.lc-tag')?.innerText ?? null,
  runs: li.querySelector('.lc-skilllib__runs')?.innerText ?? null
}))`
const tick = (name) => drive.evaluate(`(() => {
  const li = [...${SECTION}.querySelectorAll('.lc-skilllib__skill')].find((row) => row.querySelector('.lc-skilllib__skillname')?.firstChild?.textContent?.trim() === ${JSON.stringify(name)})
  li?.querySelector('input').click()
  return !!li
})()`)
const keepButton = `[...${SECTION}.querySelectorAll('.lc-skilllib__actions button')].find((b) => /^Keep/.test(b.innerText))`

try {
  await drive.ready()
  await drive.evaluate(`[...document.querySelectorAll('.lc-sidebar__nav button')].find(b => /Settings/.test(b.innerText))?.click()`)
  await wait(`!!document.querySelector('.lc-settings__navitem')`)
  await drive.evaluate(`[...document.querySelectorAll('.lc-settings__navitem')].find((b) => b.innerText.trim() === 'Teammates')?.click()`)
  await wait(`!!${SECTION}`)
  check('Settings > Teammates has Skills from GitHub, empty to start', await drive.evaluate(`${SECTION}.querySelectorAll('.lc-skilllib__kept').length === 0`))

  // 01. Look.
  await drive.evaluate(`(() => {
    const box = ${SECTION}.querySelector('.lc-skilllib__look input')
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(box, ${JSON.stringify(REPO)})
    box.dispatchEvent(new Event('input', { bubbles: true }))
  })()`)
  await drive.evaluate(`${SECTION}.querySelector('.lc-skilllib__look button[type="submit"]').click()`)
  await wait(`!!${SECTION}.querySelector('.lc-skilllib__preview') || ${SECTION}.querySelector('.lc-skilllib__said')?.classList.contains('lc-tone-amber')`)
  const said = await drive.evaluate(`${SECTION}.querySelector('.lc-skilllib__said')?.innerText ?? null`)
  if (said !== null) throw new Error(`the look said: ${said}`)
  const looked = await drive.capture(`01. Looked at ${REPO}`, () => drive.evaluate(`({ head: ${SECTION}.querySelector('.lc-skilllib__previewhead').innerText.replace(/\\s+/g, ' '), rows: ${ROWS}, keep: ${keepButton}?.innerText, keepDisabled: ${keepButton}?.disabled })`))
  const rows = looked.rows
  check(`the look lists the repository's skills (${rows.length})`, rows.length >= 5, looked.head)
  check('nothing is ticked for the person, and Keep waits for a pick', rows.every((row) => !row.ticked) && looked.keepDisabled === true, `${looked.keep}`)
  const scripted = rows.find((row) => row.name === WITH_SCRIPTS)
  check(`${WITH_SCRIPTS} names the files in it that run`, /^Runs: /.test(scripted?.runs ?? ''), String(scripted?.runs))
  check(`${PLAIN} is there to keep`, rows.some((row) => row.name === PLAIN && !row.disabled))

  // 02. Keep two.
  check('ticking picks', (await tick(WITH_SCRIPTS)) && (await tick(PLAIN)))
  // The frame shows the picked row and the warning above the button, not the list's top.
  await drive.evaluate(`(() => {
    const li = [...${SECTION}.querySelectorAll('.lc-skilllib__skill')].find((row) => row.querySelector('.lc-skilllib__skillname')?.firstChild?.textContent?.trim() === ${JSON.stringify(WITH_SCRIPTS)})
    li?.scrollIntoView({ block: 'center' })
    ${SECTION}.querySelector('.lc-skilllib__actions')?.scrollIntoView({ block: 'end' })
  })()`)
  const before = await drive.capture('02. Two picked, the warning about files that run', () => drive.evaluate(`({ warn: ${SECTION}.querySelector('.lc-skilllib__warn')?.innerText ?? null, keep: ${keepButton}?.innerText })`))
  check('a pick that carries scripts says so before keeping', /files that run\. In Auto, a teammate can run them without asking\./.test(before.warn ?? ''), String(before.warn))
  check('the button counts the pick', before.keep === 'Keep 2 skills', String(before.keep))
  await drive.evaluate(`${keepButton}.click()`)
  await wait(`${SECTION}.querySelectorAll('.lc-skilllib__kept').length === 1 || ${SECTION}.querySelector('.lc-skilllib__said')?.classList.contains('lc-tone-amber')`, 120)
  const kept = await drive.capture('03. Kept', () => drive.evaluate(`({ row: ${SECTION}.querySelector('.lc-skilllib__kept')?.innerText.replace(/\\s+/g, ' ') ?? null, said: ${SECTION}.querySelector('.lc-skilllib__said')?.innerText ?? null, preview: !!${SECTION}.querySelector('.lc-skilllib__preview') })`))
  check('kept: one row for the repository, the look closed', kept.row !== null && !kept.preview && kept.row.includes(REPO), String(kept.row))
  // Every runtime that reads skills since 0.814, not Claude Code's alone.
  check('and it says when teammates get them', /^Kept 2 skills from .+\. Claude Code, Codex, OpenCode, Copilot and Antigravity teammates get them from their next run\.$/.test(kept.said ?? ''), String(kept.said))

  // 03. What landed in the profile: the two skills, at one commit, every file listed.
  const library = join(drive.profile, 'skill-library')
  const record = JSON.parse(await readFile(join(library, 'library.json'), 'utf8'))
  const source = record.sources[0]
  check('library.json: the repository, at a commit, the two skills', record.sources.length === 1 && /^[0-9a-f]{40}$/.test(source.sha) && source.skills.map((skill) => skill.name).sort().join() === [PLAIN, WITH_SCRIPTS].sort().join(), `${source.source} @ ${String(source.sha).slice(0, 7)}`)
  check('the skill folders, nothing else', (await readdir(join(library, 'skills'))).sort().join() === [PLAIN, WITH_SCRIPTS].sort().join())
  const skillFile = await readFile(join(library, 'skills', WITH_SCRIPTS, 'SKILL.md'), 'utf8')
  check(`${WITH_SCRIPTS}/SKILL.md is the real one`, /^---/.test(skillFile) && skillFile.includes('description'), `${skillFile.length} chars`)
  check(`${WITH_SCRIPTS}'s scripts are recorded as files that run`, source.skills.find((skill) => skill.name === WITH_SCRIPTS)?.runs.length > 0)
  check('no staging folder is left behind', (await readdir(library)).every((name) => !name.startsWith('.staging-')))

  // 04. Look for changes: the kept ones ticked and tagged.
  await drive.evaluate(`[...${SECTION}.querySelectorAll('.lc-skilllib__kept button')].find((b) => b.innerText === 'Look for changes').click()`)
  await wait(`!!${SECTION}.querySelector('.lc-skilllib__preview')`)
  const again = await drive.capture('04. Looked again', () => drive.evaluate(`({ rows: ${ROWS}, keep: ${keepButton}?.innerText, said: [...${SECTION}.querySelectorAll('.lc-skilllib__preview .lc-skilllib__said')].map((p) => p.innerText) })`))
  const ticked = again.rows.filter((row) => row.ticked).map((row) => row.name).sort()
  check('looking again ticks what is kept, and tags it', ticked.join() === [PLAIN, WITH_SCRIPTS].sort().join() && again.rows.filter((row) => row.tag !== null).length === 2, `${ticked.join()} · ${again.keep}`)
  // Unless the repository moved in the seconds between, the same commit: up to date.
  check('and says whether anything changed since', again.said.some((line) => /^Up to date: nothing has changed since you kept them\.$|^Changed since you kept them\./.test(line)), again.said.join(' | '))
  await drive.evaluate(`[...${SECTION}.querySelectorAll('.lc-skilllib__actions button')].find((b) => b.innerText === 'Cancel').click()`)

  // 05. Remove: asked once, then gone from the screen and the profile.
  const remove = `[...${SECTION}.querySelectorAll('.lc-skilllib__kept button')].find((b) => /^Remove/.test(b.innerText))`
  await drive.evaluate(`${remove}.click()`)
  const armed = await drive.evaluate(`${remove}?.innerText`)
  check('Remove asks first', armed === 'Remove 2 skills?', String(armed))
  // ArmedButton ignores a second press within 400 ms (a double-click is not a second decision).
  await new Promise((resolve) => setTimeout(resolve, 700))
  await drive.evaluate(`${remove}.click()`)
  await wait(`${SECTION}.querySelectorAll('.lc-skilllib__kept').length === 0`)
  const gone = await drive.capture('05. Removed', () => drive.evaluate(`${SECTION}.querySelector('.lc-skilllib__said')?.innerText ?? null`))
  check('removed, and it says what that means', /^Removed the skills kept from .+\. Runs that start now do not get them\.$/.test(gone ?? ''), String(gone))
  check('the profile holds none of them', (await readdir(join(library, 'skills'))).length === 0 && JSON.parse(await readFile(join(library, 'library.json'), 'utf8')).sources.length === 0)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Skills from GitHub, against ${REPO}.` })
  say(failures === 0 ? 'ALL CHECKS PASSED' : `${failures} FAILED`)
  process.exit(failures === 0 ? 0 : 1)
}
