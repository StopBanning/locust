import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import { currentDrafts, repeatsOf } from '../../shared/tune.js'
import { TuneTeammate, type TuneCard } from './components/TuneTeammate.js'

/**
 * A TUNE SAYS WHEN A SETUP REPEATS (0.809). Either angle may keep the instructions as they are, and drafts that could
 * not be written start as copies of Current, so two columns can run the same words. That is still allowed -- after
 * Harness², the same setup twice shows how much one run changes by chance -- but each run counts against a paid
 * plan, so the card says which draft repeats which setup, and what the runs can then show, before anything starts.
 */
vi.mock('./useModal.js', () => ({ useModal: vi.fn() }))
const drafts = { how: { instructions: 'Check your work.', why: 'It stopped before checking.' }, knows: { instructions: 'Name your assumptions.', why: 'It treated a guess as a fact.' } }
const card: TuneCard = { id: 1, teammateId: 'tm_wren', route: { runtime: 'opencode', model: 'opencode/example-free', mode: 'accept-edits' }, name: 'Wren', model: 'Example Free', free: true, prompt: 'Explain the cause.', current: 'Be concise.', drafts, drafting: false }
const screen = (over: Partial<TuneCard> = {}): string => renderToStaticMarkup(<TuneTeammate card={{ ...card, ...over }} onChange={() => undefined} onCancel={() => undefined} onRun={() => undefined} />)

describe('a tune that repeats a setup', () => {
  it('names the first setup a draft matches, ignoring spaces at either end, and counts the different setups', () => {
    expect(repeatsOf('Be concise.', drafts)).toEqual({ distinct: 3 })
    expect(repeatsOf('Be concise.', currentDrafts('Be concise.'))).toEqual({ how: 'Current', knows: 'Current', distinct: 1 })
    expect(repeatsOf('Be concise.', { ...drafts, knows: { ...drafts.knows, instructions: ' Check your work.\n' } })).toEqual({ knows: 'How it works', distinct: 2 })
    expect(repeatsOf('', { ...drafts, how: { ...drafts.how, instructions: '' } })).toEqual({ how: 'Current', distinct: 2 })
  })

  it('says nothing when the three setups differ', () => {
    const html = screen()
    expect(html).not.toContain('Same as')
    expect(html).not.toContain('changes by chance')
  })

  it('marks the repeating draft and says what two runs of one setup show', () => {
    const html = screen({ drafts: { ...drafts, knows: { instructions: 'Be concise.', why: 'Its instructions already cover this.' } } })
    expect(html).toContain('<span class="lc-dialog__note lc-tune__same">Same as Current</span>')
    expect(html.match(/Same as /g)).toHaveLength(1)
    expect(html).toContain('Two of the three setups are the same, so those two runs also show how much one run changes by chance.')
  })

  it('says when all three are the same, as when the drafts could not be written', () => {
    const html = screen({ drafts: currentDrafts('Be concise.') })
    expect(html.match(/Same as Current/g)).toHaveLength(2)
    expect(html).toContain('All three setups are the same, so the runs only show how much one run changes by chance. Edit a draft to try something different.')
  })

  it('waits for the drafts: copies shown while they are written are placeholders, not repeats', () => {
    const html = screen({ drafting: true, drafts: currentDrafts('Be concise.') })
    expect(html).not.toContain('Same as')
    expect(html).not.toContain('changes by chance')
  })
})
