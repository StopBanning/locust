// Real GitHub pack, previewed then kept in a throwaway profile. No model calls.
// Negative control: a pre-change build has no shelf and fails the five-tiles check.
import { readFile, readdir } from 'node:fs/promises'
import { join } from 'node:path'
import { say, scratchRepository, startDrive } from './drive-lib.mjs'
const drive = await startDrive({ name: 'popular-skills', port: 9894, workspace: await scratchRepository('locust-popular-skills-'), sendsNothing: false, holdCursor: false, keep: true })
const section = `document.querySelector('[data-setting="skills-from-github"]')`
let failed = 0
const check = (name, ok) => { if (!ok) failed += 1; say(`[${ok ? 'PASS' : 'FAIL'}] ${name}`) }
const wait = async (expression, seconds = 90) => {
  for (let n = 0; n < seconds * 4; n += 1) {
    if (await drive.evaluate(expression)) return
    await new Promise((resolve) => setTimeout(resolve, 250))
  }
  throw Error(`Timed out: ${expression}`)
}
try {
  await drive.ready()
  await drive.evaluate(`[...document.querySelectorAll('.lc-sidebar__nav button')].find(b => /Settings/.test(b.innerText))?.click()`)
  await wait(`!!document.querySelector('.lc-settings__navitem')`)
  await drive.evaluate(`[...document.querySelectorAll('.lc-settings__navitem')].find(b => b.innerText.trim() === 'Teammates')?.click()`)
  await wait(`!!${section}`)
  await drive.evaluate(`${section}.querySelector('.lc-skilllib__popular')?.scrollIntoView({ block: 'start' })`)
  const shelf = await drive.capture('01. Popular skills before a look', () => drive.evaluate(`({ tiles: ${section}.querySelectorAll('.lc-skilllib__pack').length, text: ${section}.querySelector('.lc-skilllib__popular')?.innerText ?? '', kept: ${section}.querySelectorAll('.lc-skilllib__kept').length })`))
  check('all five packs and the separate Claude-Mem install warning are shown', shelf.tiles === 5 && shelf.text.includes('Needs its own install to work.') && shelf.text.includes('Stars as of 2026-10-30'))
  if (shelf.tiles !== 5) throw Error('Popular shelf absent (negative control)')
  check('nothing is kept before looking', shelf.kept === 0)
  await drive.evaluate(`[...${section}.querySelectorAll('.lc-skilllib__tile')].find(tile => tile.innerText.includes('anthropics/skills')).querySelector('.lc-skilllib__pack').click()`)
  await wait(`!!${section}.querySelector('.lc-skilllib__preview') || !!${section}.querySelector('.lc-skilllib__said.lc-tone-amber')`)
  if (!await drive.evaluate(`!!${section}.querySelector('.lc-skilllib__preview')`)) throw Error(await drive.evaluate(`${section}.querySelector('.lc-skilllib__said').innerText`))
  const preview = await drive.capture('02. The pack is listed before Keep', () => drive.evaluate(`({ rows: [...${section}.querySelectorAll('.lc-skilllib__skill')].map(row => ({ name: row.querySelector('.lc-skilllib__skillname')?.innerText, picked: row.querySelector('input').checked, held: row.querySelector('input').disabled, runs: row.querySelector('.lc-skilllib__runs')?.innerText })), kept: ${section}.querySelectorAll('.lc-skilllib__kept').length })`))
  check('all twenty offered skills are picked, including canvas-design', preview.rows.length === 20 && preview.rows.every(row => row.picked && !row.held) && preview.rows.some(row => row.name === 'canvas-design'))
  check('executable files are named before keeping', preview.rows.some(row => row.name === 'pdf' && row.runs?.includes('.py')))
  const library = join(drive.profile, 'skill-library')
  check('the look has not downloaded a skill folder', preview.kept === 0 && (await readdir(join(library, 'skills')).catch(() => [])).length === 0)
  await drive.evaluate(`${section}.querySelector('.lc-skilllib__actions').scrollIntoView({ block: 'center' })`)
  await drive.capture('03. Keep names the count and warns about scripts', () => drive.evaluate(`${section}.querySelector('.lc-skilllib__actions')?.innerText`))
  await drive.evaluate(`${section}.querySelector('.lc-skilllib__actions .lc-primarybutton').click()`)
  await wait(`${section}.querySelectorAll('.lc-skilllib__kept').length === 1 || !!${section}.querySelector('.lc-skilllib__said.lc-tone-amber')`, 240)
  await drive.evaluate(`${section}.querySelector('.lc-skilllib__kept')?.scrollIntoView({ block: 'center' })`)
  const kept = await drive.capture('04. The pack is listed as kept', () => drive.evaluate(`({ text: ${section}.querySelector('.lc-skilllib__kept')?.innerText ?? '', said: ${section}.querySelector('.lc-skilllib > .lc-skilllib__said')?.innerText ?? '' })`))
  check('the real pack is listed as kept with twenty skills', kept.text.includes('anthropics/skills') && kept.text.includes('20 skills') && kept.said.includes('Kept 20 skills'))
  const record = JSON.parse(await readFile(join(library, 'library.json'), 'utf8'))
  check('the profile records twenty pinned skills including canvas-design', record.sources[0]?.skills.length === 20 && record.sources[0]?.skills.some(skill => skill.name === 'canvas-design' && skill.bytes > 5 * 1024 * 1024) && /^[0-9a-f]{40}$/.test(record.sources[0]?.sha))
  check('all twenty folders landed and staging is gone', (await readdir(join(library, 'skills'))).length === 20 && !(await readdir(library)).some(name => name.startsWith('.staging-')))
} catch (error) { failed += 1; say(error instanceof Error ? error.message : String(error)) }
finally { await drive.finish({ intro: 'Popular skills: a real Anthropic pack, previewed then kept. No model calls.' }); say(failed === 0 ? 'ALL CHECKS PASSED' : `${failed} FAILED`); process.exitCode = failed === 0 ? 0 : 1 }
