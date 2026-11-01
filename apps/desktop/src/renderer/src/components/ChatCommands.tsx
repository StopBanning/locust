import { useEffect, useState } from 'react'
import type { ReactElement } from 'react'
import { OWN_COMMAND_OPEN, type ListedCommand } from '../../../shared/own-commands.js'
import { failureSaid, onTheComputer } from '../onTheComputer.js'

/** Settings > Chat: the commands on this computer and in this folder, and where their files are. */
export function ChatCommands(): ReactElement {
  const [commands, setCommands] = useState<readonly ListedCommand[]>([])
  const [note, setNote] = useState<string>()
  // The folder opens in the computer's file manager (0.849).
  const openStays = onTheComputer(OWN_COMMAND_OPEN)
  useEffect(() => {
    let gone = false
    void window.desktop?.ownCommands?.list({}).then((answer) => {
      if (!gone && answer.ok) setCommands(answer.commands)
    }).catch(() => undefined)
    return () => { gone = true }
  }, [])
  const open = (which: 'global' | 'project'): void => {
    void window.desktop?.ownCommands?.openFolder(which, {}).then((answer) => {
      if (answer !== undefined && !answer.ok) setNote(answer.message)
    }).catch((error: unknown) => setNote(failureSaid(error, 'That folder could not be opened. Your commands are where they were.')))
  }
  const global = commands.filter((command) => command.source === 'global')
  const project = commands.filter((command) => command.source === 'project')
  const rows = (list: readonly ListedCommand[], empty: string) => list.length === 0
    ? <span className="lc-settings__note">{empty}</span>
    : list.map((command) => (
        <span key={`${command.source}:${command.name}`} className="lc-commandlist__name">
          <span className="lc-mono">/{command.name}</span>
          {command.description !== undefined && <span className="lc-settings__note">{command.description}</span>}
        </span>
      ))
  /*
   * A ROW EACH, AS EVERY OTHER SETTINGS PAGE DRAWS ITS PLACES (0.800): the two places were headings in the code
   * face at nearly a heading's size, with an Open folder button under each, on the one page that looked like that.
   * Now each is a row of the page's own box: where, what is there, and Open folder at its end.
   */
  return (
    <div className="lc-commandlist">
      <div className="lc-settingrows">
        <div className="lc-settingrow">
          <div className="lc-settingline__text">
            <span className="lc-commandlist__where">On this computer</span>
            {rows(global, 'None yet. Add a Markdown file to the folder.')}
          </div>
          <button type="button" className="lc-button" aria-label="Open your commands folder" disabled={openStays !== undefined} {...(openStays === undefined ? {} : { title: openStays })} onClick={() => open('global')}>Open folder</button>
        </div>
        <div className="lc-settingrow">
          <div className="lc-settingline__text">
            <span className="lc-commandlist__where">This folder</span>
            {rows(project, 'None in this folder yet.')}
          </div>
          <button type="button" className="lc-button" aria-label="Open this folder's commands" disabled={openStays !== undefined} {...(openStays === undefined ? {} : { title: openStays })} onClick={() => open('project')}>Open folder</button>
        </div>
      </div>
      {note !== undefined && <p className="lc-settings__note">{note}</p>}
    </div>
  )
}
