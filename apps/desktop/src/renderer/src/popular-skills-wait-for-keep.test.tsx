import { readFileSync } from 'node:fs'
import { renderToStaticMarkup } from 'react-dom/server'
import { expect, it } from 'vitest'
import { SkillsFromGitHub, runsText } from './components/SkillsFromGitHub.js'
import { POPULAR_SKILLS } from '../../shared/popular-skills.js'

it('the popular shelf shows five packs and opens the existing Look before any Keep', () => {
  const markup = renderToStaticMarkup(<SkillsFromGitHub />)
  expect(markup).toContain('Popular skills')
  for (const pack of POPULAR_SKILLS) {
    expect(markup).toContain(pack.source)
    expect(markup).toContain(pack.license)
    expect(markup).toContain(pack.stars.toLocaleString('en-US'))
  }
  expect(markup).toContain('Needs its own install to work.')
  expect(markup).toContain('Stars as of 2026-10-30')
  const source = readFileSync(new URL('./components/SkillsFromGitHub.tsx', import.meta.url), 'utf8')
  const tile = source.slice(source.indexOf('{POPULAR_SKILLS.map'), source.indexOf('</section>'))
  expect(tile).toContain('look(pack.source, true)')
  expect(tile).not.toContain('installSkillLibrary')
  expect(source).toContain('setPicked(skillPreviewPicks(response.data, all))')
  expect(tile).toContain('openLink(`https://github.com/${pack.source}`)')
})

it('every file that runs is named before keeping even when a skill has more than four scripts', () => {
  expect(runsText(['a.sh', 'b.py', 'c.js', 'd.exe', 'e.ps1'])).toBe('Runs: a.sh, b.py, c.js, d.exe, e.ps1')
  expect(runsText([])).toBeUndefined()
})
