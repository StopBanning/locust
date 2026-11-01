import { useRef, useState } from 'react'
import type { ReactElement } from 'react'

import { useModal } from '../useModal.js'
import { MISSION_RECORD_SAVE_CHANNEL } from '../../../shared/ipc.js'
import { onTheComputer } from '../onTheComputer.js'
import { Icon } from './Icon.js'

/**
 * SAVE THE RECORD (0.575): one conversation as one Markdown file.
 *
 * The one place the person is told what the file is. The words are said once,
 * here, and not again in the file or on the way out: it holds the whole
 * conversation, and what happens to it after it is saved is theirs to decide.
 * The destination is a native save dialog the host opens; nothing uploads and
 * nothing opens.
 */
export const RECORD_CLAIM = 'This file contains the conversation. Read it before you send it.'

export function SaveRecordDialog({ missionId, onClose }: { readonly missionId: string; readonly onClose: () => void }): ReactElement {
  // It had no Escape, no x and no focus held inside (design pass, 2026-10-19), like Share your team.
  const box = useRef<HTMLDivElement>(null)
  useModal(box, onClose)
  const [includeRaw, setIncludeRaw] = useState(false)
  const [busy, setBusy] = useState(false)
  // The computer's save dialog (0.850): in a browser, Save is held and says where it works.
  const saveStays = onTheComputer(MISSION_RECORD_SAVE_CHANNEL)
  const [said, setSaid] = useState<string | undefined>(saveStays)
  const save = (): void => {
    const bridge = window.desktop
    if (bridge === undefined) {
      setSaid('Saving is not available here. Nothing was written.')
      return
    }
    setBusy(true)
    setSaid(undefined)
    bridge
      .saveMissionRecord({ missionId, includeRaw })
      .then((saved) => {
        if (!saved.ok) setSaid(saved.message)
        // Cancelled in the save dialog: the person is still here, and nothing was written.
        else if (saved.path === undefined) setSaid(undefined)
        else setSaid(saved.rawPath === undefined ? `Saved to ${saved.path}.` : `Saved to ${saved.path}, and the raw record beside it at ${saved.rawPath}.`)
      })
      .catch(() => setSaid('The record could not be saved. Nothing was written.'))
      .finally(() => setBusy(false))
  }
  return (
    <div className="lc-scrim">
      <div ref={box} className="lc-dialog lc-saverecord" role="dialog" aria-modal="true" aria-label="Save the record">
        <div className="lc-dialog__head">
          <span className="lc-dialog__title">Save the record</span>
          <button type="button" className="lc-dialog__close" aria-label="Close" onClick={onClose}>
            <Icon name="close" size={13} />
          </button>
        </div>
        <div className="lc-dialog__body lc-saverecord__body">
          <p className="lc-saverecord__claim">{RECORD_CLAIM}</p>
          <label className="lc-saverecord__with">
            <input type="checkbox" checked={includeRaw} onChange={(event) => setIncludeRaw(event.target.checked)} />
            <span>Include the raw record (JSON, not scrubbed)</span>
          </label>
          {said !== undefined && (
            <p className="lc-saverecord__claim lc-saverecord__said" role="status">
              {said}
            </p>
          )}
        </div>
        <div className="lc-dialog__foot">
          <button type="button" className="lc-button" onClick={onClose}>
            Close
          </button>
          <button type="button" className="lc-primarybutton" disabled={busy || saveStays !== undefined} {...(saveStays === undefined ? {} : { title: saveStays })} onClick={save}>
            Save the record…
          </button>
        </div>
      </div>
    </div>
  )
}
