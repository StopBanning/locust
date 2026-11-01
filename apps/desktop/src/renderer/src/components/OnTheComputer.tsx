import type { ReactElement, ReactNode } from 'react'

import { WINDOW_ONLY_SAYS } from '../../../shared/desk-calls.js'
import { onTheComputer } from '../onTheComputer.js'

/**
 * Controls whose calls stay on the computer (0.849). In a browser they are drawn as they are, disabled together (a
 * disabled fieldset disables every button, field and select inside it), with the words that say where they work
 * under them. On the computer, and wherever the call works, the children are drawn untouched. Wrap the line that holds
 * the controls, not a whole section: a "How it works" inside would be disabled too.
 */
export function OnTheComputer({ channel, args = [], children }: {
  readonly channel: string | readonly string[]
  /** The call's arguments, where a browser may make it only some ways (desk-calls.ts SOME): Scout's fixes. */
  readonly args?: readonly unknown[]
  readonly children: ReactNode
}): ReactElement {
  const said = (typeof channel === 'string' ? [channel] : channel).map((one) => onTheComputer(one, args)).find((words) => words !== undefined)
  if (said === undefined) return <>{children}</>
  return (
    <fieldset className="lc-oncomputer" disabled>
      {children}
      <p className="lc-settings__note lc-oncomputer__said">{said}</p>
    </fieldset>
  )
}

/**
 * A section whose controls, and the state they show, all stay on the computer (0.849): in a browser, its name, what it
 * does and where it works, with nothing to press and nothing read that a browser would be refused. Draw it in the
 * section's place, rather than the section, so none of the section's reads are tried.
 */
export function OnTheComputerOnly({ heading, lede }: { readonly heading: ReactNode; readonly lede: string }): ReactElement {
  return (
    <div className="lc-settingline" data-on-the-computer="">
      <div className="lc-settingline__text">
        {heading}
        <p className="lc-settings__lede">{lede}</p>
        <p className="lc-settings__note lc-oncomputer__said">{WINDOW_ONLY_SAYS}</p>
      </div>
    </div>
  )
}
