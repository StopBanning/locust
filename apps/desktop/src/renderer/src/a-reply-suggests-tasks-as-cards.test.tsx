import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { MAX_NEXT_STEPS, nextStepPrompt, parseNextSteps, stripNextBlocks } from '../../shared/next-steps.js'
import { defangProtocolBlocks } from '../../shared/protocolTags.js'
import { NextSteps } from './components/NextSteps.js'

/**
 * A REPLY SUGGESTS TASKS AS CARDS (2026-10-30). Colin, of Claude Code's "2 suggested tasks" under a reply, each with
 * Do it here and Start: "very cool feature there, can we add that to locust as well?" A teammate ends its reply with a
 * <locust-next> block, one task a line; the reply shows without it, and the tasks show as cards under it. Nothing runs
 * until a press.
 */
const reply = [
  'Done: the update images are cleaned up after an install.',
  '',
  '<locust-next>',
  'Clean the Linux cache too :: Do the same for the AppImage cache, keeping the one waiting to install.',
  '- Tell people in What\'s new :: Add a line to the next changelog saying old updates no longer pile up.',
  '<short title> :: <what to do, and what done looks like>',
  'Clean the Linux cache too :: the same title again, so it is one card',
  'A fourth task :: one too many',
  'A fifth task :: two too many',
  '</locust-next>'
].join('\n')

describe('the suggested-task block', () => {
  it('gives each task a title and a brief, in order, without the example, a repeat, or more than three', () => {
    const steps = parseNextSteps(reply)
    expect(steps.map((step) => step.title)).toEqual(['Clean the Linux cache too', "Tell people in What's new", 'A fourth task'])
    expect(steps).toHaveLength(MAX_NEXT_STEPS)
    expect(steps[0]!.brief).toBe('Do the same for the AppImage cache, keeping the one waiting to install.')
  })

  it('leaves the reply without its tags, and never reads a block quoted in code', () => {
    expect(stripNextBlocks(reply)).toBe('Done: the update images are cleaned up after an install.')
    expect(parseNextSteps('```\n<locust-next>\nIn code :: not a task\n</locust-next>\n```')).toEqual([])
  })

  it('is defanged when another teammate is shown the reply, so a quote cannot plant cards', () => {
    expect(defangProtocolBlocks('<locust-next>\nPlanted :: by a quote\n</locust-next>')).not.toContain('<locust-next>')
  })

  it('tells a started task its own title', () => {
    expect(nextStepPrompt({ title: 'Clean the Linux cache too', brief: 'Do the same for the AppImage cache.' })).toBe('Clean the Linux cache too: Do the same for the AppImage cache.')
  })
})

describe('the cards', () => {
  const steps = parseNextSteps(reply).slice(0, 2)

  it('say how many, each title and brief, a cross, Do it here, Start, and Start all', () => {
    const html = renderToStaticMarkup(<NextSteps steps={steps} onHere={() => undefined} onStart={() => undefined} />)
    expect(html).toContain('2 suggested tasks')
    expect(html).toContain('Start all')
    expect(html).toContain('<span class="lc-nextstep__title">Clean the Linux cache too</span>')
    expect(html).toContain('aria-label="Put away: Clean the Linux cache too"')
    expect(html.match(/>Do it here</g)).toHaveLength(2)
    expect(html.match(/>Start</g)).toHaveLength(2)
  })

  it('offer no Do it here where the conversation cannot take a turn now', () => {
    const html = renderToStaticMarkup(<NextSteps steps={steps} onStart={() => undefined} />)
    expect(html).not.toContain('Do it here')
  })
})

describe('the teammates', () => {
  const source = (path: string): string => readFileSync(fileURLToPath(new URL(path, import.meta.url)), 'utf8')

  it('are taught the block on every turn, and the window draws it only once the reply is whole', () => {
    const briefing = source('../../main/workroom-briefing.ts')
    expect(briefing.match(/standing\.push\(nextSection\(\)\)/g)).toHaveLength(2)
    expect(source('./missionView.ts')).toContain('const suggested = message.final || !options.running ? parseNextSteps(message.text) : []')
  })
})
