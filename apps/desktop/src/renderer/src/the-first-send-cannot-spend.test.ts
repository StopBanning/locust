import { describe, expect, it } from 'vitest'

import type { PublicModel } from '../../shared/ipc.js'
import { ACCOUNT_DEFAULT_MODEL, freeStartModel } from './status.js'

/**
 * Sol's beta review, 2026-09-21, finding 1 — the only one of the nine that
 * can cost a new person money.
 *
 * > After Install, the composer is Account Default... The one runtime that
 * > needs no account should come up on a named free model, so the first Send
 * > cannot spend.
 *
 * Reproduced twice, on 0.239 and again on 0.242: install OpenCode from the
 * welcome screen — the runtime we recommend BY NAME because it needs no
 * account — and the composer lands on `OpenCode / Account Default`. That is
 * "whatever this account gives you", which on a signed-in account is a
 * billable model somebody else chose, sitting behind the first Enter.
 *
 * The rule is FIRST LISTED FREE, never a named model. Hardcoding
 * `muse-spark-1.3-contributor-free` here would be the bet
 * `freeStartStillFree` exists to refuse: somebody else's catalogue, changed
 * on somebody else's clock, baked into our first-run path.
 */
const model = (id: string, runtime = 'opencode'): PublicModel =>
  ({ id, runtime, displayName: id, description: '', supportedEfforts: [] } as unknown as PublicModel)

describe('the first send cannot spend', () => {
  it('takes a free model, the steadiest listed first (0.712), and among the unnamed the first listed', () => {
    const listed = [
      model('opencode/paid-thing'),
      model('opencode/ling-3.0-flash-fin-free'),
      model('opencode/glm-5.3-flash-free')
    ]
    expect(freeStartModel('opencode', listed)).toBe('opencode/glm-5.3-flash-free')
    expect(freeStartModel('opencode', [model('opencode/paid-thing'), model('opencode/zz-one-free'), model('opencode/aa-two-free')])).toBe('opencode/zz-one-free')
  })

  it('leaves the route alone when nothing listed is free', () => {
    expect(freeStartModel('opencode', [model('opencode/paid-thing')])).toBeUndefined()
  })

  it('leaves the route alone before the catalogue has been read', () => {
    expect(freeStartModel('opencode', [])).toBeUndefined()
  })

  it('never answers for a runtime that needs an account', () => {
    // Codex, Claude and Cursor all bill somebody. A "free" id under one of
    // them would still be spending the person's own quota, so this function
    // has nothing to say about them — `defaultRoute` already refuses to send
    // a fresh profile anywhere but the no-account runtime.
    expect(freeStartModel('codex', [model('codex/some-free', 'codex')])).toBeUndefined()
  })

  it('does not treat a free model as a free model of another runtime', () => {
    expect(freeStartModel('opencode', [model('cursor/grok-free', 'cursor')])).toBeUndefined()
  })

  it('still reads account-default as the unchosen state', () => {
    // The effect in App.tsx only fires while the model is this exact value,
    // so the constant and the guard have to stay the same string.
    expect(ACCOUNT_DEFAULT_MODEL).toBe('account-default')
  })
})
