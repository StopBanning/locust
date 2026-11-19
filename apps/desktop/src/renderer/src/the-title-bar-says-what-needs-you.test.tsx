import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { TitleBar } from './components/TitleBar.js'

/**
 * THE TITLE BAR SAYS WHAT NEEDS YOU (0.373): beside what is running, and
 * only when something waits -- a line that is always there is furniture.
 */
const bar = (needsYou: number, withList = true): string =>
  renderToStaticMarkup(
    <TitleBar workspaceName="tides" runningCount={1} needsYou={needsYou} {...(withList ? { onNeedsYou: () => undefined } : {})} />
  )

describe('the title bar', () => {
  it('says how many things wait on you, as a button that opens their list, before what is running', () => {
    const html = bar(2)
    expect(html).toContain('2 need you')
    expect(html).toContain('aria-haspopup="menu"')
    expect(html.indexOf('2 need you')).toBeLessThan(html.indexOf('1 running'))
  })

  it('says nothing when nothing waits, or when there is no list to open', () => {
    expect(bar(0)).not.toContain('need')
    expect(bar(2, false)).not.toContain('need')
  })
})
