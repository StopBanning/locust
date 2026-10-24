import { describe, expect, it } from 'vitest'

import { parseRoutineFile, routineExportText, routineFileName, routineMarkdown, routineFileText } from './routine-file.js'
import type { RoutineFile } from './routine-file.js'

/**
 * A ROUTINE IS A MARKDOWN FILE (0.834, routine-file.ts). Exported, a routine reads as what it is -- its name, then
 * each step in its own words -- with what it asks for at the top, and comes back exactly as it went. Edited by
 * hand, it is read with the same checks as any routine file; one the Markdown would not carry is exported as JSON.
 */
const file: RoutineFile = {
  format: 'locust-routine',
  version: 1,
  name: 'Weekly notes',
  steps: [
    'Read what changed in {{folder}} since {{since}}.\n\n## What to look for\n\n- new files\n- removed tests',
    'Write the notes in a {{tone}} tone.',
    'Check the notes say only what changed.'
  ],
  inputs: [
    { key: 'folder', label: 'Folder', kind: 'folder', required: true },
    { key: 'since', label: 'Since when', kind: 'text', required: false, default: '5' },
    { key: 'tone', label: 'Tone', kind: 'choice', required: true, choices: ['Brief', 'true'], default: 'true' },
    { key: 'note', label: 'Anything else', kind: 'long-text', required: false, default: 'two\nlines' }
  ],
  handOffs: [{}, { role: 'Writer' }, { role: 'Reviewer', check: true }],
  route: { runtime: 'claude' },
  connectors: ['GitHub']
}

describe('a routine as a Markdown file', () => {
  it('reads as its name and its steps, and comes back exactly', () => {
    const text = routineMarkdown(file)
    expect(text.startsWith('---\nlocust-routine: 1\nruns-on: claude\n')).toBe(true)
    expect(text).toContain('\n# Weekly notes\n\n## Step 1\n\nRead what changed in {{folder}}')
    expect(text).toContain('\n## Step 2 · Writer\n\nWrite the notes in a {{tone}} tone.\n')
    expect(text).toContain('\n## Step 3 · Reviewer · checks the work\n')
    expect(parseRoutineFile(text)).toEqual({ ok: true, file })
    expect(routineExportText(file)).toEqual({ text, markdown: true })
    expect(routineFileName('Weekly notes', true)).toBe('Weekly notes.routine.md')
  })

  it('takes a file written by hand, with Windows line ends and a number for a default', () => {
    const hand = [
      '---', 'locust-routine: 1', 'inputs:', '  - key: count', '    label: How many', '    kind: text', '    required: false', '    default: 3', '---',
      '', '# Tidy up', '', '## Step 1', '', 'List the {{count}} oldest branches.', '', '## Step 2 · checks the work', '', 'Check none of them is main.', ''
    ].join('\r\n')
    expect(parseRoutineFile(hand)).toEqual({ ok: true, file: {
      format: 'locust-routine', version: 1, name: 'Tidy up',
      steps: ['List the {{count}} oldest branches.', 'Check none of them is main.'],
      inputs: [{ key: 'count', label: 'How many', kind: 'text', required: false, default: '3' }],
      handOffs: [{}, { check: true }], route: {}, connectors: []
    } })
  })

  it('says what is wrong with one that is not a routine, with the same checks as the JSON file', () => {
    const said = (text: string): string => { const read = parseRoutineFile(text); return read.ok ? 'read' : read.message }
    expect(said('---\nlocust-routine: 2\n---\n# A\n\n## Step 1\n\nGo.\n')).toContain('version 2')
    expect(said('---\nlocust-routine: 1\nschedule: daily\n---\n# A\n\n## Step 1\n\nGo.\n')).toContain('(schedule)')
    expect(said('---\nlocust-routine: 1\n---\n# A\n\nA note.\n\n## Step 1\n\nGo.\n')).toContain('between its title and ## Step 1')
    expect(said('---\nlocust-routine: 1\n---\n# A\n\nGo.\n')).toContain('between its title')
    expect(said('---\nlocust-routine: 1\n---\n# A\n')).toContain('no steps')
    expect(said('---\nlocust-routine: 1\n---\n# A\n\n## Step 1 · Writer · Reviewer\n\nGo.\n')).toContain('more than one teammate')
    expect(said('---\nlocust-routine: 1\n---\n\n## Step 1\n\nGo.\n')).toContain('name as a title')
    expect(said('---\nlocust-routine: 1\n\n# A\n')).toContain('never closes')
    expect(said('---\nlocust-routine: 1\n---\n# A\n\n## Step 1\n\nRead {{missing}}.\n')).toContain('not declared')
    expect(said('---\nnot: [closed\n---\n# A\n')).toContain('not readable')
    expect(said('---\na: &x 1\nb: *x\n---\n# A\n')).toContain('not readable')
  })

  it('is exported as the JSON file when the Markdown would not carry it exactly', () => {
    const awkward = { ...file, steps: [file.steps[0]!, 'Then:\n## Step 3\nwritten in the step itself', file.steps[2]!] }
    expect(routineExportText(awkward)).toEqual({ text: routineFileText(awkward), markdown: false })
    const spaced = { ...file, steps: ['Ends on a blank line.\n\n', file.steps[1]!, file.steps[2]!] }
    expect(routineExportText(spaced).markdown).toBe(false)
    const dotted = { ...file, handOffs: [{}, { role: 'A · B' }, { role: 'Reviewer', check: true as const }] }
    expect(routineExportText(dotted).markdown).toBe(false)
    expect(routineFileName('Weekly notes')).toBe('Weekly notes.locust-routine.json')
  })
})
