import { useCallback, useEffect, useRef, useState } from 'react'
import {
  browserRuntimeUnsupportedNote,
  type BrowserPanelState,
  type BrowserProfile
} from '../../../shared/browser.js'
import { browserPointControl, BROWSER_POINT_SHORTCUT, isBrowserPointShortcut, type BrowserPoint } from '../../../shared/browser-point.js'
import type { MissionRuntimeId } from '@teammate/runtime-adapters'
import { Icon } from './Icon.js'

export default function BrowserPanel({
  visible,
  runtime,
  profiles = [],
  pointingOff,
  onPointed,
  onClose
}: {
  readonly visible: boolean
  readonly runtime?: MissionRuntimeId
  /** Teammates with their own sign-ins (0.839): the + offers a tab as each, to sign it in. */
  readonly profiles?: readonly BrowserProfile[]
  readonly pointingOff?: string
  readonly onPointed: (pick: { readonly point: BrowserPoint; readonly png?: Uint8Array }) => void | string | Promise<void | string | undefined>
  readonly onClose: () => void
}) {
  const [state, setState] = useState<BrowserPanelState>({ open: visible, tabs: [] })
  const [addressInput, setAddressInput] = useState('')
  const [editingAddress, setEditingAddress] = useState(false)
  const [localError, setLocalError] = useState<string>()
  const [pointing, setPointing] = useState<'idle' | 'pointing' | { readonly said: string }>('idle')
  const [newTabMenu, setNewTabMenu] = useState(false)
  // The menu closes on a press anywhere else, or Esc.
  useEffect(() => {
    if (!newTabMenu) return
    const away = (event: Event): void => {
      if (event instanceof KeyboardEvent ? event.key === 'Escape' : !(event.target instanceof Element && event.target.closest('.lc-browser__newtab, .lc-browser__newtabmenu'))) setNewTabMenu(false)
    }
    window.addEventListener('pointerdown', away, true)
    window.addEventListener('keydown', away, true)
    return () => {
      window.removeEventListener('pointerdown', away, true)
      window.removeEventListener('keydown', away, true)
    }
  }, [newTabMenu])
  const viewportRef = useRef<HTMLDivElement>(null)
  const pointRef = useRef<() => void>(() => undefined)

  const activeTab = state.tabs.find((t) => t.id === state.activeTabId) ?? state.tabs[0]
  const runtimeNote = runtime !== undefined ? browserRuntimeUnsupportedNote(runtime) : undefined

  useEffect(() => {
    if (!editingAddress) {
      setAddressInput(activeTab?.url ?? '')
    }
  }, [activeTab?.id, activeTab?.url, editingAddress])

  useEffect(() => {
    const api = window.desktop?.browser
    if (api === undefined) return
    const off = api.onState((next) => {
      setState(next)
    })
    void api
      .state()
      .then((initial) => {
        setState(initial)
      })
      .catch(() => undefined)
    return () => {
      off()
    }
  }, [])

  const reportBounds = useCallback(() => {
    const api = window.desktop?.browser
    if (api === undefined) return
    if (!visible || viewportRef.current === null) {
      void api.setBounds(null).catch(() => undefined)
      return
    }
    const rect = viewportRef.current.getBoundingClientRect()
    if (rect.width <= 0 || rect.height <= 0) {
      void api.setBounds(null).catch(() => undefined)
      return
    }
    void api
      .setBounds({
        x: Math.round(rect.left),
        y: Math.round(rect.top),
        width: Math.round(rect.width),
        height: Math.round(rect.height)
      })
      .catch(() => undefined)
  }, [visible])

  useEffect(() => {
    reportBounds()
    if (!visible || viewportRef.current === null) return
    const observer = new ResizeObserver(() => reportBounds())
    observer.observe(viewportRef.current)
    window.addEventListener('resize', reportBounds)
    const frame = requestAnimationFrame(() => reportBounds())
    return () => {
      cancelAnimationFrame(frame)
      observer.disconnect()
      window.removeEventListener('resize', reportBounds)
      void window.desktop?.browser?.setBounds(null).catch(() => undefined)
    }
  }, [visible, reportBounds, state.activity, state.error, localError, runtimeNote, state.tabs.length, newTabMenu])

  const point = useCallback((): void => {
    const api = window.desktop?.browser
    if (api === undefined || pointingOff !== undefined) return
    if (pointing === 'pointing') {
      void api.cancelPoint().catch(() => undefined)
      setPointing('idle')
      return
    }
    if (activeTab?.canPoint === false) return
    if (activeTab?.url === undefined || activeTab.url.length === 0) {
      setPointing({ said: 'There is no page to point at. Open one in the browser first.' })
      return
    }
    setPointing('pointing')
    void api
      .point()
      .then(async (answer) => {
        if (!answer.ok) {
          setPointing(answer.blocked === true ? 'idle' : { said: answer.message })
          return
        }
        setPointing('idle')
        if (answer.picked === undefined) return
        const said = await onPointed({ point: answer.picked, ...(answer.png === undefined ? {} : { png: answer.png }) })
        if (typeof said === 'string' && said.length > 0) setPointing({ said })
      })
      .catch(() => setPointing({ said: 'The page could not be pointed at. It stayed as it was.' }))
  }, [activeTab?.canPoint, activeTab?.url, onPointed, pointing, pointingOff])
  pointRef.current = point

  useEffect(() => {
    if (visible || pointing !== 'pointing') return
    void window.desktop?.browser?.cancelPoint().catch(() => undefined)
    setPointing('idle')
  }, [visible, pointing])

  useEffect(() => {
    return () => {
      void window.desktop?.browser?.cancelPoint().catch(() => undefined)
    }
  }, [])

  useEffect(() => {
    if (!visible) return
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && pointing === 'pointing') {
        event.preventDefault()
        pointRef.current()
        return
      }
      if (!isBrowserPointShortcut(event) || event.repeat) return
      event.preventDefault()
      pointRef.current()
    }
    window.addEventListener('keydown', onKey, true)
    const off = window.desktop?.browser?.onPointHotkey(() => pointRef.current())
    return () => {
      window.removeEventListener('keydown', onKey, true)
      off?.()
    }
  }, [visible, pointing])

  const submitAddress = async (event: React.FormEvent) => {
    event.preventDefault()
    const api = window.desktop?.browser
    if (api === undefined) return
    const trimmed = addressInput.trim()
    if (!trimmed) return
    setLocalError(undefined)
    setEditingAddress(false)
    try {
      const res = await api.navigate(trimmed, activeTab?.id)
      setState(res.state)
      if (!res.ok && res.message) {
        setLocalError(res.message)
      }
    } catch {
      setLocalError('The address did not open. The current page stayed open.')
    }
  }

  const errorText = localError ?? state.error
  const pointControl = browserPointControl({
    ...(pointingOff === undefined ? {} : { refusal: pointingOff }),
    blocked: activeTab?.canPoint === false,
    pointing: pointing === 'pointing',
    hasPage: activeTab?.url !== undefined && activeTab.url.length > 0
  })

  return (
    <aside className="lc-viewer lc-browser" aria-label="Browser panel" hidden={!visible}>
      <div className="lc-viewer__head">
        <span className="lc-viewer__name">Browser</span>
        {activeTab?.profile === undefined ? (
          <span
            className="lc-browser__partition"
            title="Isolated Chromium session (persist:locust-browser). Separate from your Chrome profile and cookies."
          >
            Isolated session
          </span>
        ) : (
          <span
            className="lc-browser__partition is-own"
            style={{ '--lc-tab-hue': `var(--lc-hue-${activeTab.profile.hue})` } as React.CSSProperties}
            title={`${activeTab.profile.name}'s own sign-ins, apart from yours and every other teammate's.`}
          >
            {`${activeTab.profile.name}'s sign-ins`}
          </span>
        )}
        <span className="lc-viewer__spacer" />
        <span className="lc-browser__newtab">
          <button
            className="lc-viewer__action"
            type="button"
            aria-label="New browser tab"
            title="New tab"
            aria-haspopup={profiles.length > 0 ? 'menu' : undefined}
            aria-expanded={profiles.length > 0 ? newTabMenu : undefined}
            onClick={() => {
              setLocalError(undefined)
              // With a teammate that signs in on its own (0.839), + asks whose tab it is.
              if (profiles.length > 0) {
                setNewTabMenu(!newTabMenu)
                return
              }
              void window.desktop?.browser
                ?.newTab()
                .then((next) => setState(next))
                .catch(() => undefined)
            }}
          >
            <Icon name="plus" size={13} />
          </button>
        </span>
        <button
          className="lc-viewer__close"
          type="button"
          aria-label="Hide browser"
          title="Hide browser (tabs stay open)"
          onClick={onClose}
        >
          <Icon name="close" size={13} />
        </button>
      </div>

      {/*
        * Whose tab, a row of its own rather than a menu over the page: the page is a native view drawn above the
        * window's HTML, so anything laid over it is hidden. The row pushes the page down instead.
        */}
      {newTabMenu && profiles.length > 0 && (
        <div className="lc-browser__newtabmenu" role="menu" aria-label="New tab">
          {[undefined, ...profiles].map((profile) => (
            <button
              key={profile?.id ?? 'shared'}
              type="button"
              role="menuitem"
              onClick={() => {
                setNewTabMenu(false)
                void window.desktop?.browser
                  ?.newTab(undefined, profile?.id)
                  .then((next) => setState(next))
                  .catch(() => undefined)
              }}
            >
              {profile === undefined ? (
                'New tab'
              ) : (
                <>
                  <span className="lc-browser__hue" style={{ '--lc-tab-hue': `var(--lc-hue-${profile.hue})` } as React.CSSProperties} aria-hidden="true" />
                  {`As ${profile.name}`}
                </>
              )}
            </button>
          ))}
        </div>
      )}

      <div className="lc-browser__tabs" role="tablist" aria-label="Browser tabs">
        {state.tabs.map((tab) => (
          <div
            className={`lc-browser__tab${tab.profile === undefined ? '' : ' is-own'}`}
            key={tab.id}
            {...(tab.profile === undefined ? {} : { style: { '--lc-tab-hue': `var(--lc-hue-${tab.profile.hue})` } as React.CSSProperties })}
          >
            <button
              role="tab"
              type="button"
              aria-selected={activeTab?.id === tab.id}
              title={tab.profile === undefined ? tab.url || tab.title || 'New Tab' : `${tab.profile.name}'s sign-ins · ${tab.url || tab.title || 'New Tab'}`}
              onClick={() => {
                setLocalError(undefined)
                void window.desktop?.browser
                  ?.selectTab(tab.id)
                  .then((next) => setState(next))
                  .catch(() => undefined)
              }}
            >
              {tab.title || 'New Tab'}
            </button>
            <button
              type="button"
              aria-label={`Close ${tab.title || 'New Tab'} tab`}
              title="Close tab"
              onClick={() => {
                setLocalError(undefined)
                void window.desktop?.browser
                  ?.closeTab(tab.id)
                  .then((next) => setState(next))
                  .catch(() => undefined)
              }}
            >
              <Icon name="close" size={12} />
            </button>
          </div>
        ))}
      </div>

      <div className="lc-browser__bar">
        <button
          type="button"
          className="lc-viewer__action"
          aria-label="Back"
          title="Back"
          disabled={!activeTab?.canGoBack}
          onClick={() =>
            void window.desktop?.browser
              ?.back(activeTab?.id)
              .then((next) => setState(next))
              .catch(() => undefined)
          }
        >
          <Icon name="arrow-left" size={13} />
        </button>
        <button
          type="button"
          className="lc-viewer__action"
          aria-label="Forward"
          title="Forward"
          disabled={!activeTab?.canGoForward}
          onClick={() =>
            void window.desktop?.browser
              ?.forward(activeTab?.id)
              .then((next) => setState(next))
              .catch(() => undefined)
          }
        >
          <Icon name="arrow-right" size={13} />
        </button>
        <button
          type="button"
          className="lc-viewer__action"
          aria-label={activeTab?.loading ? 'Stop loading' : 'Reload'}
          title={activeTab?.loading ? 'Stop loading' : 'Reload'}
          onClick={() => {
            const api = window.desktop?.browser
            if (!api) return
            const p = activeTab?.loading ? api.stop(activeTab.id) : api.reload(activeTab?.id)
            void p.then((next) => setState(next)).catch(() => undefined)
          }}
        >
          <Icon name={activeTab?.loading ? 'close' : 'refresh'} size={13} />
        </button>
        <BrowserPointButton control={pointControl} onClick={point} />
        <form className="lc-browser__form" onSubmit={(e) => void submitAddress(e)}>
          <input
            type="text"
            className="lc-browser__address"
            aria-label="Browser address"
            placeholder="https://example.com, localhost:3000, or ./index.html"
            value={addressInput}
            onFocus={() => setEditingAddress(true)}
            onBlur={() => setEditingAddress(false)}
            onChange={(e) => setAddressInput(e.target.value)}
          />
          <button type="submit" className="lc-button" aria-label="Go">
            Go
          </button>
        </form>
      </div>

      {state.activity !== undefined && (
        <div className="lc-browser__activity" role="status" aria-live="polite">
          <span className="lc-dot is-pulsing lc-tone-live" />
          <span>
            {state.activity.label} · {state.activity.action}
            {state.activity.detail ? ` — ${state.activity.detail}` : ''}
          </span>
        </div>
      )}

      {runtimeNote !== undefined && <p className="lc-browser__note">{runtimeNote}</p>}

      {errorText !== undefined && (
        <p role="alert" className="lc-browser__error">
          {errorText}
        </p>
      )}

      {pointing === 'pointing' && (
        <p className="lc-browser__hint" role="status">Click a part of the page to ask about it. Esc stops.</p>
      )}
      {typeof pointing === 'object' && (
        <p className="lc-browser__hint is-said" role="status">{pointing.said}</p>
      )}

      <div className="lc-browser__viewport" ref={viewportRef}>
        {activeTab?.previewDataUrl !== undefined && (
          <img className="lc-browser__preview" src={activeTab.previewDataUrl} alt="" />
        )}
        {(!activeTab || !activeTab.url) && (
          <div className="lc-browser__empty">
            <p>
              Open any <code>https:</code>, <code>http:</code>, <code>localhost</code>, or workspace <code>file:</code>{' '}
              page above, or ask a teammate to use the browser.
            </p>
            <p>
              This browser runs in Locust’s own Chromium partition (<code>persist:locust-browser</code>), separate from
              your Chrome profile and cookies. Before signing in, paying, sending, submitting a form, downloading, or
              leaving for a new site, teammates ask on an approval card in the conversation.
            </p>
          </div>
        )}
      </div>
    </aside>
  )
}

/** The toolbar control. A page that will not run the picker says so on the button. */
export function BrowserPointButton({
  control,
  onClick
}: {
  readonly control: ReturnType<typeof browserPointControl>
  readonly onClick: () => void
}) {
  const label = control.sentence
    ? control.label
    : control.pressed
      ? 'Stop pointing'
      : control.label === 'Open a page to point at it.'
        ? 'Open a page to point at it.'
        : 'Point at a part of the page'
  return (
    <button
      type="button"
      className={`lc-browser__point${control.pressed ? ' is-active' : ''}${control.sentence ? ' is-sentence' : ''}`}
      aria-label={control.sentence ? control.label : control.pressed ? 'Stop pointing' : control.label === 'Open a page to point at it.' ? 'Open a page to point at it.' : 'Point at a part of the page'}
      aria-pressed={control.pressed}
      aria-keyshortcuts="Control+Shift+C"
      title={control.disabled ? label : `${label} (${BROWSER_POINT_SHORTCUT})`}
      disabled={control.disabled}
      onClick={onClick}
    >
      {control.sentence ? label : <Icon name="target" size={13} />}
    </button>
  )
}
