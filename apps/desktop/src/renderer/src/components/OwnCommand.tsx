import type { ReactElement } from 'react'
import { OWN_COMMAND_ALLOW, type ConversationCommand } from '../../../shared/own-commands.js'
import { onTheComputer } from '../onTheComputer.js'

/** The box, once a draft starts with `!`. Escape leaves it; nothing is sent. */
export function CommandModeNote(): ReactElement {
  return (
    <div className="lc-commandmode" role="status">
      Command mode. This runs in this folder. Escape leaves it.
    </div>
  )
}

/**
 * A project's `!` blocks, named, before they run. Allowing is for the folder,
 * once. Not now leaves the box as it was.
 */
export function CommandAsk({ lines, onAllow, onNotNow }: {
  readonly lines: readonly string[]
  readonly onAllow: () => void
  readonly onNotNow: () => void
}): ReactElement {
  // A folder's commands are allowed on the computer they run on (0.850): a browser holds Allow, and says so.
  const stays = onTheComputer(OWN_COMMAND_ALLOW)
  return (
    <div className="lc-commandask" role="dialog" aria-label="Allow this folder's commands">
      <p>This folder&apos;s commands have not been allowed to run yet. Allow them once, and these will run:</p>
      <ul className="lc-commandask__lines">
        {lines.map((line) => <li key={line} className="lc-mono">{line}</li>)}
      </ul>
      {stays !== undefined && <p className="lc-oncomputer__said">{stays}</p>}
      <div className="lc-commandask__actions">
        <button type="button" className="lc-button" onClick={onAllow} disabled={stays !== undefined} {...(stays === undefined ? {} : { title: stays })}>Allow</button>
        <button type="button" className="lc-button" onClick={onNotNow}>Not now</button>
      </div>
    </div>
  )
}

/** The person's own command and its output. Not a teammate's message. */
export function CommandEntry({ command, onStop }: {
  readonly command: ConversationCommand
  readonly onStop?: (id: string) => void
}): ReactElement {
  const status = command.running ? 'Running' : command.stopped ? 'Stopped' : command.exitCode === null ? 'Ended' : `Exit ${String(command.exitCode)}`
  return (
    <div className="lc-command">
      <div className="lc-bubble">{`!${command.command}`}</div>
      {command.output.length > 0 && <div className="lc-command__output">{command.output}</div>}
      {command.omitted !== undefined && <p className="lc-command__note">{command.omitted}</p>}
      <div className="lc-command__bar">
        <span className="lc-mono">{status}</span>
        {command.running && onStop !== undefined && (
          <button type="button" className="lc-button" aria-label="Stop this command" onClick={() => onStop(command.id)}>Stop</button>
        )}
      </div>
    </div>
  )
}
