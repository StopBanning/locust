import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { KeyCard, TeammateKeysField } from './components/KeyCard.js'

/**
 * A KEY IS ASKED FOR ON A CARD (0.833, components/KeyCard.tsx). Under the answer that asked: the key's name, what it is
 * for, a box that never shows what is typed, and Keep; once kept, only that the teammate has it. A name no key may
 * take is refused there, and an agent that cannot pass keys says so instead of offering a box.
 */
const draw = (element: Parameters<typeof renderToStaticMarkup>[0]): string => renderToStaticMarkup(element)

describe('a key asked for on a card', () => {
  it('offers a masked box and Keep for each key asked for', () => {
    const card = draw(<KeyCard teammateId="tm_mira" name="Mira" reaches requests={[{ name: 'STRIPE_KEY', why: 'To make test charges' }]} />)
    expect(card).toContain('Mira asks for a key: ')
    expect(card).toContain('<code>STRIPE_KEY</code> -- To make test charges')
    expect(card).toMatch(/<input[^>]*type="password"[^>]*aria-label="The key STRIPE_KEY for Mira"/)
    expect(card).toContain('Keep for Mira')
    expect(card).toContain('Kept encrypted on this computer, for Mira alone')
  })

  it('says a key is kept, refuses a name no key may take, and says when the agent cannot use one', () => {
    const has = draw(<KeyCard teammateId="tm_mira" name="Mira" reaches kept={['STRIPE_KEY']} requests={[{ name: 'STRIPE_KEY', why: '' }]} />)
    expect(has).toContain('Mira has STRIPE_KEY.')
    expect(has).not.toContain('type="password"')
    const reserved = draw(<KeyCard teammateId="tm_mira" name="Mira" reaches requests={[{ name: 'OPENAI_API_KEY', why: 'to call the model' }]} />)
    expect(reserved).toContain('an agent or this computer reads for itself')
    expect(reserved).not.toContain('type="password"')
    const codex = draw(<KeyCard teammateId="tm_mira" name="Mira" reaches={false} requests={[{ name: 'STRIPE_KEY', why: '' }]} />)
    expect(codex).toContain('Locust does not give keys to its agent yet')
    expect(codex).toContain('Keys reach Claude Code and Cursor teammates for now')
    expect(codex).not.toContain('type="password"')
  })

  it('lists a teammate’s keys by name in its settings, with Remove, and adds one masked', () => {
    const field = draw(<TeammateKeysField teammateId="tm_mira" name="Mira" kept={['SLACK_WEBHOOK_URL', 'STRIPE_KEY']} />)
    expect(field).toContain('<code>SLACK_WEBHOOK_URL</code>')
    expect(field.match(/>Remove</g)).toHaveLength(2)
    expect(field).toMatch(/<input[^>]*type="password"[^>]*aria-label="The key"/)
    expect(field).toContain('Keys reach Claude Code and Cursor teammates for now.')
  })
})
