import { describe, expect, it } from 'vitest'

import type { MissionMode } from '../../shared/ipc.js'
import { availableCommands, matchingCommands, runtimeSlashCommands, slashQuery, SLASH_COMMANDS } from './slashCommands.js'

const ALL_MODES: readonly MissionMode[] = ['ask', 'plan', 'accept-edits', 'approve-each', 'auto']
const every = availableCommands({ modes: ALL_MODES, running: true, canSwarm: true })

describe('recognising a command being typed', () => {
  it('reads a bare slash as the menu opening', () => {
    expect(slashQuery('/')).toBe('')
  })

  it('reads what has been typed after it', () => {
    expect(slashQuery('/pl')).toBe('pl')
    expect(slashQuery('/PLAN')).toBe('plan')
    expect(slashQuery('  /stop  ')).toBe('stop')
  })

  it('ignores a slash inside a sentence', () => {
    // THE test. "run /plan on this" is a message that happens to contain a
    // slash; acting on it would be the app doing something it was never asked
    // to do, to a message the person meant to send.
    expect(slashQuery('run /plan on this')).toBeUndefined()
    expect(slashQuery('look at src/main/index.ts')).toBeUndefined()
    expect(slashQuery('what does / mean here')).toBeUndefined()
  })

  it('is not a command when there is no slash at all', () => {
    expect(slashQuery('plan')).toBeUndefined()
    expect(slashQuery('')).toBeUndefined()
  })
})

describe('which commands are offered', () => {
  it('offers /tune only with a finished teammate conversation', () => {
    expect(availableCommands({ modes: ['ask'], running: false, canSwarm: false }).some((row) => row.name === 'tune')).toBe(false)
    expect(availableCommands({ modes: ['ask'], running: false, canSwarm: false, tune: true }).some((row) => row.name === 'tune')).toBe(true)
    expect(availableCommands({ modes: ['ask'], running: true, canSwarm: false, tune: true }).some((row) => row.name === 'tune')).toBe(false)
  })
  it('offers only modes this runtime can honour', () => {
    // A command listed and then refused is the dead control this app keeps
    // removing. The filter is the reason it cannot happen.
    const limited = availableCommands({ modes: ['ask', 'plan'], running: false, canSwarm: true })
    const names = limited.map((command) => command.name)
    expect(names).toContain('ask')
    expect(names).toContain('plan')
    expect(names).not.toContain('auto')
    expect(names).not.toContain('approve')
  })

  it('offers stop only while something is running', () => {
    expect(availableCommands({ modes: ALL_MODES, running: false, canSwarm: true }).map((c) => c.name)).not.toContain('stop')
    expect(every.map((command) => command.name)).toContain('stop')
  })

  it('offers swarm only where it can be turned on', () => {
    expect(availableCommands({ modes: ALL_MODES, running: false, canSwarm: false }).map((c) => c.name)).not.toContain('swarm')
  })

  it('always offers the route picker, which is always reachable', () => {
    expect(availableCommands({ modes: [], running: false, canSwarm: false }).map((c) => c.name)).toEqual(['model'])
  })

  it('offers an issue only for a folder whose issues can be read', () => {
    expect(availableCommands({ modes: ALL_MODES, running: false, canSwarm: false }).some((command) => command.name === 'issue')).toBe(false)
    expect(availableCommands({ modes: ALL_MODES, running: false, canSwarm: false, issues: true }).some((command) => command.name === 'issue')).toBe(true)
  })
})

describe('narrowing as you type', () => {
  it('keeps the ones that start with what was typed', () => {
    expect(matchingCommands('a', every).map((command) => command.name)).toEqual(['ask', 'approve', 'auto'])
    expect(matchingCommands('st', every).map((command) => command.name)).toEqual(['stop'])
  })

  it('shows everything for a bare slash', () => {
    expect(matchingCommands('', every)).toHaveLength(every.length)
  })

  it('shows nothing rather than something wrong for an unknown word', () => {
    expect(matchingCommands('zzz', every)).toEqual([])
  })
})

describe('the commands themselves', () => {
  it('every one says what it does', () => {
    // The menu is the only place these are explained, so a blank detail would
    // be a command nobody can predict the effect of.
    for (const command of SLASH_COMMANDS) {
      expect(command.detail.length, command.name).toBeGreaterThan(10)
      expect(command.name).toMatch(/^[a-z]+$/)
    }
  })

  it('names no command twice', () => {
    expect(new Set(SLASH_COMMANDS.map((command) => command.name)).size).toBe(SLASH_COMMANDS.length)
  })
})

describe("a runtime's own commands (0.426)", () => {
  const claude = runtimeSlashCommands('Claude Code', [
    { name: 'compact', description: 'Clear conversation history but keep a summary in context', argumentHint: '<optional custom summarization instructions>' },
    { name: 'security-review', description: '', argumentHint: '' },
    { name: 'plugin:skill', description: 'A skill from a plugin', argumentHint: '' },
    // A name Locust already uses stays Locust's.
    { name: 'plan', description: 'Something else', argumentHint: '' }
  ])

  it("sit under the runtime, after Locust's own, with what goes after them", () => {
    const offered = availableCommands({ modes: ALL_MODES, running: false, canSwarm: true, runtimeCommands: claude })
    const names = offered.map((command) => command.name)
    expect(names.slice(0, names.indexOf('auto') + 1)).toEqual(SLASH_COMMANDS.filter((c) => c.name !== 'stop' && c.action.kind !== 'issues' && c.action.kind !== 'tune').map((c) => c.name))
    expect(names.slice(names.indexOf('auto') + 1)).toEqual(['compact', 'security-review', 'plugin:skill'])
    const compact = offered.find((command) => command.name === 'compact')
    expect(compact).toMatchObject({ group: 'Claude Code', hint: '<optional custom summarization instructions>', action: { kind: 'runtime' } })
    // One with no description still says whose it is.
    expect(offered.find((command) => command.name === 'security-review')?.detail).toBe("Claude Code's own command.")
    expect(offered.filter((command) => command.name === 'plan')).toHaveLength(1)
    expect(offered.find((command) => command.name === 'plan')?.action.kind).toBe('mode')
  })

  it('are not offered while a run is going, since one is sent as a message', () => {
    const offered = availableCommands({ modes: ALL_MODES, running: true, canSwarm: true, runtimeCommands: claude })
    expect(offered.some((command) => command.action.kind === 'runtime')).toBe(false)
  })

  it('are found by what is typed, names with marks included', () => {
    const offered = availableCommands({ modes: ALL_MODES, running: false, canSwarm: true, runtimeCommands: claude })
    expect(slashQuery('/security-r')).toBe('security-r')
    expect(slashQuery('/plugin:')).toBe('plugin:')
    expect(slashQuery('/team/review')).toBe('team/review')
    expect(matchingCommands('security-r', offered).map((command) => command.name)).toEqual(['security-review'])
    expect(matchingCommands('plugin:', offered).map((command) => command.name)).toEqual(['plugin:skill'])
    expect(matchingCommands('co', offered).map((command) => command.name)).toEqual(['compact'])
  })
})
