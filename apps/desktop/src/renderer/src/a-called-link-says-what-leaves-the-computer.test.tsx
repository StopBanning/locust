import { renderToStaticMarkup } from 'react-dom/server'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { RoutineDialog } from './components/RoutineDialog.js'
import { HOOK_WARNING, HOOK_ASK } from '../../shared/routine-hook.js'

/** A CALLED LINK SAYS WHAT LEAVES THE COMPUTER (2026-10-25): readable relay data, a password, and Ask. */
afterEach(() => { vi.unstubAllGlobals() })
const editor = (): string => renderToStaticMarkup(<RoutineDialog initialName="Event" initialSteps={['Read the event']} initialSchedule={{ kind: 'hook' }} teammate={undefined} truncated={false} busy={false} routeLabel={undefined} error={undefined} onSave={() => undefined} onCancel={() => undefined} />)
describe('a called link in the routine editor', () => {
  it('offers the new kind and its privacy and Ask words, with a save-first message for a new routine', () => {
    const html = editor()
    expect(html).toContain('When a link is called')
    expect(html).toContain(HOOK_WARNING)
    expect(html).toContain(HOOK_ASK)
    expect(html).toContain('Save the routine to make its link.')
    expect(html).toContain('Settings &gt; Phone')
    expect(html).not.toContain('value="https://')
    expect(html).not.toContain('The link is shown only in Locust on your computer.')
  })

  it('tells a browser the link stays on this computer, with no Copy or New link', () => {
    vi.stubGlobal('window', { desktop: { remote: { computer: 'this computer' } } })
    const html = editor()
    expect(html).toContain('The link is shown only in Locust on your computer.')
    expect(html).toContain(HOOK_WARNING)
    expect(html).toContain(HOOK_ASK)
    expect(html).not.toContain('>Copy<')
    expect(html).not.toContain('>New link<')
    expect(html).not.toContain('>Make link<')
    expect(html).not.toContain('aria-label="Routine link"')
    expect(html).not.toContain('Save the routine to make its link.')
  })
})
