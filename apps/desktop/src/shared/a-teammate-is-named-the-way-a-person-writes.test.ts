import { describe, expect, it } from 'vitest'

import { addressedTeammate, pairingCodeIn, spacedCode, teammateCommands } from './chat-apps.js'

/**
 * A TEAMMATE IS NAMED THE WAY A PERSON WRITES (0.832, shared/chat-apps.ts). In a chat app, a person writes to a
 * teammate as they would to anyone: "Ada: check the build", "Ada, check the build", "@ada check the build". Slack
 * has no slash for it at all, so this is how Slack is written to; Telegram and Discord take it beside their /ada.
 */
const TEAM = [
  { teammateId: 't_ada', name: 'Ada' },
  { teammateId: 't_lovelace', name: 'Ada Lovelace' },
  { teammateId: 't_zoe', name: 'Zoë Ng-Smith' },
  { teammateId: 't_wren', name: 'Wren' }
]

describe('a teammate named the way a person writes', () => {
  it('reads a name and a colon or a comma, or an @, in any case and without its accents', () => {
    expect(addressedTeammate('Ada: check the build', TEAM)).toEqual({ teammateId: 't_ada', rest: 'check the build' })
    expect(addressedTeammate('ada, check the build', TEAM)).toEqual({ teammateId: 't_ada', rest: 'check the build' })
    expect(addressedTeammate('  @ADA check the build', TEAM)).toEqual({ teammateId: 't_ada', rest: 'check the build' })
    expect(addressedTeammate('@wren: and the tests', TEAM)).toEqual({ teammateId: 't_wren', rest: 'and the tests' })
    expect(addressedTeammate('zoe ng-smith: hello', TEAM)).toEqual({ teammateId: 't_zoe', rest: 'hello' })
    // Its command works as its name does.
    expect(addressedTeammate('zoe_ng_smith: hello', TEAM)).toEqual({ teammateId: 't_zoe', rest: 'hello' })
    // The longest name wins.
    expect(addressedTeammate('Ada Lovelace: the engine', TEAM)).toEqual({ teammateId: 't_lovelace', rest: 'the engine' })
    // Nothing after the name: who the next messages go to.
    expect(addressedTeammate('Wren:', TEAM)).toEqual({ teammateId: 't_wren', rest: '' })
    expect(addressedTeammate('@wren', TEAM)).toEqual({ teammateId: 't_wren', rest: '' })
  })

  it('leaves alone a message that only begins with a name, or a word like one', () => {
    expect(addressedTeammate('Ada said the build is fine', TEAM)).toBeUndefined()
    expect(addressedTeammate('Adam: hello', TEAM)).toBeUndefined()
    expect(addressedTeammate('@adam hello', TEAM)).toBeUndefined()
    expect(addressedTeammate('Note: the build failed', TEAM)).toBeUndefined()
    expect(addressedTeammate('check Ada: later', TEAM)).toBeUndefined()
    expect(addressedTeammate('', TEAM)).toBeUndefined()
  })

  it('reads a pairing code however it was spaced, and nothing else as one', () => {
    expect(pairingCodeIn('4729 1836')).toBe('47291836')
    expect(pairingCodeIn(' 4729-1836 ')).toBe('47291836')
    expect(pairingCodeIn('47291836')).toBe('47291836')
    expect(pairingCodeIn('4729 183')).toBeUndefined()
    expect(pairingCodeIn('my code is 47291836')).toBeUndefined()
    expect(spacedCode('47291836')).toBe('4729 1836')
  })

  it('never names a teammate by a command Locust keeps for itself', () => {
    const commands = teammateCommands([{ teammateId: 'a', name: 'Pair' }, { teammateId: 'b', name: 'Waiting' }])
    expect([...commands.values()]).toEqual(['pair2', 'waiting2'])
  })
})
