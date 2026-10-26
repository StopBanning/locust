import { useEffect, useRef, useState } from 'react'
import type { ReactElement } from 'react'

import type { FileTurn } from '../missionView.js'
import { ColouredCode } from './ColouredCode.js'
import { languageOfPath } from '../codeColors.js'
import { AgentText } from './ThreadItems.js'
import { DiffView } from './DiffView.js'
import { Icon } from './Icon.js'
import { fileMenuEvent } from '../fileMenuOpen.js'
import { SheetView } from './SheetView.js'
import { PdfPages } from './PdfPages.js'
import { DocumentView } from './DocumentView.js'
import { fileAsItWas } from '../fileVersions.js'
import { documentChanges } from '../documentChange.js'
import { DocumentChange } from './DocumentChange.js'
import { ZoomableImage } from './ZoomableImage.js'
import type { OfficeDocument } from '../../../shared/office-document.js'
import type { Workbook } from '../../../shared/sheet.js'
import { quoteOfPagePick, VIEWER_FRAME_NAME } from '../../../shared/page-pick.js'

/** The file's language for colouring its code (0.719), when Locust carries one. */
const languageProp = (path: string): { language?: string } => {
  const language = languageOfPath(path)
  return language === undefined ? {} : { language }
}

/**
 * A file a teammate wrote, open beside the conversation.
 *
 * Colin, 2026-09-20: *"is there a way like what claude code has where when you
 * click a file/md it opens it over here near where our activity would be if
 * opened? would be a good QOL update."* Until now a teammate wrote you a
 * report and the app's best offer was a file manager.
 *
 * It shares the inspector's region rather than inventing one. That region
 * already knows how to sit beside the thread without covering the composer --
 * which cost a release to get right (0.187.0, at 1120x720) -- and a second
 * drawer would be a second answer to "what is this panel".
 *
 * IT RENDERS AND IT NEVER RUNS. Markdown goes through `AgentText`, the same
 * renderer every reply uses: React escapes every value, so nothing in the
 * file can become markup, and links go through the host's own outbound rules
 * rather than the browser's. Anything else is monospace in its own scroll box.
 * A file in this workspace was written by a MODEL, which is the same threat
 * model as a reply and the reason `shell.openPath` is refused in
 * `reveal-file.ts`. Do not add an "open externally" control here.
 */
export function FileViewer({
  path,
  text,
  mode,
  turns = [],
  onClose,
  onReveal,
  onSave,
  workbook,
  document: officeDocument,
  pageUrl,
  onPointAt,
  pdf
}: {
  readonly path: string
  /** The file's text, or a `data:` URL when the mode is `image`. */
  readonly text: string
  readonly mode: 'markdown' | 'code' | 'image' | 'table' | 'document' | 'pdf'
  /** A PDF's page pictures, folder-relative, when the mode is `pdf` (0.714, PdfPages). */
  readonly pdf?: { readonly pages: number; readonly pictures: readonly string[]; readonly folder: string }
  /** A spreadsheet's cells, read by the host, when the mode is `table`. */
  readonly workbook?: Workbook
  /** A Word or PowerPoint file's words, read by the host, when the mode is `document` (0.517). */
  readonly document?: OfficeDocument
  /**
   * The turns in this conversation that changed this file, oldest first.
   *
   * Empty for a file nobody here touched -- a project file a teammate only
   * read, say -- and the strip is then absent rather than present and empty.
   */
  readonly turns?: readonly FileTurn[]
  readonly onClose: () => void
  /** Show it in the file manager -- the thing the pill did before this existed. */
  readonly onReveal: () => void
  readonly onSave: () => void
  /**
   * A web page's address in the preview (0.425): shown WORKING, in a frame
   * with its own origin (main/page-preview.ts), its source one tab away.
   */
  readonly pageUrl?: string
  /**
   * A part of the running page, pointed at (0.484): its quote for the chat
   * box and a picture of it. Absent where there is no chat box to take it.
   */
  readonly onPointAt?: (pick: { readonly quote: string; readonly png?: Uint8Array }) => void
}): ReactElement {
  /** Which turn's change is being read, or undefined for the file as it is. */
  const [showing, setShowing] = useState<number>()
  /** An earlier turn's version read as its change rather than as the file it was (0.517). */
  const [asChange, setAsChange] = useState(false)
  // A document's change reads as its words (0.530); the lines are a click away.
  const [asLines, setAsLines] = useState(false)
  const [asSource, setAsSource] = useState(false)
  const [reloads, setReloads] = useState(0)
  const [pointing, setPointing] = useState<'idle' | 'pointing' | { readonly said: string }>('idle')
  const frame = useRef<HTMLIFrameElement>(null)
  const running = pageUrl !== undefined && !asSource
  const fileName = path.replace(/\\/g, '/').split('/').pop() ?? path
  // A pick left open when the viewer goes (another file, closed) is let go.
  useEffect(() => {
    if (pointing !== 'pointing' || pageUrl === undefined) return
    return () => {
      void window.desktop?.cancelPagePick(pageUrl).catch(() => undefined)
    }
  }, [pointing, pageUrl])
  const point = (): void => {
    const bridge = window.desktop
    const box = frame.current?.getBoundingClientRect()
    if (bridge === undefined || pageUrl === undefined || box === undefined || onPointAt === undefined) return
    if (pointing === 'pointing') {
      void bridge.cancelPagePick(pageUrl).catch(() => undefined)
      return
    }
    setPointing('pointing')
    frame.current?.focus()
    void bridge
      .pickInPage({ pageUrl, frame: { x: box.left, y: box.top, width: box.width, height: box.height } })
      .then((answer) => {
        if (!answer.ok) {
          setPointing({ said: answer.message })
          return
        }
        setPointing('idle')
        if (answer.picked !== undefined) onPointAt({ quote: quoteOfPagePick(fileName, answer.picked), ...(answer.png === undefined ? {} : { png: answer.png }) })
      })
      .catch(() => setPointing({ said: 'The page could not be pointed at. Reload it and try again.' }))
  }
  const version = showing === undefined ? undefined : turns[showing]
  // The file as it was after that turn, when it can be rebuilt exactly (fileVersions.ts): text files only.
  const rebuilt = version === undefined || showing === undefined || (mode !== 'markdown' && mode !== 'code') ? undefined : fileAsItWas(text, turns, showing)
  const turnName = `turn ${String((showing ?? 0) + 1)}`
  return (
    <aside className="lc-viewer" aria-label={`Viewing ${path}`}>
      <div className="lc-viewer__head">
        <Icon name="file" size={13} />
        {/*
          * The full path in the title, the name in the tab. A viewer that
          * says only `report.md` cannot tell you WHICH report.md, and the
          * pane is too narrow for the path at any useful size.
          */}
        <span className="lc-viewer__name" title={path} onContextMenu={fileMenuEvent(path)}>
          {path.replace(/\\/g, '/').split('/').pop() ?? path}
        </span>
        <span className="lc-viewer__spacer" />
        {pageUrl !== undefined && (
          <>
            <div className="lc-segmented lc-viewer__pagetabs" role="radiogroup" aria-label="Show the page or its source">
              <button type="button" role="radio" aria-checked={!asSource} className={`lc-button${asSource ? '' : ' is-active'}`} onClick={() => setAsSource(false)}>
                Page
              </button>
              <button type="button" role="radio" aria-checked={asSource} className={`lc-button${asSource ? ' is-active' : ''}`} onClick={() => setAsSource(true)}>
                Source
              </button>
            </div>
            {running && onPointAt !== undefined && (
              <button
                type="button"
                className={`lc-viewer__action${pointing === 'pointing' ? ' is-active' : ''}`}
                title={pointing === 'pointing' ? 'Stop pointing' : 'Point at a part of the page to ask about it'}
                aria-label={pointing === 'pointing' ? 'Stop pointing' : 'Point at a part of the page to ask about it'}
                aria-pressed={pointing === 'pointing'}
                onClick={point}
              >
                <Icon name="target" size={13} />
              </button>
            )}
            {running && (
              <button type="button" className="lc-viewer__action" title="Reload the page" aria-label="Reload the page" onClick={() => setReloads((count) => count + 1)}>
                <Icon name="refresh" size={13} />
              </button>
            )}
          </>
        )}
        <button type="button" className="lc-viewer__action" title="Show it in the file manager" onClick={onReveal}>
          <Icon name="folder" size={13} />
        </button>
        <button type="button" className="lc-viewer__action" title="Save a copy…" onClick={onSave}>
          <Icon name="download" size={13} />
        </button>
        <button type="button" className="lc-viewer__close" aria-label="Close the file" onClick={onClose}>
          <Icon name="close" size={13} />
        </button>
      </div>
      {/*
        * WHAT THIS FILE HAS BEEN THROUGH, when this conversation put it there.
        *
        * Colin asked for artifacts; this is the half of the word that is a
        * READING feature over what the ledger already keeps. Each chip is a
        * turn that changed this file, oldest on the left, and pressing one
        * shows the change that turn made.
        *
        * Pressing one shows the file AS IT WAS after that turn (0.517) --
        * but only when the later turns' recorded changes undo exactly
        * (fileVersions.ts). Patches arrive truncated, and some runtimes
        * report an edit with no diff at all; then the chip shows the change
        * that turn made and says why the version itself cannot be shown.
        */}
      {turns.length > 0 && (
        <div className="lc-viewer__versions">
          <span className="lc-viewer__versionlabel lc-mono">
            CHANGED IN {turns.length} {turns.length === 1 ? 'TURN' : 'TURNS'}
          </span>
          <span className="lc-viewer__spacer" />
          {turns.map((turn, index) => (
            <button
              key={`${turn.missionId}_${String(index)}`}
              type="button"
              className={`lc-viewer__version${showing === index ? ' is-showing' : ''}`}
              /* The ask, so a version has a reason on it and not just a number. */
              title={turn.prompt}
              aria-pressed={showing === index}
              onClick={() => {
                setAsChange(false)
                setShowing(showing === index ? undefined : index)
              }}
            >
              {index + 1}
            </button>
          ))}
          <button
            type="button"
            className={`lc-viewer__version is-now${showing === undefined ? ' is-showing' : ''}`}
            title="The file as it is on disk now"
            aria-pressed={showing === undefined}
            onClick={() => setShowing(undefined)}
          >
            Now
          </button>
        </div>
      )}
      {version !== undefined && rebuilt?.ok === true && !asChange ? (
        <div className="lc-viewer__scroll">
          <p className="lc-viewer__versionnote">
            {rebuilt.next === null ? `The file was not there after ${turnName}.` : `The file as it was after ${turnName}.`}{' '}
            <button type="button" className="lc-linkbutton" onClick={() => setAsChange(true)}>
              {`What ${turnName} changed`}
            </button>
          </p>
          {rebuilt.next === null ? null : mode === 'markdown' ? (
            <div className="lc-viewer__prose">
              <AgentText text={rebuilt.next} streaming={false} />
            </div>
          ) : (
            <pre className="lc-viewer__code"><ColouredCode code={rebuilt.next} {...languageProp(path)} /></pre>
          )}
        </div>
      ) : version !== undefined ? (
        <div className="lc-viewer__scroll">
          <p className="lc-viewer__versionnote">
            {`What ${turnName} changed. `}
            {rebuilt?.ok === true ? (
              <button type="button" className="lc-linkbutton" onClick={() => setAsChange(false)}>
                {`The file as it was after ${turnName}`}
              </button>
            ) : rebuilt !== undefined ? (
              `It cannot be shown as it was, because ${rebuilt.why}. `
            ) : null}
            {rebuilt?.ok === true ? null : <>The file itself is under <strong>Now</strong>.</>}
          </p>
          {mode === 'markdown' && !asLines ? (
            <>
              <DocumentChange passages={documentChanges(version.file)} />
              <p className="lc-viewer__versionnote">
                {version.truncated ? 'Part of this change was recorded cut short. ' : ''}
                <button type="button" className="lc-linkbutton" onClick={() => setAsLines(true)}>
                  Show the lines that changed
                </button>
              </p>
            </>
          ) : (
            <>
              {mode === 'markdown' && (
                <p className="lc-viewer__versionnote">
                  <button type="button" className="lc-linkbutton" onClick={() => setAsLines(false)}>
                    Show it as words
                  </button>
                </p>
              )}
              <DiffView key={showing} file={version.file} truncated={version.truncated} reported={version.reported} />
            </>
          )}
        </div>
      ) : running ? (
        /*
         * THE PAGE, RUNNING (0.425). Its own origin (locust-page://<token>),
         * never the app's, so allow-same-origin gives it its OWN storage and
         * nothing of Locust's; the host answers IPC from the top frame only,
         * and a sub-frame gets no preload. docs/DECISION-2026-09-28-PAGE-PREVIEW.md.
         */
        <div className="lc-viewer__pagewrap">
        {pointing === 'pointing' && (
          <p className="lc-viewer__pointhint" role="status">Click a part of the page to ask about it. Esc stops.</p>
        )}
        {typeof pointing === 'object' && (
          <p className="lc-viewer__pointhint is-said" role="status">{pointing.said}</p>
        )}
        <iframe
          ref={frame}
          name={VIEWER_FRAME_NAME}
          key={reloads}
          className="lc-viewer__page"
          src={pageUrl}
          title={`${path.replace(/\\/g, '/').split('/').pop() ?? 'page'}, running`}
          sandbox="allow-scripts allow-same-origin allow-forms allow-modals allow-popups allow-downloads"
        />
        </div>
      ) : (
      <div className="lc-viewer__scroll">
        {mode === 'image' ? (
          /*
           * A raster image, drawn as itself. `alt` is the file's own name and
           * nothing more: the model chose the name, so any description here
           * would be the model describing its own picture to a person who
           * cannot see it, with nothing checking the claim.
           *
           * SVG never reaches this branch -- `image-files.ts` leaves it out
           * on purpose, because an SVG is a document that can carry script.
           * It opens as code instead, which is the honest way to show one.
           */
          <ZoomableImage src={text} alt={path.replace(/\\/g, '/').split('/').pop() ?? 'image'} />
        ) : mode === 'pdf' && pdf !== undefined ? (
          // Its pages as Locust drew them for the agent (0.714): pictures, never the file run.
          <PdfPages name={fileName} pages={pdf.pages} pictures={pdf.pictures} folder={pdf.folder} />
        ) : mode === 'table' && workbook !== undefined ? (
          // Read, never run: cells as escaped text (shared/sheet.ts).
          <SheetView workbook={workbook} />
        ) : mode === 'document' && officeDocument !== undefined ? (
          // Read, never run: words as escaped text (shared/office-document.ts).
          <DocumentView document={officeDocument} />
        ) : mode === 'markdown' ? (
          <div className="lc-viewer__prose">
            <AgentText text={text} streaming={false} />
          </div>
        ) : (
          /*
           * Its own horizontal scroll, never the pane's. A long line in a log
           * must not make the whole drawer scroll sideways -- that is the
           * rule every wide thing in this app follows.
           */
          <pre className="lc-viewer__code"><ColouredCode code={text} {...languageProp(path)} /></pre>
        )}
      </div>
      )}
      {/*
        * THE REASON THE OPEN BUTTON IS MISSING, said out loud.
        *
        * Design agent's ruling, 2026-09-20: the three controls are the right
        * words, and "what is absent is the reason. A person who wants to open
        * a .md and cannot will read the missing control as an oversight, not
        * a decision -- and the decision is a good one that the panel is
        * currently keeping to itself."
        *
        * It is a standing fact about how the app works rather than a message
        * about this file, so it sits at the foot of the panel and never
        * changes. It also stops Reveal looking like the safe fallback for a
        * control somebody forgot.
        */}
      <p className="lc-viewer__register">
        {running
          ? 'This page runs here, in a frame of its own: it cannot reach Locust, your files or your accounts.'
          /*
            * Said as what it does (0.515). "Locust does not open files" sat
            * under the file Locust had just shown -- read as a contradiction
            * in a pass on 0.512. The point is that it never LAUNCHES one.
            */
          : `Shown here only: Locust never runs a file a teammate made, or opens it in another app. Reveal shows where it is in ${typeof navigator !== 'undefined' && /Macintosh|Mac OS X/.test(navigator.userAgent) ? 'the Finder' : 'Windows'}.`}
      </p>
    </aside>
  )
}
