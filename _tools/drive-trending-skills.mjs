// Real GitHub pack from the New this week row, previewed then kept in a throwaway profile. No model calls.
// Negative control: a build without the row has four tiles fewer and fails the row check.
import { readFile, readdir } from 'node:fs/promises'
import { join } from 'node:path'
import { say, scratchRepository, startDrive } from './drive-lib.mjs'
const drive = await startDrive({ name: 'trending-skills', port: 9895, workspace: await scratchRepository('locust-trending-skills-'), sendsNothing: false, holdCursor: false, keep: true })
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
  await drive.evaluate(`[...${section}.querySelectorAll('.lc-skilllib__name')].find(el => el.innerText === 'New this week')?.scrollIntoView({ block: 'start' })`)
  const shelf = await drive.capture('01. New this week under Popular skills', () => drive.evaluate(`({ tiles: ${section}.querySelectorAll('.lc-skilllib__pack').length, text: ${section}.querySelector('.lc-skilllib__popular')?.innerText ?? '' })`))
  check('nine packs: five popular and four new this week', shelf.tiles === 9)
  check('the row is named, dated and says how long each pack has climbed', shelf.text.includes('New this week') && shelf.text.includes('Stars as of 2026-10-31') && shelf.text.includes('stars since Oct 24 · MIT'))
  if (shelf.tiles !== 9) throw Error('New this week row absent (negative control)')
  await drive.evaluate(`[...${section}.querySelectorAll('.lc-skilllib__tile')].find(tile => tile.innerText.includes('t3dotgg/tstack')).querySelector('.lc-skilllib__pack').click()`)
  await wait(`!!${section}.querySelector('.lc-skilllib__preview') || !!${section}.querySelector('.lc-skilllib__said.lc-tone-amber')`)
  if (!await drive.evaluate(`!!${section}.querySelector('.lc-skilllib__preview')`)) throw Error(await drive.evaluate(`${section}.querySelector('.lc-skilllib__said').innerText`))
  await drive.evaluate(`${section}.querySelector('.lc-skilllib__preview').scrollIntoView({ block: 'start' })`)
  const preview = await drive.capture('02. tstack is listed before Keep', () => drive.evaluate(`({ rows: [...${section}.querySelectorAll('.lc-skilllib__skill')].map(row => ({ name: row.querySelector('.lc-skilllib__skillname')?.innerText, picked: row.querySelector('input').checked })), kept: ${section}.querySelectorAll('.lc-skilllib__kept').length })`))
  check('all nine tstack skills are offered and picked', preview.rows.length === 9 && preview.rows.every(row => row.picked) && preview.rows.some(row => row.name === 'babysit-pr'))
  const library = join(drive.profile, 'skill-library')
  check('the look has not downloaded a skill folder', preview.kept === 0 && (await readdir(join(library, 'skills')).catch(() => [])).length === 0)
  await drive.evaluate(`${section}.querySelector('.lc-skilllib__actions .lc-primarybutton').click()`)
  await wait(`${section}.querySelectorAll('.lc-skilllib__kept').length === 1 || !!${section}.querySelector('.lc-skilllib__said.lc-tone-amber')`, 240)
  await drive.evaluate(`${section}.querySelector('.lc-skilllib__kept')?.scrollIntoView({ block: 'center' })`)
  const kept = await drive.capture('03. tstack is listed as kept', () => drive.evaluate(`${section}.querySelector('.lc-skilllib__kept')?.innerText ?? ''`))
  check('tstack is listed as kept with nine skills', kept.includes('t3dotgg/tstack') && kept.includes('9 skills'))
  const record = JSON.parse(await readFile(join(library, 'library.json'), 'utf8'))
  check('the profile records nine pinned skills', record.sources[0]?.skills.length === 9 && /^[0-9a-f]{40}$/.test(record.sources[0]?.sha))
  check('all nine folders landed', (await readdir(join(library, 'skills'))).length === 9)
} catch (error) { failed += 1; say(error instanceof Error ? error.message : String(error)) }
finally { await drive.finish({ intro: 'New this week: a real trending pack, previewed then kept. No model calls.' }); say(failed === 0 ? 'ALL CHECKS PASSED' : `${failed} FAILED`); process.exitCode = failed === 0 ? 0 : 1 }
