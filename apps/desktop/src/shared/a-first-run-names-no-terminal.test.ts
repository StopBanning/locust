import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

import { installSentence } from './runtime-install.js'

/**
 * A FIRST RUN NAMES NO TERMINAL (0.840). The clean-machine drive of 2026-10-28, on 0.839 with nothing installed:
 * Settings told a newcomer to "run codex once to sign in" and showed `npm install -g` with a Copy button beside an
 * Install button that does it, though the launch plan's rule is that nothing makes a newcomer type commands. Sign in
 * is a button on the row once the agent is there, and the npm line stays only where Install cannot run it.
 */
const read = (path: string): string => readFileSync(fileURLToPath(new URL(path, import.meta.url)), 'utf8')

describe('a first run names no terminal', () => {
  it('says Install, then Sign in, for every npm agent that needs an account', () => {
    for (const [id, name] of [['claude', 'Claude Code'], ['codex', 'Codex CLI'], ['copilot', 'Copilot CLI']] as const) {
      const said = installSentence(id, name)
      expect(said).toContain('Press Install, then Sign in.')
      expect(said).not.toMatch(/\brun\b/)
    }
    expect(installSentence('opencode', 'OpenCode')).not.toContain('--')
  })

  it('shows the npm line only where the row has no working Install', () => {
    const screens = read('../renderer/src/components/Screens.tsx')
    const at = screens.indexOf('<InstallCommand command={installCommand(runtime.id)!} />')
    const guard = screens.slice(screens.lastIndexOf('{status.tag', at), at)
    expect(guard).toContain("(onInstall === undefined || npmMissing === true)")
    expect(screens).not.toContain('one command, no')
  })
})
