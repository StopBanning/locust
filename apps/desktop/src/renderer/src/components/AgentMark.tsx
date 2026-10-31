import type { ReactElement } from 'react'

import { usageReadLine, usageWindowSentence, usageWindowsOf } from '../missionView.js'
import { RuntimeMark } from './RuntimeMark.js'

/**
 * AN AGENT'S MARK ON HOME, AND WHAT IT KNOWS ON HOVER (0.389).
 *
 * 0.388 ringed each mark with its account's usage, and the row of marks lost
 * the calm it had. Colin, 2026-09-27: "it was just very clean showing the
 * agents earlier ... im sure you can cook up something better", then "when you
 * hover the logo the usage window can appear" -- and, of 0.388, "the hover
 * logo wasnt working": an SVG's own title is not a hover a person can rely on.
 *
 * So the row is marks again, and each mark is a thing to point at: hover or
 * focus it and a card opens above it -- the agent's name and state, and one
 * bar for each usage window its last run reported, with when it resets. A bar
 * is amber from 80% and red when the window is spent, the tones the app uses
 * for "may not finish" and "stopped". Upward, because the row sits just over
 * the message box.
 *
 * Only what the runs reported (Claude's windows; Codex's since 0.388). An
 * agent that reported none gets its name and state alone, not an empty meter.
 */
export interface AgentMarkProps {
  readonly runtime: string
  readonly name: string
  /** "Ready · 2.1.283". */
  readonly state: string
  /** The latest usage reading for this agent's account, as the host keeps it. */
  readonly usage?: string
  /** The clock a reset is compared against; tests pass one. */
  readonly now?: Date
}

export function AgentMark({ runtime, name, state, usage, now = new Date() }: AgentMarkProps): ReactElement {
  const windows = usage === undefined ? [] : usageWindowsOf(usage, now)
  const said = [name, state, ...(usage === undefined || windows.length === 0 ? [] : [usageWindowSentence(usage, now)])].join('. ')
  return (
    <span className="lc-agentmark" tabIndex={0} role="img" aria-label={said}>
      <RuntimeMark runtime={runtime} size={15} />
      <AgentCard runtime={runtime} name={name} state={state} {...(usage === undefined ? {} : { usage })} now={now} />
    </span>
  )
}

/**
 * THE CARD ITSELF (2026-10-30): the agent's name and state, a bar a usage window, and when its reading was taken.
 * Drawn by the page, not left to the system's tooltip: a mark's SVG title was "not a hover a person can rely on"
 * (0.389), and the chat box's model chip carried Codex's usage only in its `title`, which a Mac shows only while
 * Locust is the app in front, and after a pause -- Colin, on the Mac: "hovering isnt showing usage window for codex
 * ... model picker usually does it". So the chip shows this same card above it (Composer.tsx), as the marks on Home do.
 * `className` places it: the marks centre it over themselves; the chip, at the window's right edge, aligns it right.
 */
export function AgentCard({ runtime, name, state, usage, now = new Date(), className }: AgentMarkProps & { readonly className?: string }): ReactElement {
  const windows = usage === undefined ? [] : usageWindowsOf(usage, now)
  const read = usage === undefined ? undefined : usageReadLine(usage)
  return (
    <span className={`lc-agentcard${className === undefined ? '' : ` ${className}`}`} aria-hidden="true">
      <span className="lc-agentcard__head">
        <RuntimeMark runtime={runtime} size={16} />
        <span className="lc-agentcard__name">{name}</span>
        <span className="lc-agentcard__state">{state}</span>
      </span>
      {windows.map((window) => {
        // Reset since the reading: its percentage is no longer true (0.406).
        if (window.expired === true) {
          return (
            <span className="lc-agentcard__window is-reset" key={window.name}>
              <span className="lc-agentcard__label">{window.name.charAt(0).toUpperCase() + window.name.slice(1)}</span>
              <span className="lc-agentcard__resets">reset since{window.resets === undefined ? '' : ` (${window.resets})`}</span>
            </span>
          )
        }
        const tone = window.percent >= 100 ? ' is-spent' : window.percent >= 80 ? ' is-pressing' : ''
        return (
          <span className={`lc-agentcard__window${tone}`} key={window.name}>
            <span className="lc-agentcard__label">{window.name.charAt(0).toUpperCase() + window.name.slice(1)}</span>
            <span className="lc-agentcard__percent">{window.remaining ?? window.percent}%{window.remaining === undefined ? '' : ' left'}</span>
            <span className="lc-agentcard__bar">
              <span className="lc-agentcard__fill" style={{ width: `${String(window.percent)}%` }} />
            </span>
            {window.resets !== undefined && <span className="lc-agentcard__resets">resets {window.resets}</span>}
          </span>
        )
      })}
      {windows.length > 0 && read !== undefined && <span className="lc-agentcard__read">{read}</span>}
    </span>
  )
}
