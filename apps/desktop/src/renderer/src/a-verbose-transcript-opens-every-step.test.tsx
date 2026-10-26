import { writeFileSync, mkdirSync } from 'node:fs'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { ActivityCard } from './components/ActivityCard.js'
import type { ActivityDetail } from './missionView.js'

/**
 * A finished run of 220 steps. Normal draws one line. Verbose opens every
 * step and each command's output. The times are what the report compares.
 */
const STEPS = 220

function steps(): ActivityDetail[] {
  return Array.from({ length: STEPS }, (_, index) => ({
    kind: 'shell',
    name: `echo step-${String(index)}`,
    tool: 'bash',
    title: `Checked step ${String(index)}`,
    settled: true,
    output: `done ${String(index)}`
  }))
}

function draw(verbose: boolean): { readonly html: string; readonly ms: number } {
  const started = performance.now()
  const html = renderToStaticMarkup(
    <ActivityCard
      variant="steps"
      summary=""
      trace={[{ key: 'line', text: 'Checked 220 steps' }]}
      finished
      details={steps()}
      runtimeName={undefined}
      workspacePath="C:/work/locust"
      openByDefault={false}
      {...(verbose ? { verbose: true } : {})}
    />
  )
  return { html, ms: performance.now() - started }
}

describe('Verbose opens every step of a long transcript', () => {
  it('times the folded line and the open steps', () => {
    const folded = draw(false)
    const open = draw(true)
    try {
      mkdirSync('D:/work/scratch/grok-polish-a', { recursive: true })
      writeFileSync('D:/work/scratch/grok-polish-a/verbose-times.txt', `before ${folded.ms.toFixed(1)} ms, ${String(folded.html.length)} bytes\nafter ${open.ms.toFixed(1)} ms, ${String(open.html.length)} bytes\n`)
    } catch { /* the times are the assertions below when this folder is not there */ }
    expect(folded.html).toContain('Checked 220 steps')
    expect(folded.html).not.toContain('done 219')
    expect(open.html).toContain('done 0')
    expect(open.html).toContain('done 219')
    expect(open.html).toContain('aria-expanded="true"')
    expect(folded.ms).toBeLessThan(2000)
    expect(open.ms).toBeLessThan(2000)
  })
})
