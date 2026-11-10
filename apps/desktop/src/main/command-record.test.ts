import { describe, expect, it } from 'vitest'

import type { RuntimeCommandSpec } from '@teammate/runtime-adapters'
import { recordableCommand } from './command-record.js'

const spec = (args: readonly string[]): RuntimeCommandSpec =>
  ({
    runtime: 'copilot',
    executablePath: 'C:\\Windows\\System32\\cmd.exe',
    args,
    cwd: 'C:\\work',
    stdin: 'none',
    stdout: 'jsonl'
  }) as RuntimeCommandSpec

describe('the command the host records', () => {
  it('keeps the flags, which is the whole point', () => {
    const recorded = recordableCommand(
      spec(['/d', '/s', '/c', 'copilot.cmd', '--output-format', 'json', '--deny-tool=write,shell']),
      'Edit the file'
    )
    expect(recorded.cwd).toBe('C:\\work')
    expect(recorded.args).toContain('--deny-tool=write,shell')
    expect(recorded.args).toContain('--output-format')
    expect(recorded.executablePath).toBe('C:\\Windows\\System32\\cmd.exe')
  })

  it('never records the prompt, whatever flag introduced it', () => {
    // Copilot takes it after `-p`, OpenCode as a positional. Matching by value
    // rather than position is what makes one rule cover both.
    const secret = 'Rewrite the billing handler and mention hunter2'
    const copilot = recordableCommand(spec(['-p', secret, '--no-color']), secret)
    const opencode = recordableCommand(spec(['run', secret]), secret)
    expect(copilot.args).toEqual(['-p', '<prompt>', '--no-color'])
    expect(opencode.args).toEqual(['run', '<prompt>'])
    expect(JSON.stringify(copilot.args)).not.toContain('hunter2')
    expect(JSON.stringify(opencode.args)).not.toContain('hunter2')
  })

  it('matches the prompt after trimming, the way a builder may have passed it', () => {
    expect(recordableCommand(spec(['-p', '  do the thing  ']), 'do the thing').args)
      .toEqual(['-p', '<prompt>'])
  })

  it('leaves a flag alone that merely contains the prompt as a substring', () => {
    // Whole-value equality in both directions: a flag holding the prompt is
    // not the prompt, and a prompt equal to a flag is still the prompt.
    const recorded = recordableCommand(spec(['--model', 'auto-ok']), 'ok')
    expect(recorded.args).toEqual(['--model', 'auto-ok'])
  })

  it('records nothing as the prompt when there is no prompt', () => {
    // A run whose prompt is empty must not turn every empty argument into a
    // marker, which would misreport the command.
    expect(recordableCommand(spec(['--flag', '']), '   ').args).toEqual(['--flag', ''])
  })

  it('truncates an overlong argument rather than dropping it', () => {
    const recorded = recordableCommand(spec(['--config', 'x'.repeat(900)]), 'p')
    expect(recorded.args[1]).toHaveLength(512)
    expect(recorded.args[1]?.endsWith('…')).toBe(true)
  })

  it('bounds how many arguments it will keep, and says how many it dropped', () => {
    const many = Array.from({ length: 70 }, (_unused, index) => `--flag${String(index)}`)
    const recorded = recordableCommand(spec(many), 'p')
    expect(recorded.args).toHaveLength(65)
    expect(recorded.args.at(-1)).toBe('… 6 more')
  })
})
