import { useRef, type ReactElement } from 'react'
import { setupSections, type TeammateSetup as Setup } from '../../../shared/teammate-setup.js'
import { useModal } from '../useModal.js'

/**
 * A TEAMMATE'S SETUP (0.811, shared/teammate-setup.ts): `/setup` in its conversation, as Claude Code's `/status` is
 * for a session -- what shapes its turns, in the order a turn is given them, read fresh. It changes nothing; Edit
 * teammate is one press away for what can be changed there.
 */
export function TeammateSetup({ name, runtime, model, setup, problem, onEdit, onClose }: {
  readonly name: string
  /** The runtime as the window names it: "Claude Code". */
  readonly runtime: string
  /** "Claude Code / Haiku 5.5 · auto", as the window says it. */
  readonly model: string
  readonly setup?: Setup | undefined
  readonly problem?: string | undefined
  readonly onEdit: () => void
  readonly onClose: () => void
}): ReactElement {
  const box = useRef<HTMLDivElement>(null)
  useModal(box, onClose)
  const sections = setup === undefined ? [] : setupSections(setup, name, runtime)
  return (
    <div className="lc-scrim">
      <div ref={box} className="lc-dialog lc-setup" role="dialog" aria-modal="true" aria-label={`${name}’s setup`}>
        <div className="lc-dialog__head">
          <span className="lc-dialog__title">Setup</span>
          <span className="lc-dialog__sub">{name}</span>
          <button type="button" className="lc-dialog__close" aria-label="Close" onClick={onClose}>×</button>
        </div>
        <div className="lc-dialog__body lc-setup__body">
          <dl className="lc-setup__rows">
            <div className="lc-setup__row">
              <dt>Model</dt>
              <dd>{model}</dd>
            </div>
          </dl>
          {problem !== undefined && <p className="lc-dialog__error" role="alert">{problem}</p>}
          {setup === undefined && problem === undefined && <p className="lc-dialog__note" role="status">Reading what shapes {name}’s turns…</p>}
          {sections.map((section) => (
            <section key={section.title} className="lc-setup__section" aria-label={section.title}>
              <h3 className="lc-fieldlabel lc-mono lc-setup__title">{section.title}</h3>
              <p className="lc-dialog__note lc-setup__note">{section.note}</p>
              <dl className="lc-setup__rows">
                {section.rows.map((row) => (
                  <div key={row.label} className="lc-setup__row">
                    <dt>{row.label}</dt>
                    <dd>{row.value}</dd>
                  </div>
                ))}
              </dl>
            </section>
          ))}
        </div>
        <div className="lc-dialog__foot">
          <button type="button" className="lc-ghostbutton" onClick={onClose}>Close</button>
          <button type="button" className="lc-primarybutton" onClick={onEdit}>Edit teammate</button>
        </div>
      </div>
    </div>
  )
}
