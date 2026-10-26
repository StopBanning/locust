import { describe, expect, it } from 'vitest'
import { composerRouteFor, pickKeyFor, routeAtStart } from '../shared/route-at-start.js'
import type { TeammateRoute } from '../shared/ipc.js'

const saved: TeammateRoute = { runtime: 'codex', model: 'gpt-5.6-luna', mode: 'accept-edits', effort: 'low' }
const wren = { teammateId: 'tm_wren', route: saved }
const fallback = { runtime: 'opencode', model: 'account-default' } as const

describe('route authority at the moment of start', () => {
  it('honours an explicit picker override for this teammate', () => {
    expect(routeAtStart({ ...fallback, routeOverrideFor: 'tm_wren' }, wren)).toEqual(fallback)
  })
  it('uses the host saved route over discovery/default selections, including saved effort', () => {
    expect(routeAtStart(fallback, wren)).toEqual({ runtime: 'codex', model: saved.model, effort: 'low' })
  })
  it('leaves nobody\'s mission on the composer route, regardless of old picker intent', () => {
    expect(routeAtStart({ ...fallback, effort: 'high', routeOverrideFor: 'tm_wren' }, undefined))
      .toEqual({ ...fallback, effort: 'high' })
  })
  it('does not mistake another teammate\'s picker change for this teammate\'s choice', () => {
    expect(routeAtStart({ ...fallback, routeOverrideFor: 'tm_gem' }, wren).runtime).toBe('codex')
  })
  it('preserves account-default as an intentional picker selection', () => {
    expect(routeAtStart({ runtime: 'codex', model: 'account-default', routeOverrideFor: 'tm_wren' }, wren))
      .toEqual({ runtime: 'codex', model: 'account-default' })
  })
  it('does not promote a seeded account-default to picker intent', () => {
    expect(routeAtStart({ runtime: 'codex', model: 'account-default' }, wren).model).toBe(saved.model)
  })
  it('keeps existing defaults for a new teammate without a saved route', () => {
    expect(routeAtStart(fallback, { teammateId: 'tm_new' })).toEqual(fallback)
    expect(routeAtStart({}, undefined)).toEqual({ runtime: 'codex' })
  })
  it('keeps effort changes when the request is based on the saved picker identity', () => {
    expect(routeAtStart({ runtime: 'codex', model: saved.model, effort: 'high' }, wren))
      .toEqual({ runtime: 'codex', model: saved.model, effort: 'high' })
  })
  it('preserves concrete effort variants without declaring them a new picker choice', () => {
    const cursor = { teammateId: 'tm_wren', route: { ...saved, runtime: 'cursor' as const, model: 'grok-low' } }
    expect(routeAtStart({ runtime: 'cursor', modelChoice: 'grok-low', model: 'grok-high' }, cursor))
      .toEqual({ runtime: 'cursor', model: 'grok-high' })
  })
  it('cannot carry a stale effort or model transform across a different saved route', () => {
    expect(routeAtStart({ runtime: 'cursor', modelChoice: 'grok-low', model: 'grok-high', effort: 'high' }, wren))
      .toEqual({ runtime: 'codex', model: saved.model, effort: 'low' })
  })
  it('returns no permission mode for the caller to accidentally inherit', () => {
    expect(routeAtStart(fallback, wren)).not.toHaveProperty('mode')
  })
})

describe('a pick belongs to the conversation it was made in (0.552)', () => {
  // Colin, 10/02: "if i have a teammate set to a certain model i should be
  // able to run a sepate chat with a new model without assigning that as
  // their new model". Picks were keyed by teammate; now by conversation.
  const choices = new Map([['mission_a', fallback], ['new:tm_gem', fallback]])
  it('a new chat with a teammate starts on their saved model', () => {
    expect(composerRouteFor(fallback, wren, choices, pickKeyFor(undefined, wren.teammateId))).toEqual(saved)
  })
  it('a pick made in a conversation is shown there again', () => {
    expect(composerRouteFor({ runtime: 'codex', model: saved.model }, wren, choices, 'mission_a')).toEqual(fallback)
  })
  it('a conversation shows what it ran on, not the saved model of its teammate', () => {
    const ran = { runtime: 'opencode' as const, model: 'opencode/free' }
    expect(composerRouteFor(fallback, wren, choices, 'mission_b', ran)).toEqual(ran)
  })
  it('a pick for a new chat with one teammate is not for another', () => {
    expect(composerRouteFor(fallback, { ...wren, teammateId: 'tm_gem' }, choices, pickKeyFor(undefined, 'tm_gem'))).toEqual(fallback)
    expect(composerRouteFor(fallback, wren, choices, pickKeyFor(undefined, 'tm_wren'))).toEqual(saved)
  })
  it('home does not inherit a teammate override', () => {
    const home = { runtime: 'claude' as const, model: 'account-default' }
    expect(composerRouteFor(home, undefined, choices, pickKeyFor(undefined, undefined))).toBe(home)
  })
  it('shows a Kilo teammate saved on the account default as Auto Free', () => {
    const kilo = { teammateId: 'tm_wren', route: { runtime: 'kilo' as const, model: 'account-default', mode: 'ask' as const } }
    expect(composerRouteFor(fallback, kilo, new Map(), pickKeyFor(undefined, kilo.teammateId))).toMatchObject({
      runtime: 'kilo',
      model: 'kilo/kilo-auto/free'
    })
    expect(composerRouteFor({ runtime: 'kilo', model: 'account-default' }, undefined, new Map(), 'new')).toMatchObject({
      runtime: 'kilo',
      model: 'kilo/kilo-auto/free'
    })
    expect(routeAtStart({ runtime: 'kilo', model: 'account-default' }, kilo)).toMatchObject({
      runtime: 'kilo',
      model: 'kilo/kilo-auto/free'
    })
  })
})
