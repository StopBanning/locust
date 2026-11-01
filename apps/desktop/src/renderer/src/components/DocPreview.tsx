import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import type { ReactElement } from 'react'

import { csvWorkbook } from '../../../shared/sheet.js'
import { extensionOf, viewerMode } from '../../../shared/text-files.js'
import type { DiffCounts, DiffFile } from '../diff.js'
import { DiffView } from './DiffView.js'
import { Icon } from './Icon.js'
import { SheetView } from './SheetView.js'
import { AgentText } from './ThreadItems.js'
import { WORKSPACE_PAGE_CHANNEL } from '../../../shared/ipc.js'
import { onTheComputer } from '../onTheComputer.js'

/**
 * A NEW DOCUMENT, SHOWN AS ONE (0.363).
 *
 * A teammate that wrote a brief, a guide or a draft left it in the thread as
 * a green block of raw Markdown with line numbers -- "+ ## Voice", "+ **We
 * are:** welcoming" -- above the answer that described it (Iris's brand
 * guide, the Write & design drive, packaged 0.362). For a new file every
 * line of that diff is an addition, so the diff says nothing the document
 * does not; it only says it in the form a writer reads least.
 *
 * So a new Markdown file opens in the fold as the page it is: drawn by the
 * same renderer as a reply and the file viewer (which escapes everything and
 * runs nothing -- DECISION-2026-09-20), cut at a readable height with the
 * rest one press away in the viewer. The change is one press away too, for
 * anyone who wants line numbers. A file that was CHANGED still opens as its
 * diff: then what changed is the question.
 */
export function isNewDocument(file: DiffFile): boolean {
  return file.status === 'ADDED' && (viewerMode(file.path) === 'markdown' || isTableFile(file.path) || isPageFile(file.path))
}

/** A new web page is shown running, in a frame of its own (0.425). */
function isPageFile(path: string): boolean {
  const extension = extensionOf(path)
  return extension === 'html' || extension === 'htm'
}

/**
 * A web page a teammate just made, running (0.425): the same frame the viewer
 * uses (main/page-preview.ts), sized to the card. Colin, 2026-09-28: "full
 * functionality, sacrifice nothing."
 */
export function PagePreview({ path, name, column }: { readonly path: string; readonly name: string; readonly column?: { readonly compareId: string; readonly slot: string } }): ReactElement {
  const [url, setUrl] = useState<string>()
  // Served by the computer at an address only it reaches (0.850): a browser says where the page runs.
  const stays = onTheComputer(WORKSPACE_PAGE_CHANNEL) === undefined ? undefined : 'This page runs in Locust on your computer.'
  const [refused, setRefused] = useState<string | undefined>(stays)
  useEffect(() => {
    if (stays !== undefined) return
    let live = true
    void window.desktop?.pageUrlFor(path, column).then((answer) => {
      if (!live) return
      if (answer.ok) setUrl(answer.url)
      else setRefused(answer.message)
    }).catch(() => undefined)
    return () => {
      live = false
    }
  }, [path, column?.compareId, column?.slot])
  return url === undefined ? (
    <p className="lc-docpreview__framewait">{refused ?? 'Opening the page…'}</p>
  ) : (
    <iframe
      className="lc-docpreview__frame"
      src={url}
      title={`${name}, running`}
      sandbox="allow-scripts allow-same-origin allow-forms allow-modals allow-popups allow-downloads"
    />
  )
}

/** A new CSV or TSV is a table, previewed as its first rows (0.364). */
function isTableFile(path: string): boolean {
  const extension = extensionOf(path)
  return extension === 'csv' || extension === 'tsv'
}

/** The text a new file was written with: every added row, in order. */
export function documentTextOf(file: DiffFile): string {
  return file.hunks.flatMap((hunk) => hunk.rows.filter((row) => row.kind === 'add').map((row) => row.text)).join('\n')
}

export function DocPreview({
  file,
  truncated,
  reported,
  onOpen
}: {
  readonly file: DiffFile
  /** The record holds less than the runtime wrote: this is the start of it. */
  readonly truncated: boolean
  readonly reported: DiffCounts | undefined
  /** Open the whole file in the viewer beside the conversation. */
  readonly onOpen?: () => void
}): ReactElement {
  const [asChange, setAsChange] = useState(false)
  const page = useRef<HTMLDivElement>(null)
  const [clipped, setClipped] = useState(false)
  useLayoutEffect(() => {
    const element = page.current
    if (element === null) return
    setClipped(element.scrollHeight > element.clientHeight + 1)
  }, [file, asChange])
  const name = file.path.replace(/\\/g, '/').split('/').pop() ?? file.path

  if (asChange) {
    return (
      <div className="lc-docpreview is-change">
        <DiffView file={file} truncated={truncated} reported={reported} />
        <div className="lc-docpreview__foot">
          <button type="button" className="lc-docpreview__switch" onClick={() => setAsChange(false)}>
            Show it as a document
          </button>
        </div>
      </div>
    )
  }
  return (
    <div className="lc-docpreview" role="region" aria-label={`${name}, as written`}>
      {isPageFile(file.path) ? (
        <PagePreview path={file.path} name={name} />
      ) : isTableFile(file.path) ? (
        <div className="lc-docpreview__table">
          <SheetView workbook={csvWorkbook(name, documentTextOf(file), extensionOf(file.path) === 'tsv' ? '\t' : ',')} compact />
        </div>
      ) : (
        <div ref={page} className={`lc-docpreview__page${clipped ? ' is-clipped' : ''}`}>
          <AgentText text={documentTextOf(file)} streaming={false} />
        </div>
      )}
      <div className="lc-docpreview__foot">
        {onOpen !== undefined && (
          <button type="button" className="lc-docpreview__open" onClick={onOpen}>
            {/* The document glyph the handed-file pill wears: `maximize` is a
                bare square beside a word, and read as a checkbox. */}
            <Icon name="file" size={12} />
            Open {name}
          </button>
        )}
        {(clipped || truncated) && !isPageFile(file.path) && (
          <span className="lc-docpreview__note">{truncated ? 'Only its start was recorded here.' : 'The rest is in the file.'}</span>
        )}
        <button type="button" className="lc-docpreview__switch" onClick={() => setAsChange(true)}>
          Show the change
        </button>
      </div>
    </div>
  )
}
