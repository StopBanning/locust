import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { AgentMark } from './components/AgentMark.js'
import { usagePercent, usageWindowSentence, usageWindowsOf } from './missionView.js'

const reading = 'Gemini: 5-hour window 95% left · Gemini: weekly window 67% left · Claude and GPT: 5-hour window 100% left · Claude and GPT: weekly window 20% left · resets 2099-10-10T19:09:25Z · as of 2026-10-07T04:00:00Z'

describe('Antigravity remaining quota on Home', () => {
  it('uses consumption for bar width and warning tone, but names the remaining percentage', () => {
    const windows = usageWindowsOf(reading)
    expect(windows.map((window) => [window.name, window.percent, window.remaining])).toEqual([
      ['Gemini: 5-hour window', 5, 95], ['Gemini: weekly window', 33, 67],
      ['Claude and GPT: 5-hour window', 0, 100], ['Claude and GPT: weekly window', 80, 20]
    ])
    expect(usagePercent(reading)).toBe(80)
    const html = renderToStaticMarkup(<AgentMark runtime="antigravity" name="Antigravity" state="Ready" usage={reading} />)
    expect(html).toContain('95% left</span>')
    expect(html).toContain('style="width:5%"')
    expect(html).toContain('class="lc-agentcard__window is-pressing"')
    expect(html).toContain('20% left</span>')
    // The card is narrow: its labels drop the trailing "window" so "Claude and GPT: weekly" does not wrap a lone word.
    expect(html).toContain('<span class="lc-agentcard__label">Claude and GPT: weekly</span>')
    expect(html).not.toContain('weekly window</span>')
    expect(html).toContain('Read from your account at')
    expect(usageWindowSentence(reading)).toContain('95% of the Gemini: 5-hour window left')
    expect(usageWindowSentence(reading)).toContain('20% of the Claude and GPT: weekly window left')
  })
  it('does not carry an expired reading into the current warning', () => {
    const expired = 'Gemini: 5-hour window 0% left · resets 2026-10-06T00:00:00Z'
    expect(usagePercent(expired, new Date('2026-10-07T00:00:00Z'))).toBeUndefined()
    expect(usageWindowSentence(expired, new Date('2026-10-07T00:00:00Z'))).toBe('the Gemini: 5-hour window has reset since')
  })
  it('leaves existing used readings unchanged', () => {
    expect(usageWindowsOf('5-hour window 35% used')).toEqual([{ name: '5-hour window', percent: 35 }])
    expect(usageWindowSentence('5-hour window 35% used')).toBe('35% of the 5-hour window used')
  })
})
