import { describe, expect, it } from 'vitest'

import { FREE_START_RUNTIME, installCommand, installSentence, runtimeInstallFacts } from './runtime-install.js'

describe('what a person has to do to get a runtime', () => {
  it('names the exact package for each of the four that install from npm', () => {
    // Read off the machine that has them all, not off documentation:
    // %APPDATA%\npm\node_modules holds these four names.
    expect(installCommand('claude')).toBe('npm install -g @anthropic-ai/claude-code')
    expect(installCommand('codex')).toBe('npm install -g @openai/codex')
    expect(installCommand('copilot')).toBe('npm install -g @github/copilot')
    expect(installCommand('opencode')).toBe('npm install -g opencode-ai')
  })

  it('offers no command for the ones that do not install from npm', () => {
    // Cursor installs itself into %LOCALAPPDATA%\cursor-agent and leaves
    // nothing behind naming the command that ran it. A guessed command line
    // is worse than a link, because a person will paste it.
    expect(installCommand('cursor')).toBeUndefined()
    expect(runtimeInstallFacts('cursor')?.install).toEqual({ kind: 'vendor', url: 'https://cursor.com/cli' })
    expect(installCommand('antigravity')).toBeUndefined()
  })

  it('knows the one runtime that needs no account, and that the others do', () => {
    // This is the whole on-ramp: `opencode auth list` reports 0 credentials on
    // the machine every drive in this repo runs on, and those drives run on
    // its free model. So one npm install is a working teammate.
    expect(FREE_START_RUNTIME).toBe('opencode')
    expect(runtimeInstallFacts('opencode')?.signIn).toBeUndefined()
    expect(runtimeInstallFacts('opencode')?.account).toBeUndefined()

    // The negative control. Without it, "opencode needs no account" is also
    // satisfied by a table where nothing needs one.
    for (const runtime of ['claude', 'codex', 'copilot', 'cursor']) {
      expect(runtimeInstallFacts(runtime)?.signIn).toBeTypeOf('string')
      expect(runtimeInstallFacts(runtime)?.account).toBeTypeOf('string')
    }
  })

  it('says what to do next, not only what is missing', () => {
    // The one that needs nothing says so, because it is the fastest way from
    // "nothing works" to "one teammate works".
    expect(installSentence('opencode', 'OpenCode')).toBe(
      'OpenCode was not found on this machine. Press Install. It needs no account and no sign-in, and its free model runs as soon as it is there.'
    )
    // Sign in is a button on the row once the agent is there: no terminal is named (0.840).
    expect(installSentence('claude', 'Claude Code')).toBe(
      'Claude Code was not found on this machine. Press Install, then Sign in. It needs an Anthropic account.'
    )
    expect(installSentence('copilot', 'Copilot CLI')).toBe(
      'Copilot CLI was not found on this machine. Press Install, then Sign in. It needs a GitHub Copilot subscription.'
    )
    // Kilo's free model needs no account, so it is not told to sign in first.
    expect(installSentence('kilo', 'Kilo')).toBe(
      'Kilo was not found on this machine. Press Install. Its free model needs no account; run kilo auth login only for a paid model.'
    )
    expect(installSentence('cursor', 'Cursor Agent')).toBe(
      'Cursor Agent was not found on this machine. It installs from https://cursor.com/cli.'
    )
  })

  it('answers nothing for an id that names no installable thing', () => {
    // The settings list carries `omniroute`, which chooses a route rather than
    // being a thing to install, and `gemini`, which no mission can run under.
    // Both must draw no install help rather than throw or invent one.
    expect(installCommand('omniroute')).toBeUndefined()
    expect(runtimeInstallFacts('omniroute')).toBeUndefined()
    expect(installSentence('omniroute', 'Omniroute')).toBe('Omniroute was not found on this machine.')
    expect(runtimeInstallFacts('gemini')).toBeUndefined()
  })
})
