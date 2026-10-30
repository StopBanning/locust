import type { ReactElement, ReactNode } from 'react'

import { Icon } from './Icon.js'
import { needsYouChip } from '../needsYou.js'

/**
 * Window chrome: the mono workspace title centered, run state at the right.
 * On Windows and Linux the window is frameless and the controls are ours, at
 * the right. On a Mac they are macOS's own traffic lights, which the window
 * puts in this bar's empty left column (the first Mac run, 2026-10-30, found
 * Windows' minimize, maximize and close drawn at the right there).
 */
export function TitleBar({
  workspaceName,
  runningCount,
  swarm,
  needsYou = 0,
  onNeedsYou,
  browser,
  terminal
}: {
  readonly workspaceName: string
  readonly runningCount: number
  readonly swarm: boolean
  /** How many things wait on the person (needsYou.ts); the chip shows only above zero. */
  readonly needsYou?: number
  /** Opens the list under the chip. */
  readonly onNeedsYou?: (anchor: HTMLElement) => void
  readonly browser?: ReactNode
  readonly terminal?: ReactNode
}): ReactElement {
  return (
    <header className="lc-titlebar" onDoubleClick={() => window.desktop?.toggleMaximize()}>
      <span />
      <span className="lc-titlebar__title">{workspaceName}</span>
      <div className="lc-titlebar__right" onDoubleClick={(event) => event.stopPropagation()}>
        {browser}
        {terminal}
        {/* A workspace-wide setting deserves a persistent, visible statement. */}
        {swarm && <span className="lc-swarmchip">Swarm · every run at max effort</span>}
        {/* What waits on the person, before what is merely running: the list is one press away. */}
        {needsYou > 0 && onNeedsYou !== undefined && (
          <button type="button" className="lc-needsyou" aria-haspopup="menu" onClick={(event) => onNeedsYou(event.currentTarget)}>
            <span className="lc-dot lc-tone-amber" />
            {needsYouChip(needsYou)}
          </button>
        )}
        {runningCount > 0 && (
          <span className="lc-runstate">
            <span className="lc-dot is-pulsing lc-tone-live" />
            {runningCount} running
          </span>
        )}
        {/* In a browser (0.830) the browser's own window has these, and on a Mac macOS draws its own. */}
        {(typeof window === 'undefined' || (window.desktop?.remote === undefined && window.desktop?.platform !== 'darwin')) && <div className="lc-windowcontrols">
          <button type="button" aria-label="Minimize" onClick={() => window.desktop?.minimize()}>
            <Icon name="minimize" size={14} />
          </button>
          <button type="button" aria-label="Maximize" onClick={() => window.desktop?.toggleMaximize()}>
            <Icon name="maximize" size={12} />
          </button>
          <button type="button" className="lc-close" aria-label="Close" onClick={() => window.desktop?.close()}>
            <Icon name="close" size={14} />
          </button>
        </div>}
      </div>
    </header>
  )
}
