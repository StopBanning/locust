import { useEffect, useState } from 'react'
import type { ReactElement } from 'react'
import { REPLY_PAGE_CHANNEL } from '../../../shared/ipc.js'
import { onTheComputer } from '../onTheComputer.js'

/**
 * A WHOLE WEB PAGE IN A REPLY, RUNNING (0.553, shared/reply-page.ts).
 *
 * Colin, 2026-10-02, over three arcade games that arrived as code blocks:
 * "we got to find a way to have these display and work properly". The page
 * runs on a framed stage, as a page a teammate wrote to disk does in its
 * card (DocPreview's PagePreview): served at its own address, with its own
 * origin, sandboxed the same way. Its code is one tab away, unchanged.
 */
export function ReplyPage({ code, language }: { readonly code: string; readonly language?: string }): ReactElement {
  const [tab, setTab] = useState<'page' | 'code'>('page')
  const [url, setUrl] = useState<string>()
  const [refused, setRefused] = useState<string>()
  // A new run of the same page starts it again (a game over, a stuck state).
  const [run, setRun] = useState(0)
  useEffect(() => {
    let live = true
    setUrl(undefined)
    setRefused(undefined)
    const bridge = window.desktop
    if (bridge?.replyPageUrl === undefined) {
      setRefused('This page cannot run here. Its code is in the Code tab.')
      return
    }
    // Served by the computer at an address only it reaches (0.850): a browser says where it runs, and keeps the code.
    if (onTheComputer(REPLY_PAGE_CHANNEL) !== undefined) {
      setRefused('This page runs in Locust on your computer. Its code is in the Code tab.')
      return
    }
    void bridge.replyPageUrl(code).then((answer) => {
      if (!live) return
      if (answer.ok) setUrl(answer.url)
      else setRefused(answer.message)
    }).catch(() => {
      if (live) setRefused('This page could not be opened. Its code is in the Code tab.')
    })
    return () => {
      live = false
    }
  }, [code])
  return (
    <figure className="lc-replypage">
      <figcaption className="lc-replypage__bar">
        <span className="lc-replypage__tabs" role="tablist" aria-label="Show the page or its code">
          <button type="button" role="tab" aria-selected={tab === 'page'} className={`lc-replypage__tab${tab === 'page' ? ' is-on' : ''}`} onClick={() => setTab('page')}>
            Page
          </button>
          <button type="button" role="tab" aria-selected={tab === 'code'} className={`lc-replypage__tab${tab === 'code' ? ' is-on' : ''}`} onClick={() => setTab('code')}>
            Code
          </button>
        </span>
        {tab === 'page' && url !== undefined && <span className="lc-replypage__live">Live · try it</span>}
        {tab === 'page' && url !== undefined && (
          <button type="button" className="lc-replypage__action" onClick={() => setRun((count) => count + 1)} title="Start the page again">
            Reload
          </button>
        )}
      </figcaption>
      {tab === 'code' ? (
        <pre className="lc-code lc-replypage__code">
          {language !== undefined && <span className="lc-code__lang lc-mono">{language}</span>}
          <code>{code}</code>
        </pre>
      ) : url === undefined ? (
        <p className="lc-replypage__wait">{refused ?? 'Opening the page…'}</p>
      ) : (
        <iframe
          key={run}
          className="lc-replypage__frame"
          src={url}
          title="The page in this reply, running"
          sandbox="allow-scripts allow-same-origin allow-forms allow-modals allow-popups allow-downloads"
        />
      )}
    </figure>
  )
}
