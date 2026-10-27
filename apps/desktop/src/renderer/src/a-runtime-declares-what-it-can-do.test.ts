import { describe, expect, it } from 'vitest'

import type { MissionMode } from '../../shared/ipc.js'
import { RUNTIME_CAPABILITIES, modeRunsOn, modeUnavailableReason } from './status.js'

/**
 * A runtime cannot claim a mode by saying nothing.
 *
 * `modeRunsOn` was a chain of `if` clauses ending in `return true`, each
 * clause a war story worth keeping — and the fall-through was the defect.
 *
 * MEASURED 2026-09-21, by accident, while adding Muse: putting `muse` into
 * `MissionRuntimeId` made the app offer **Ask, Accept edits, Auto and Plan**
 * on a runtime with no adapter, no command builder and no event normalizer.
 * Nothing asked; silence meant yes.
 *
 * (A correction worth recording: I first reported that adding the runtime
 * "broke nothing — typecheck clean, 3,689 green". That was a stale package
 * build. Rebuilt, the compiler named two exhaustive records it had to be
 * added to. The tooling was better than my account of it.)
 *
 * The first test below is the CONTROL. This commit is a refactor, so every
 * runtime that existed before must answer exactly as it did before, on both
 * platforms, for all five modes — the table was transcribed from the `if`
 * chain and a transcription error would be silent and awful.
 */
const MODES: readonly MissionMode[] = ['ask', 'accept-edits', 'approve-each', 'plan', 'auto']

/** Exactly what the `if` chain answered on 2026-09-21, before it was replaced. */
const BEFORE: Readonly<Record<string, Readonly<Record<string, readonly string[]>>>> = {
  win32: {
    codex: ['ask', 'accept-edits', 'approve-each', 'plan', 'auto'],
    claude: ['ask', 'accept-edits', 'plan', 'auto'],
    cursor: ['accept-edits', 'auto'],
    gemini: ['ask', 'accept-edits', 'plan', 'auto'],
    opencode: ['ask', 'accept-edits', 'plan', 'auto'],
    copilot: ['ask', 'accept-edits', 'plan', 'auto'],
    antigravity: ['accept-edits']
  },
  darwin: {
    codex: ['ask', 'accept-edits', 'approve-each', 'plan', 'auto'],
    claude: ['ask', 'accept-edits', 'plan', 'auto'],
    cursor: ['ask', 'accept-edits', 'plan', 'auto'],
    gemini: ['ask', 'accept-edits', 'plan', 'auto'],
    opencode: ['ask', 'accept-edits', 'plan', 'auto'],
    copilot: ['ask', 'accept-edits', 'plan', 'auto'],
    antigravity: ['accept-edits']
  }
}

describe('a runtime declares what it can do', () => {
  /*
   * DELIBERATELY CHANGED SINCE, and named here rather than edited out of
   * BEFORE. The table above is the historical record of what the `if` chain
   * answered; a correction that quietly rewrote it would destroy the only
   * evidence of what the refactor was checked against.
   */
  const CORRECTED: Readonly<Record<string, readonly string[]>> = {
    // Gemini is PLANNED, no mission can run under it, and its stream has
    // never been captured -- Google refuses the CLI to consumer accounts.
    // It claimed three modes purely through the old fall-through, was
    // flagged when the table was written, and this is the commit it was
    // flagged for.
    gemini: [],
    // Antigravity through its CLI (0.540): every ordinary mode, each a flag
    // measured on agy 1.2.14. The app route alone was Edit only.
    antigravity: ['ask', 'plan', 'accept-edits', 'auto'],
    // Cursor read-only on Windows too (0.485): its own `--mode ask` held
    // against three pushed writes there on 2026-09-30, where plan mode had
    // written files (2026-09-02). The sandbox is still asked for wherever it
    // runs.
    cursor: ['ask', 'accept-edits', 'plan', 'auto'],
    // Claude Code gained Approve-each on 2026-10-26 (0.837): its own default
    // mode asks Locust's permission host before every edit and command --
    // measured with a stand-in permission tool on 2.1.311.
    claude: ['ask', 'accept-edits', 'approve-each', 'plan', 'auto'],
    // OpenCode gained Approve-each on 2026-09-25 (A6.7): it rides
    // `opencode serve`, which stops and asks -- measured, and driven
    // (drive-opencode-approve-each: an approved command ran, a declined one
    // did not). `run` could only reject, which is why it was not offered.
    opencode: ['ask', 'accept-edits', 'approve-each', 'plan', 'auto'],
    // Copilot gained Approve-each on 2026-09-26 (0.377): it rides the Agent
    // Client Protocol (`copilot --acp`), which stops and asks -- measured
    // with probes, and driven (drive-copilot-approve-each).
    copilot: ['ask', 'accept-edits', 'approve-each', 'plan', 'auto']
  }

  it('answers exactly as the if-chain did, except where a claim was corrected on purpose', () => {
    for (const [platform, byRuntime] of Object.entries(BEFORE)) {
      for (const [runtime, expected] of Object.entries(byRuntime)) {
        const actual = MODES.filter((mode) => modeRunsOn(mode, runtime as never, platform))
        const want = CORRECTED[runtime] ?? expected
        expect(actual.sort(), `${runtime} on ${platform}`).toEqual([...want].sort())
      }
    }
  })

  it('claims nothing for a runtime no mission can run under', () => {
    // The correction itself, asserted rather than implied by the table
    // above. Muse went the other way on the same day: `modes: []` until a
    // run proved two, then exactly those two. Same rule, both directions.
    expect(RUNTIME_CAPABILITIES.gemini.modes).toEqual([])
    for (const mode of MODES) {
      expect(modeRunsOn(mode, 'gemini', 'win32'), `gemini must not claim ${mode}`).toBe(false)
    }
  })

  it('offers a runtime only the modes someone has actually run under it', () => {
    /*
     * Muse. This row was `modes: []` for as long as no `muse exec` had been
     * run, because the docs promise an OS sandbox and approval modes and the
     * docs are not a measurement -- the case the old fall-through got wrong.
     *
     * Two have been run since, through the real builder, runner and
     * normalizer: `node _tools/drive-muse-echo.mjs` on the free echo
     * provider, read-only and workspace-write, both to `run.completed`.
     * Those were the two modes here, and Plan comes free with Ask. Auto was
     * added in 0.773 once it meant something: `--disable-sandbox`, run
     * 2026-10-15 to `Write-Output hello`, exit 0, on a PC whose Muse sandbox
     * was never set up.
     */
    expect(RUNTIME_CAPABILITIES.muse.modes).toEqual(['ask', 'accept-edits', 'auto'])
    expect(RUNTIME_CAPABILITIES.muse.evidence).toBe('measured')
    expect(modeRunsOn('plan', 'muse', 'win32')).toBe(true)
    expect(modeRunsOn('auto', 'muse', 'win32')).toBe(true)
    // Approve-each needs a per-call approval the exec transport cannot ask.
    expect(modeRunsOn('approve-each', 'muse', 'win32')).toBe(false)
  })

  it('keeps plan available exactly where ask is, and never apart from it', () => {
    // Plan is derived from ask rather than declared, so the invariant cannot
    // drift: a plan that could edit the workspace is a promise the app cannot
    // keep. Declaring both would be two places to get it wrong.
    for (const platform of ['win32', 'darwin', undefined]) {
      for (const runtime of Object.keys(RUNTIME_CAPABILITIES)) {
        expect(modeRunsOn('plan', runtime as never, platform), `${runtime} on ${String(platform)}`)
          .toBe(modeRunsOn('ask', runtime as never, platform))
      }
    }
  })

  it('says how it knows, for every runtime', () => {
    // The other half of a conformance claim: an adapter must not advertise
    // what it cannot do, and the row has to say what backs it. `unproven`
    // is a placeholder, not a verdict.
    for (const [runtime, facts] of Object.entries(RUNTIME_CAPABILITIES)) {
      expect(['measured', 'unproven'], `${runtime} has no evidence field`).toContain(facts.evidence)
    }
    // Gemini is listed PLANNED and no mission can run under it, yet it has
    // always claimed three modes through the fall-through. Kept as it was so
    // the control above stays honest about what this change touched — but
    // the claim is now visible instead of implied by silence.
    expect(RUNTIME_CAPABILITIES.gemini.evidence).toBe('unproven')
  })

  it('names the runtime the person chose when it refuses a mode', () => {
    /*
     * MEASURED on 2026-09-21 by driving the packaged build: with Muse Code
     * chosen, the mode menu read "Codex CLI only. Codex CLI cannot stop and
     * ask yet" and "Codex CLI runs its own agent under its own
     * permissions" -- about a runtime that was not in play. `runtimeLabel`
     * was an if-chain ending in `return 'Codex CLI'`, so a runtime nobody
     * added to it was not missing, it was renamed. The same fall-through
     * that let this table exist in the first place.
     */
    for (const runtime of Object.keys(RUNTIME_CAPABILITIES)) {
      for (const mode of MODES) {
        const reason = modeUnavailableReason(mode, runtime as never, 'win32')
        if (reason === undefined || runtime === 'codex') continue
        // Approve-each names Codex and OpenCode on purpose -- it says which
        // runtimes DO have it -- so only the second half is checked.
        const aboutThisRuntime = mode === 'approve-each' ? reason.replace('Claude Code, Codex CLI, OpenCode and Copilot CLI only.', '') : reason
        expect(aboutThisRuntime, `${runtime} / ${mode}: ${reason}`).not.toContain('Codex CLI')
      }
    }
    // Muse has Auto since 0.773; Approve each is the mode it still refuses.
    expect(modeUnavailableReason('approve-each', 'muse', 'win32')).toContain('Muse Code cannot stop and ask')
  })

  it('bars nothing by platform now: Cursor answers read-only everywhere (0.485)', () => {
    // Its Windows bar was the only one; ask mode lifted it.
    for (const platform of ['win32', 'darwin', 'linux', undefined]) {
      expect(modeRunsOn('ask', 'cursor', platform)).toBe(true)
    }
  })
})
