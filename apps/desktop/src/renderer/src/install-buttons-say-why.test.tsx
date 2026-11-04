import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import type { LocalRuntimeId, PublicRuntimeStatus } from '../../shared/ipc.js'
import { FirstLaunch } from './components/FirstLaunch.js'

/**
 * A switched-off Install button says why on the screen, not on hover.
 *
 * Reported by the first outside tester on 0.55.0, in their own words: "it
 * shows up as either get it or install but they do nothing", and "one of
 * them is highlighted green but still no difference". They also said, in the
 * same breath, "it automatically connected with codex" — which is the whole
 * bug.
 *
 * Four of the five runtimes install through npm, so with no Node.js on the
 * machine every Install button is correctly disabled. The sentence saying so
 * was gated on `connected === 0`. Codex signs itself in on many machines, so
 * the moment it was found the count stopped being zero, the explanation
 * disappeared, and the reason survived only in a `title` tooltip. What is
 * left is a panel of buttons that do nothing, with nothing on screen to say
 * why — which is exactly what was described.
 *
 * The green one is the free runtime's primary Install button, disabled like
 * all the others and looking the most pressable of the lot.
 */

/** A real status record rather than a cast, so a field added upstream fails here. */
const runtime = (id: LocalRuntimeId, name: string, connected: boolean): PublicRuntimeStatus => ({
  id,
  displayName: name,
  installed: connected,
  version: connected ? '1.0.0' : null,
  auth: connected ? 'authenticated' : 'unknown',
  ready: connected,
  status: connected ? 'ready' : 'not-installed'
})

function panel(options: { connected: number; npmMissing: boolean; npmIsBundled?: boolean; installing?: string }): string {
  // One connected runtime standing in for Codex, plus ones that are not.
  const runtimes = [
    runtime('codex', 'Codex', options.connected > 0),
    runtime('claude', 'Claude Code', false),
    runtime('opencode', 'OpenCode', false),
    runtime('cursor', 'Cursor', false)
  ]
  return renderToStaticMarkup(
    <FirstLaunch
      runtimes={runtimes}
      limitedRuntimes={new Map()}
      discoveryPhase="ready"
      workspacePath="C:\\work"
      teammateCount={1}
      onChooseFolder={() => undefined}
      onInstall={() => undefined}
      npmMissing={options.npmMissing}
      npmIsBundled={options.npmIsBundled ?? false}
      {...(options.installing === undefined ? {} : { installing: options.installing })}
    />
  )
}

describe('when Node.js is missing', () => {
  it('renders the panel at all', () => {
    // The control. A panel that threw or drew nothing would satisfy every
    // `not.toContain` below.
    expect(panel({ connected: 0, npmMissing: true })).toContain('Install')
  })

  it('says so even after a runtime has connected', () => {
    /*
     * THE regression. With Codex connected the count is 1, and the sentence
     * explaining every disabled button vanished.
     */
    const withCodex = panel({ connected: 1, npmMissing: true })
    expect(withCodex).toContain('Node.js is not on this machine')
    expect(withCodex).toContain('cannot run')
  })

  it('still says so when nothing at all has connected', () => {
    expect(panel({ connected: 0, npmMissing: true })).toContain('Node.js is not on this machine')
  })

  it('says nothing about Node when npm is there', () => {
    expect(panel({ connected: 1, npmMissing: false })).not.toContain('Node.js is not on this machine')
  })

  it('offers a way to get Node that this app can actually open', () => {
    const withCodex = panel({ connected: 1, npmMissing: true })
    expect(withCodex).toContain('Get Node.js')
    // Not an anchor. The host denies window.open and cancels navigation, so
    // the link that was here did nothing at all -- and it was the only way
    // out offered to the person who had no Node.
    expect(withCodex).not.toContain('<a href="https://nodejs.org"')
    expect(withCodex).toContain('lc-linkbutton')
  })
})

/**
 * And when the app carries its own npm, the buttons are NOT switched off.
 *
 * Ian, 2026-09-17: he downloaded Locust and nothing worked until he
 * installed Node.js. The screen was honest -- every button said why it could
 * not run -- but honest about a limit that did not have to exist. An Electron
 * binary started with ELECTRON_RUN_AS_NODE=1 is a Node runtime (24.18.1 in
 * the packaged build), and npm is a node script, so the app had a working npm
 * the whole time.
 *
 * Measured on a real packaged build, 2026-09-18: that npm installed a package
 * with no Node anywhere on PATH, in one second.
 *
 * What the note must NOT do is overclaim. npm writes launcher shims that call
 * node by name, so the CLI it installs is reachable from Locust and not from
 * the person's own terminal until they install Node themselves.
 */
describe('when this app carries its own npm', () => {
  /*
   * WHILE AN INSTALL IS ACTUALLY HAPPENING, which is when this sentence is
   * for.
   *
   * It used to be drawn on the cold first screen, before the person had
   * pressed anything -- and measured on the first frame anybody has taken of
   * that screen with nothing installed, it was the longest run of prose on
   * it. "Node.js is not on this machine" reads as a problem statement to
   * anybody who does not already know what Node.js is, which is exactly the
   * person this screen is being rebuilt for (2026-09-19).
   *
   * The content below is unchanged and still Ian's case. Only the moment
   * moved: from before the press to from the press onward.
   */
  const carried = panel({ connected: 0, npmMissing: false, npmIsBundled: true, installing: 'opencode' })

  it('does not disable a single Install button', () => {
    // The whole point. `npmMissing` is what disables them, and a build that
    // ships npm has no reason to set it.
    expect(carried).toContain('Install')
    expect(carried).not.toContain('disabled=""')
  })

  it('does not tell the person the buttons cannot run', () => {
    expect(carried).not.toContain('cannot run')
  })

  it('says where the install came from, since the buttons working is the surprise', () => {
    expect(carried).toContain('the copy of npm it carries')
  })

  it('says the part that is still true: their own terminal is unchanged', () => {
    // Without this the person installs a CLI, opens a terminal, and finds
    // nothing -- which reads as the install having failed.
    expect(carried).toContain('your own terminal')
    expect(carried).toContain('lc-linkbutton')
  })

  it('says none of it on a machine that has its own Node', () => {
    const ordinary = panel({ connected: 1, npmMissing: false })
    expect(ordinary).not.toContain('the copy of npm it carries')
    expect(ordinary).not.toContain('Node.js is not on this machine')
  })
})

/**
 * And WHEN each of those sentences appears, which is its own decision.
 *
 * The first screen on a machine with nothing installed was measured for the
 * first time on 2026-09-19 (`_tools/bare-machine-frame.mjs` — every frame
 * before it had been taken on a machine with all six agents present, reading
 * "6 ready"). Two things on it spoke too early for the person it is for.
 */
describe('what the cold first screen does not say yet', () => {
  it('keeps the Node.js explanation until the person has pressed something', () => {
    const cold = panel({ connected: 0, npmMissing: false, npmIsBundled: true })
    expect(cold).not.toContain('the copy of npm it carries')
    // And the thing it DOES say is the step: one lit row with one button.
    expect(cold).toContain('Install')
  })

  it('still says it once anything is connected, which is when a terminal comes up', () => {
    // Ian's case is not "while installing", it is "afterwards, in my own
    // terminal, nothing is there". So the sentence has to outlive the spinner.
    const after = panel({ connected: 1, npmMissing: false, npmIsBundled: true })
    expect(after).toContain('your own terminal')
  })

  it('stops explaining the missing Node once the install has finished', () => {
    /*
     * Sol's beta review, 2026-09-21, finding 8: OpenCode Ready, catalogue up,
     * and the screen still opened a paragraph with "Node.js is not on this
     * machine" -- a dependency complaint under an install that had just
     * worked.
     *
     * THE TEST ABOVE IS WHY THIS IS A SPLIT AND NOT A DELETION. Ian needs the
     * terminal half afterwards; Sol is right that the bundled-npm half has
     * nothing to say to him by then. The pair below is the whole rule, and
     * either one failing means the paragraph was merged back together.
     */
    const during = panel({ connected: 0, npmMissing: false, npmIsBundled: true, installing: 'opencode' })
    expect(during).toContain('the copy of npm it carries')

    const after = panel({ connected: 1, npmMissing: false, npmIsBundled: true })
    expect(after).not.toContain('the copy of npm it carries')
    expect(after).not.toContain('Node.js is not on this machine')
  })

  it('offers the other agents behind a press while none of them can help', () => {
    /*
     * Colin, 2026-09-19: "i really want a new user without tech savvyness to
     * be able to just use the software off rip, maybe even ask them how to get
     * the other agents working" -- the other agents come after, in his own
     * sentence. Five rows each naming an account or a subscription the person
     * does not have are five reasons not to press the one button that works.
     *
     * Deferred, not hidden: the count is in the control's own words.
     */
    const cold = panel({ connected: 0, npmMissing: false, npmIsBundled: true })
    expect(cold).toContain('Locust can drive')
    expect(cold).not.toContain('needs a Cursor account')
    // The on-ramp itself is never deferred: it is the step.
    expect(cold).toContain('OpenCode')
    expect(cold).toContain('no account needed')
  })

  it('offers an agent nobody has added yet as ready to install, never as "not installed"', () => {
    /*
     * 0.866: the AppImage catalog's test reads the first window's words and fails an app whose screen says "not
     * installed", the words an install error uses. On Home they were OpenCode's row, the one step that works: an
     * offer, not a fault, so the row says what pressing Install does.
     */
    const cold = panel({ connected: 0, npmMissing: false, npmIsBundled: true })
    expect(cold).toContain('ready to install')
    expect(cold).not.toMatch(/not installed/i)
    const warm = panel({ connected: 1, npmMissing: false })
    expect(warm).not.toMatch(/not installed/i)
  })

  it('shows every agent again the moment one of them works', () => {
    // The control. Once anything is connected, "what else can this drive" is
    // a real question and the catalogue is the right answer to it.
    const warm = panel({ connected: 1, npmMissing: false })
    expect(warm).toContain('needs a Cursor account')
    expect(warm).not.toContain('Locust can drive')
  })
})

/**
 * And a row that is ON THIS MACHINE is never deferred.
 *
 * Fable, pass 2, on 0.198.0 — the day the deferral shipped — on a Linux box
 * with four CLIs installed and hung: the screen drew ONE row while the head
 * note read *6 on this machine · none answering*, the composer said *A coding
 * agent is installed but not answering — Check again above*, and the four
 * hanging CLIs sat behind *5 others Locust can drive — they each need their
 * own account*. They need no account, they are here, and the Check again that
 * would repair them was hidden behind a control giving no reason to press it.
 *
 * The frame the deferral was designed against had nothing installed, where
 * "not connected" and "not installed" are the same rows. They are not.
 */
describe('the deferral, on a machine that HAS some of them', () => {
  const withInstalled = (): string => {
    const runtimes = [
      runtime('opencode', 'OpenCode', false),
      // Installed and usable is `connected`; this is installed and NOT usable,
      // which is the hung and the signed-out row both.
      { ...runtime('codex', 'Codex', false), installed: true },
      runtime('cursor', 'Cursor', false)
    ]
    return renderToStaticMarkup(
      <FirstLaunch
        runtimes={runtimes}
        limitedRuntimes={new Map()}
        discoveryPhase="ready"
        workspacePath="C:\work"
        teammateCount={1}
        onChooseFolder={() => undefined}
        onInstall={() => undefined}
        checkingGaveUp
      />
    )
  }

  it('draws the installed one even while nothing is connected', () => {
    expect(withInstalled()).toContain('Codex')
  })

  it('still defers the ones that are only products you could buy', () => {
    const html = withInstalled()
    expect(html).toContain('Locust can drive')
    expect(html).not.toContain('Cursor')
  })

  it('counts only what it actually deferred', () => {
    // The head note's arithmetic and the control's must describe the same
    // screen. Three rows, two drawn, so the control says one.
    expect(withInstalled()).toMatch(/1 other Locust can drive/)
  })
})
