import { useRef, useState } from 'react'
import type { ReactElement } from 'react'
import { useModal } from '../useModal.js'

import type { PublicGroup, TeammateRoute } from '../../../shared/ipc.js'
import { routeChrome, routeModelName } from '../routeName.js'

/** The store's own cap, said here so the box cannot promise more than the file keeps. */
export const MAX_GROUP_INSTRUCTIONS = 4_000
/** The counter appears only once it is worth reading. */
export const COUNTER_FROM = 3_600

/** What a route is called on screen: the same names the composer's chip uses. */
export function routeLabel(route: TeammateRoute): string {
  return routeChrome(route.runtime, route.model, routeModelName(route.runtime, route.model))
}

/**
 * A group's two properties, edited together.
 *
 * The design agent's ruling of 2026-09-16, against the 0.162.0 frames: a
 * menu holds actions; a dialog holds what a thing carries. A group carries
 * three things -- it files, it briefs, it routes. Filing is the sidebar. The
 * other two are properties, and a property that has to be STATED in order
 * to be edited cannot live on a menu row: the route line had ended up doing
 * four jobs in one 560px strip. So both live here, under one title that
 * names neither, and the menu is three short verbs again.
 *
 * The words say what is true and nothing more. Above the box, read before
 * anything is typed: turns started from now on; turns already run were not
 * briefed. Below it, in mono, the provenance. How to stop needs no
 * sentence: empty the box and Save becomes Clear instructions.
 */
export function GroupSettingsDialog({
  group,
  currentRoute,
  onSave,
  onCancel
}: {
  readonly group: PublicGroup
  /** What the composer is set to now: what "Use current" would take. */
  readonly currentRoute: TeammateRoute
  readonly onSave: (next: {
    readonly instructions: string
    readonly route: TeammateRoute | undefined
    readonly routeChanged: boolean
  }) => void
  readonly onCancel: () => void
}): ReactElement {
  const [text, setText] = useState(group.instructions)
  const [route, setRoute] = useState<TeammateRoute | undefined>(group.route)
  const [routeChanged, setRouteChanged] = useState(false)
  const trimmed = text.trim()
  const textChanged = trimmed !== group.instructions.trim()
  const clearing = trimmed.length === 0 && group.instructions.trim().length > 0
  // Focus in, Tab held inside, Escape closes, focus back to the opener.
  const box = useRef<HTMLDivElement>(null)
  useModal(box, onCancel)
  return (
    <div className="lc-scrim">
      <div ref={box} className="lc-dialog lc-groupsettings" role="dialog" aria-modal="true" aria-label={`Project settings for ${group.name}`}>
        <div className="lc-dialog__head">
          <span className="lc-dialog__title">Project settings</span>
          <span className="lc-dialog__sub">{group.name}</span>
          <button type="button" className="lc-dialog__close" aria-label="Close" onClick={onCancel}>
            ×
          </button>
        </div>
        <div className="lc-dialog__body lc-groupsettings__body">
          <section className="lc-groupsettings__section">
            <div className="lc-groupsettings__labelrow">
              <span className="lc-groupsettings__label">Standing instructions</span>
              {text.length > COUNTER_FROM && (
                <span className="lc-groupsettings__count lc-mono">
                  {String(text.length)} / {String(MAX_GROUP_INSTRUCTIONS)}
                </span>
              )}
            </div>
            <p className="lc-groupsettings__claim">Briefs every turn started from now on. Turns already run were not briefed.</p>
            <textarea
              className="lc-input lc-groupsettings__text"
              aria-label="Standing instructions"
              rows={5}
              maxLength={MAX_GROUP_INSTRUCTIONS}
              // This read "Quote sizes in shares. Analysis only; never propose a
              // trade." -- a leftover from a finance example, and the first
              // sentence a person reads when they open the new feature in what
              // is a coding-agent control room (outside tester, 0.164.0).
              placeholder="Run the tests before calling anything done. Keep changes to the files the task names."
              value={text}
              onChange={(event) => setText(event.target.value)}
            />
            <span className="lc-groupsettings__foot lc-mono">Given after the folder&apos;s own LOCUST.md.</span>
          </section>

          <section className="lc-groupsettings__section lc-groupsettings__section--route">
            <div className="lc-groupsettings__labelrow">
              <span className="lc-groupsettings__label">Default route</span>
            </div>
            <p className="lc-groupsettings__claim">
              Selected in the composer when a conversation joins this project. Each conversation owns its route after that.
            </p>
            <div className="lc-groupsettings__route">
              <span className={`lc-groupsettings__dot${route === undefined ? '' : ' is-set'}`} aria-hidden="true" />
              <span className={`lc-groupsettings__routetext${route === undefined ? ' is-none' : ''}`}>
                {route === undefined ? 'None — each conversation keeps what it has' : routeLabel(route)}
              </span>
              <span className="lc-groupsettings__routeactions">
                <button
                  type="button"
                  className="lc-button lc-groupsettings__use"
                  onClick={() => {
                    setRoute(currentRoute)
                    setRouteChanged(true)
                  }}
                >
                  Use current
                </button>
                {/* Disabled rather than hidden: a control that vanishes changes the row's shape under the pointer. */}
                <button
                  type="button"
                  className="lc-button lc-groupsettings__clear"
                  disabled={route === undefined}
                  onClick={() => {
                    setRoute(undefined)
                    setRouteChanged(true)
                  }}
                >
                  Clear
                </button>
              </span>
            </div>
            <span className="lc-groupsettings__foot lc-mono">
              {route === undefined
                ? `Use current takes ${routeLabel(currentRoute)} from the composer.`
                : `Composer now shows ${routeLabel(currentRoute)}.`}
            </span>
          </section>
        </div>
        <div className="lc-dialog__foot">
          <button type="button" className="lc-button" onClick={onCancel}>
            Cancel
          </button>
          <button
            type="button"
            className={`lc-primarybutton${clearing ? ' lc-groupsettings__save--clearing' : ''}`}
            disabled={!textChanged && !routeChanged}
            onClick={() => onSave({ instructions: trimmed, route, routeChanged })}
          >
            {clearing ? 'Clear instructions' : 'Save'}
          </button>
        </div>
      </div>
    </div>
  )
}
