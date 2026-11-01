import { describe, expect, it } from 'vitest'

import { modelDisplayName, shortRuntimeName } from './routeName.js'

describe('how a route reads on a chip', () => {
  it('says the three Colin named, exactly as he named them', () => {
    // The ask, verbatim: "it should just say Cursor / Grok 4.6 Claude /
    // Fable 5.1 Codex / GPT-6 Astra".
    expect(`${shortRuntimeName('cursor')} / ${modelDisplayName('cursor', 'cursor-grok-4.6')}`).toBe('Cursor / Grok 4.6')
    expect(`${shortRuntimeName('claude')} / ${modelDisplayName('claude', 'fable-5.1')}`).toBe('Claude / Fable 5.1')
    expect(`${shortRuntimeName('codex')} / ${modelDisplayName('codex', 'gpt-6-astra')}`).toBe('Codex / GPT-6 Astra')
  })

  it('keeps the hyphen only where the makers keep one', () => {
    expect(modelDisplayName('codex', 'gpt-5.6-luna')).toBe('GPT-5.6 Luna')
    expect(modelDisplayName('cursor', 'cursor-grok-4.6-medium')).toBe('Grok 4.6 Medium')
    expect(modelDisplayName('cursor', 'composer-2.5')).toBe('Composer 2.5')
  })

  it('spells the free models a new person meets first as their makers do', () => {
    // The bare Mac's first hour, 2026-11-01: the chip read "Glm 5.3 Flash", a Stop and switch "Mimo V2.6 Flash
    // Free", and the picker listed "DeepSeek V4.1 Flash" a row above "Deepseek V4.1 Flash Free".
    expect(modelDisplayName('opencode', 'opencode/deepseek-v4.1-flash-free')).toBe('DeepSeek V4.1 Flash Free')
    expect(modelDisplayName('opencode', 'opencode/glm-5.3-flash-free')).toBe('GLM 5.3 Flash Free')
    expect(modelDisplayName('opencode', 'opencode/mimo-v2.6-flash-free')).toBe('MiMo V2.6 Flash Free')
    expect(modelDisplayName('opencode', 'opencode/nemotron-3.5-super-vl-free')).toBe('Nemotron 3.5 Super VL Free')
    expect(modelDisplayName('opencode', 'opencode/minimax-m2.5-free')).toBe('MiniMax M2.5 Free')
  })

  it('does not touch a name that already reads as one', () => {
    // Providers give real display names, and this runs over those too. It
    // must be a no-op on them or it would slowly corrupt what they said.
    expect(modelDisplayName('codex', 'GPT-6 Astra')).toBe('GPT-6 Astra')
    expect(modelDisplayName('cursor', 'Grok 4.6')).toBe('Grok 4.6')
    expect(modelDisplayName('claude', 'Sonnet')).toBe('Sonnet')
  })

  it('drops a provider that repeats the runtime and keeps one that does not', () => {
    // OpenCode aggregates, so whose model it is stays -- that is the reason
    // `modelLabelFor` exists and this must not undo it.
    expect(modelDisplayName('opencode', 'opencode/ling-3.0-flash-fin-free')).toBe('Ling 3.0 Flash Fin Free')
    expect(modelDisplayName('opencode', 'anthropic/claude-sonnet-4')).toBe('Anthropic/Claude Sonnet 4')
  })

  it('invents nothing: every word comes from the id', () => {
    // An id nobody anticipated still reads as itself.
    expect(modelDisplayName('cursor', 'some-unheard-of-model-9')).toBe('Some Unheard Of Model 9')
    expect(modelDisplayName('claude', 'sonnet')).toBe('Sonnet')
  })

  it('leaves an id with nothing but the runtime in it alone', () => {
    expect(modelDisplayName('cursor', 'cursor')).toBe('Cursor')
  })

  it('reads a version written with a hyphen as a version', () => {
    // A mission on Cursor's `claude-opus-5-5-medium` read "Claude Opus 5 5
    // Medium" (2026-09-23): Anthropic's ids write the point as a hyphen.
    expect(modelDisplayName('cursor', 'claude-opus-5-5-medium')).toBe('Claude Opus 5.5 Medium')
    expect(modelDisplayName('cursor', 'claude-opus-4-8-thinking-low')).toBe('Claude Opus 4.8 Thinking Low')
    expect(modelDisplayName('cursor', 'claude-3-5-sonnet')).toBe('Claude 3.5 Sonnet')
    expect(modelDisplayName('claude', 'claude-haiku-4-5-20251001')).toBe('Haiku 4.5 20251001')
    // A date is not a version, and a version already dotted is left alone.
    expect(modelDisplayName('codex', 'gpt-4o-2024-08-06')).toBe('GPT-4o 2024 08 06')
    expect(modelDisplayName('cursor', 'cursor-grok-4.6-high')).toBe('Grok 4.6 High')
    expect(modelDisplayName('cursor', 'some-model-1-2-3')).toBe('Some Model 1 2 3')
  })

  it('keeps the second word where it is the program being named', () => {
    // Settings is telling you which program this is; the chip is not.
    expect(shortRuntimeName('opencode')).toBe('OpenCode')
    expect(shortRuntimeName('antigravity')).toBe('Antigravity')
  })
})

describe('a model the runtime picks for itself', () => {
  it('says it is a model, never the bare word a mode is named (0.378)', () => {
    // "Auto · running" over an Approve-each turn read as the Auto MODE
    // (drive-copilot-approve-each, 0.377).
    expect(modelDisplayName('copilot', 'auto')).toBe('Auto model')
    expect(modelDisplayName('cursor', 'auto')).toBe('Auto model')
    // Every other id is spelled as before.
    expect(modelDisplayName('copilot', 'gpt-5.6-luna')).not.toContain('model')
  })
})
