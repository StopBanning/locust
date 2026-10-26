import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { RoutineDialog } from './components/RoutineDialog.js'
import { HOOK_WARNING, HOOK_ASK } from '../../shared/routine-hook.js'

/** A CALLED LINK SAYS WHAT LEAVES THE COMPUTER (2026-10-25): readable relay data, a password, and Ask. */
describe('a called link in the routine editor', () => {
  it('offers the new kind and its privacy and Ask words, with a save-first message for a new routine', () => {
    const html = renderToStaticMarkup(<RoutineDialog initialName="Event" initialSteps={['Read the event']} initialSchedule={{ kind: 'hook' }} teammate={undefined} truncated={false} busy={false} routeLabel={undefined} error={undefined} onSave={() => undefined} onCancel={() => undefined} />)
    expect(html).toContain('When a link is called')
    expect(html).toContain(HOOK_WARNING)
    expect(html).toContain(HOOK_ASK)
    expect(html).toContain('Save the routine to make its link.')
    expect(html).toContain('Settings &gt; Phone')
    expect(html).not.toContain('value="https://')
  })
})
