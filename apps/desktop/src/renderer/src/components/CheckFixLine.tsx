import type { ReactElement } from 'react'

import { Icon } from './Icon.js'

/**
 * FIX UNTIL THE CHECK PASSES (0.812, shared/check-fix.ts): the turn Locust sent, where the person's message would be.
 *
 * Nobody typed it, so it is never drawn as a message from you -- the rule a turn carried on after a plan ran out keeps
 * (0.774). Claude Code shows a Stop hook's word the same way: a line in the transcript, its full text a click away.
 */
export function CheckFixLine({
  command,
  fix,
  of,
  prompt,
  teammateName
}: {
  readonly command: string
  readonly fix: number
  readonly of: number
  /** What Locust sent: what the check printed, and what the teammate was asked to do about it. */
  readonly prompt: string
  /** Whom it went back to; a conversation with nobody has no name to say. */
  readonly teammateName?: string
}): ReactElement {
  const to = teammateName === undefined ? '' : ` to ${teammateName}`
  return (
    <div className="lc-checkfixline" role="note">
      <span className="lc-checkfixline__said">
        <Icon name="refresh" size={12} />
        <span>
          <code className="lc-mono">{command}</code>
          {fix === 1 ? ` failed, so Locust sent it back${to}` : ` still failed, so Locust sent it back${to} again`}
        </span>
        <span className="lc-checkfixline__count lc-mono">{`· fix ${String(fix)} of ${String(of)}`}</span>
      </span>
      <details className="lc-checkfixline__sent">
        <summary>What Locust sent</summary>
        <pre className="lc-mono">{prompt}</pre>
      </details>
    </div>
  )
}
