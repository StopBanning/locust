import { randomBytes, randomUUID, timingSafeEqual } from 'node:crypto'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { createServer } from 'node:http'
import type { Server } from 'node:http'
import { tmpdir } from 'node:os'
import { basename, join } from 'node:path'

import { BrowserWindow, session, WebContentsView } from 'electron'
import type { Session, WebContents } from 'electron'

import {
  BROWSER_PARTITION,
  browserOriginForUrl,
  browserPartitionFor,
  classifyBrowserAction,
  formatBrowserPageSnapshot,
  isLoopbackOrWorkspaceOrigin,
  type BrowserActivity,
  type BrowserApprovalDecision,
  type BrowserConsoleEntry,
  type BrowserElementSnapshot,
  type BrowserNetworkEntry,
  type BrowserPageSnapshot,
  type BrowserPanelState,
  type BrowserProfile,
  type BrowserRect,
  type BrowserTabState
} from '../shared/browser.js'
import { resolveBrowserUrl } from './browser-url.js'
import { BROWSER_POINT_CANCEL, BROWSER_POINT_PROBE, pointInBrowser, type BrowserPointPage } from './browser-point.js'
import { beginBrowserPoint, type BrowserPointResponse } from '../shared/browser-point.js'

export interface BrowserMcpRegistration {
  readonly configPath: string
  readonly copilotConfigPath: string
  readonly command: string
  readonly args: readonly string[]
  readonly env: Readonly<Record<string, string>>
}

export interface BrowserRunContext {
  readonly runId: string
  readonly missionId: string
  readonly teammateLabel: string
  readonly cwd: string
  /** The teammate's own sign-ins, when it has them on (0.839): its tools reach only its own tabs. */
  readonly profile?: BrowserProfile
  readonly askApproval?: (decision: BrowserApprovalDecision) => Promise<{
    readonly approved: boolean
    readonly always?: boolean
    readonly reason?: string
  }>
}

export interface BrowserToolContentItem {
  readonly type: 'text' | 'image'
  readonly text?: string
  readonly data?: string
  readonly mimeType?: string
}

export interface BrowserToolResponse {
  readonly content: readonly BrowserToolContentItem[]
  readonly isError?: boolean
}

export interface BrowserHostOptions {
  readonly bridgePath: string
  readonly node: string
  readonly getWorkspaceRoot: () => string | undefined
  readonly policyRefusal?: () => string | undefined
  readonly emitState?: (state: BrowserPanelState) => void
  readonly onToolReceipt?: (receipt: {
    readonly missionId: string
    readonly runId: string
    readonly toolName: string
    readonly summary: string
  }) => void
  readonly emitPointHotkey?: () => void
  /** A teammate's own sign-ins, when it has them on; the person's "New tab as" asks for one. */
  readonly profileOf?: (teammateId: string) => Promise<BrowserProfile | undefined>
  readonly now?: () => Date
  readonly port?: number
}

export interface BrowserHost {
  start(): Promise<void>
  readonly port: number
  attachWindow(win: BrowserWindow): void
  getState(): BrowserPanelState
  openPanel(url?: string): Promise<BrowserPanelState>
  closePanel(): BrowserPanelState
  setBounds(bounds: BrowserRect | null): void
  navigate(rawUrl: string, tabId?: string): Promise<{ readonly ok: boolean; readonly message?: string; readonly state: BrowserPanelState }>
  goBack(tabId?: string): Promise<BrowserPanelState>
  goForward(tabId?: string): Promise<BrowserPanelState>
  reload(tabId?: string): Promise<BrowserPanelState>
  stop(tabId?: string): Promise<BrowserPanelState>
  newTab(rawUrl?: string, profileId?: string): Promise<BrowserPanelState>
  /** Close a teammate's own tabs and clear its partition: its sign-ins are gone (0.839). */
  forgetProfile(teammateId: string): Promise<BrowserPanelState>
  selectTab(tabId: string): BrowserPanelState
  closeTab(tabId: string): BrowserPanelState
  point(): Promise<BrowserPointResponse>
  cancelPoint(): Promise<void>
  registerRun(run: BrowserRunContext): Promise<BrowserMcpRegistration>
  releaseRun(runId: string): Promise<void>
  callTool(
    runId: string,
    toolName: string,
    args: Record<string, unknown>
  ): Promise<BrowserToolResponse>
  /** Count of live WebContentsView instances (0 before first open). */
  readonly viewCount: number
  dispose(): Promise<void>
}

interface InternalTab {
  readonly id: string
  readonly view: WebContentsView
  title: string
  url: string
  loading: boolean
  previewDataUrl?: string
  readonly personOrigins: Set<string>
  readonly consoleEntries: BrowserConsoleEntry[]
  readonly networkEntries: BrowserNetworkEntry[]
  lastElements: BrowserElementSnapshot[]
  canPoint?: boolean
  readonly profile?: BrowserProfile
}

interface RegisteredRun extends BrowserRunContext {
  readonly token: string
  readonly configDir: string
  readonly approvedGates: Set<string>
}

const MAX_LOG_ENTRIES = 100
const DEFAULT_OFFSCREEN_BOUNDS: BrowserRect = { x: 10000, y: 10000, width: 640, height: 480 }

/** Whether a run may use this tab: a person's press may use any; a run, only tabs in its own sign-ins (0.839). */
function ownTab(tab: Pick<InternalTab, 'profile'>, run: Pick<BrowserRunContext, 'profile'> | undefined): boolean {
  return run === undefined || tab.profile?.id === run.profile?.id
}

function textAnswer(text: string, isError = false): BrowserToolResponse {
  return {
    content: [{ type: 'text', text }],
    ...(isError ? { isError: true } : {})
  }
}

function pushBounded<T>(list: T[], entry: T, max = MAX_LOG_ENTRIES): void {
  list.push(entry)
  if (list.length > max) {
    list.splice(0, list.length - max)
  }
}

export function createBrowserMcpConfigJson(options: {
  readonly node: string
  readonly bridgePath: string
  readonly url: string
  readonly token: string
  readonly missionId: string
  readonly teammateLabel: string
  readonly forCopilot?: boolean
}): string {
  const env = {
    ELECTRON_RUN_AS_NODE: '1',
    LOCUST_BROWSER_URL: options.url,
    LOCUST_BROWSER_TOKEN: options.token,
    LOCUST_BROWSER_MISSION_ID: options.missionId,
    LOCUST_BROWSER_TEAMMATE: options.teammateLabel
  }
  return JSON.stringify(
    {
      mcpServers: {
        locust_browser: {
          ...(options.forCopilot ? { type: 'local', tools: ['*'] } : {}),
          command: options.node,
          args: [options.bridgePath],
          env
        }
      }
    },
    null,
    2
  )
}

const SNAPSHOT_SCRIPT = `(() => {
  const CREDENTIAL_RE = /\\b(password|passcode|passphrase|secret|api[-_\\s]?key|token|otp|2fa|two[-_\\s]?factor|verification\\s+code|auth\\s+code|pin\\b|cvv|cvc|csc|card\\s*number|cc[-_\\s]?number|cc[-_\\s]?exp|cc[-_\\s]?csc|credit\\s*card|debit\\s*card|routing\\s*number|bank\\s*account|iban|ssn|social\\s*security|tax\\s*id)\\b/i;
  const SELECTOR = 'a[href], button, input:not([type="hidden"]), textarea, select, summary, [role="button"], [role="link"], [role="checkbox"], [role="radio"], [role="tab"], [role="menuitem"], [role="searchbox"], [role="combobox"], [contenteditable="true"]';
  const nodes = Array.from(document.querySelectorAll(SELECTOR));
  const elements = [];
  let seq = 0;

  const isVisible = (el) => {
    const style = window.getComputedStyle(el);
    if (style.display === 'none' || style.visibility === 'hidden' || Number(style.opacity) === 0) return false;
    const rect = el.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0;
  };

  const labelFor = (el) => {
    const aria = el.getAttribute('aria-label');
    if (aria && aria.trim()) return aria.trim();
    const labelledBy = el.getAttribute('aria-labelledby');
    if (labelledBy) {
      const text = labelledBy
        .split(/\\s+/)
        .map((id) => document.getElementById(id)?.innerText ?? '')
        .join(' ')
        .trim();
      if (text) return text;
    }
    if (el.id) {
      const explicit = document.querySelector('label[for="' + CSS.escape(el.id) + '"]');
      if (explicit && explicit.innerText.trim()) return explicit.innerText.trim();
    }
    const wrappingLabel = el.closest('label');
    if (wrappingLabel && wrappingLabel.innerText.trim()) return wrappingLabel.innerText.trim();
    const tag = el.tagName.toLowerCase();
    if (tag === 'input' || tag === 'textarea' || tag === 'select') {
      const placeholder = el.getAttribute('placeholder');
      if (placeholder && placeholder.trim()) return placeholder.trim();
      const nameAttr = el.getAttribute('name') || el.getAttribute('id') || '';
      if (nameAttr.trim()) return nameAttr.trim();
    }
    const inner = (el.innerText || el.textContent || el.value || el.getAttribute('title') || el.getAttribute('alt') || '').replace(/\\s+/g, ' ').trim();
    return inner.slice(0, 160);
  };

  const roleFor = (el) => {
    const explicit = el.getAttribute('role');
    if (explicit) return explicit.toLowerCase();
    const tag = el.tagName.toLowerCase();
    if (tag === 'a') return 'link';
    if (tag === 'button' || tag === 'summary') return 'button';
    if (tag === 'select') return 'combobox';
    if (tag === 'textarea') return 'textbox';
    if (tag === 'input') {
      const t = (el.getAttribute('type') || 'text').toLowerCase();
      if (t === 'submit' || t === 'button' || t === 'reset') return 'button';
      if (t === 'checkbox') return 'checkbox';
      if (t === 'radio') return 'radio';
      if (t === 'search') return 'searchbox';
      return 'textbox';
    }
    return tag;
  };

  for (const el of nodes) {
    if (!isVisible(el)) continue;
    seq += 1;
    const ref = '@e' + String(seq);
    el.setAttribute('data-locust-ref', ref);
    const tag = el.tagName.toLowerCase();
    const type = (el.getAttribute('type') || (tag === 'input' ? 'text' : '')).toLowerCase() || undefined;
    const role = roleFor(el);
    const name = labelFor(el);
    const placeholder = (el.getAttribute('placeholder') || '').trim() || undefined;
    const autocomplete = (el.getAttribute('autocomplete') || '').trim();
    const rawDescriptor = [name, placeholder, type, autocomplete, el.getAttribute('name') || '', el.getAttribute('id') || ''].join(' ');
    const sensitive = type === 'password' || CREDENTIAL_RE.test(rawDescriptor);
    const form = el.closest('form');
    const isSubmitButton =
      type === 'submit' ||
      (tag === 'button' && (!type || type === 'submit') && Boolean(form)) ||
      (tag === 'input' && Boolean(form) && type !== 'button' && type !== 'checkbox' && type !== 'radio');
    const href = tag === 'a' ? el.href || el.getAttribute('href') || undefined : undefined;
    const download = tag === 'a' && el.hasAttribute('download');
    const rawValue = 'value' in el && typeof el.value === 'string' ? el.value.slice(0, 200) : undefined;
    elements.push({
      ref,
      role,
      name,
      tag,
      ...(type ? { type } : {}),
      ...(!sensitive && rawValue ? { value: rawValue } : {}),
      ...(placeholder ? { placeholder } : {}),
      ...(href ? { href } : {}),
      ...(el.disabled ? { disabled: true } : {}),
      ...(typeof el.checked === 'boolean' && (role === 'checkbox' || role === 'radio') ? { checked: el.checked } : {}),
      ...(sensitive ? { sensitive: true } : {}),
      ...(isSubmitButton ? { submit: true } : {}),
      ...(download ? { download: true } : {}),
      ...(form?.action ? { formAction: form.action } : {})
    });
    if (elements.length >= 120) break;
  }

  const bodyText = (document.body?.innerText || '').replace(/\\n{3,}/g, '\\n\\n').trim().slice(0, 12000);
  return {
    title: document.title || '',
    url: window.location.href || '',
    text: bodyText,
    elements
  };
})()`

export function createBrowserHost(options: BrowserHostOptions): BrowserHost {
  const now = options.now ?? (() => new Date())
  let mainWindow: BrowserWindow | undefined
  // One per partition: the shared one, and each teammate's own (0.839).
  const sessions = new Map<string, Session>()
  let server: Server | undefined
  let starting: Promise<void> | undefined
  let port = 0
  let panelOpen = false
  let currentBounds: BrowserRect | null = null
  let activeTabId: string | undefined
  let currentActivity: BrowserActivity | undefined
  let lastError: string | undefined
  let tabSeq = 0
  let activeToolRun: RegisteredRun | undefined
  let pointing = false
  let pointGeneration = 0

  const pageOf = (wc: WebContents): BrowserPointPage => ({
    executeJavaScript: (code, userGesture) => wc.executeJavaScript(code, userGesture),
    capturePage: (rect) => wc.capturePage(rect),
    getURL: () => wc.getURL(),
    isDestroyed: () => wc.isDestroyed(),
    getZoomFactor: () => wc.getZoomFactor()
  })

  const probePointing = (tab: InternalTab): void => {
    const wc = tab.view.webContents
    if (wc.isDestroyed() || tab.url.length === 0 || tab.url === 'about:blank') {
      tab.canPoint = undefined
      return
    }
    void wc.executeJavaScript(BROWSER_POINT_PROBE, true).then(
      (value) => {
        tab.canPoint = value === true
        notifyState()
      },
      () => {
        tab.canPoint = false
        notifyState()
      }
    )
  }

  const tabs = new Map<string, InternalTab>()
  const runsByToken = new Map<string, RegisteredRun>()
  const tokensByRunId = new Map<string, string>()

  const toTabState = (tab: InternalTab): BrowserTabState => {
    const wc = tab.view.webContents
    const destroyed = wc.isDestroyed()
    return {
      id: tab.id,
      title: tab.title || tab.url || 'New Tab',
      url: tab.url,
      loading: tab.loading,
      canGoBack: !destroyed && wc.navigationHistory.canGoBack(),
      canGoForward: !destroyed && wc.navigationHistory.canGoForward(),
      ...(tab.previewDataUrl !== undefined ? { previewDataUrl: tab.previewDataUrl } : {}),
      ...(tab.canPoint !== undefined ? { canPoint: tab.canPoint } : {}),
      ...(tab.profile !== undefined ? { profile: tab.profile } : {})
    }
  }

  const refreshTabPreview = async (tab: InternalTab): Promise<void> => {
    const wc = tab.view.webContents
    if (wc.isDestroyed() || !tab.url) return
    try {
      await new Promise((r) => setTimeout(r, 80))
      if (wc.isDestroyed()) return
      const image = await wc.capturePage()
      if (!image.isEmpty()) {
        tab.previewDataUrl = image.toDataURL()
        notifyState()
      }
    } catch {
      // Ignore capture errors when offscreen or closing
    }
  }

  const buildState = (): BrowserPanelState => ({
    open: panelOpen,
    tabs: [...tabs.values()].map(toTabState),
    ...(activeTabId !== undefined ? { activeTabId } : {}),
    ...(currentActivity !== undefined ? { activity: currentActivity } : {}),
    ...(lastError !== undefined ? { error: lastError } : {})
  })

  const notifyState = (): BrowserPanelState => {
    const state = buildState()
    options.emitState?.(state)
    return state
  }

  const syncViewAttachment = (): void => {
    if (!mainWindow || mainWindow.isDestroyed()) return
    const contentView = mainWindow.contentView
    for (const tab of tabs.values()) {
      const isActiveVisible =
        panelOpen &&
        tab.id === activeTabId &&
        tab.url.length > 0 &&
        currentBounds !== null &&
        currentBounds.width > 0 &&
        currentBounds.height > 0
      const attached = contentView.children.includes(tab.view)
      if (!attached) {
        contentView.addChildView(tab.view)
      }
      if (isActiveVisible && currentBounds) {
        tab.view.setBounds({
          x: Math.round(currentBounds.x),
          y: Math.round(currentBounds.y),
          width: Math.max(1, Math.round(currentBounds.width)),
          height: Math.max(1, Math.round(currentBounds.height))
        })
        tab.view.setVisible(true)
      } else {
        tab.view.setBounds(DEFAULT_OFFSCREEN_BOUNDS)
      }
    }
  }

  const ensureSession = (partition: string = BROWSER_PARTITION): Session => {
    const known = sessions.get(partition)
    if (known !== undefined) return known
    const part = session.fromPartition(partition)
    part.setSpellCheckerDictionaryDownloadURL('http://127.0.0.1:0/')
    part.setPermissionRequestHandler((_wc, _permission, callback) => callback(false))
    part.setPermissionCheckHandler(() => false)

    part.webRequest.onCompleted((details) => {
      for (const tab of tabs.values()) {
        if (!tab.view.webContents.isDestroyed() && tab.view.webContents.id === details.webContentsId) {
          pushBounded(tab.networkEntries, {
            method: details.method,
            url: details.url,
            status: details.statusCode,
            resourceType: details.resourceType,
            ok: details.statusCode >= 200 && details.statusCode < 400,
            at: now().toISOString()
          })
          break
        }
      }
    })

    part.webRequest.onErrorOccurred((details) => {
      for (const tab of tabs.values()) {
        if (!tab.view.webContents.isDestroyed() && tab.view.webContents.id === details.webContentsId) {
          pushBounded(tab.networkEntries, {
            method: details.method,
            url: details.url,
            resourceType: details.resourceType,
            ok: false,
            error: details.error,
            at: now().toISOString()
          })
          break
        }
      }
    })

    part.on('will-download', (event, item, webContents) => {
      const filename = basename(item.getFilename() || 'download')
      const sourceUrl = item.getURL() || webContents.getURL()
      const run = activeToolRun
      if (run?.askApproval) {
        item.pause()
        const decision = classifyBrowserAction({
          action: 'download',
          currentUrl: sourceUrl,
          filename
        })!
        const gateKey = `${decision.gate}:${filename}`
        const proceed = async (): Promise<void> => {
          let approved = run.approvedGates.has(gateKey)
          if (!approved) {
            const answer = await run.askApproval!(decision)
            approved = answer.approved
            if (answer.approved && answer.always) {
              run.approvedGates.add(gateKey)
            }
          }
          if (!approved) {
            item.cancel()
            lastError = `Download of ${filename} was declined. Nothing was saved to disk.`
            notifyState()
            return
          }
          const baseDir = options.getWorkspaceRoot() ?? tmpdir()
          const targetDir = join(baseDir, '.locust', 'downloads')
          await mkdir(targetDir, { recursive: true }).catch(() => undefined)
          item.setSavePath(join(targetDir, filename))
          item.resume()
        }
        void proceed().catch(() => {
          item.cancel()
        })
        return
      }
      // Person-initiated download inside the browser panel: save to .locust/downloads without blocking native modal.
      const baseDir = options.getWorkspaceRoot() ?? tmpdir()
      const targetDir = join(baseDir, '.locust', 'downloads')
      void mkdir(targetDir, { recursive: true }).catch(() => undefined)
      item.setSavePath(join(targetDir, filename))
      void event
    })

    sessions.set(partition, part)
    return part
  }

  const createTabInternal = (profile?: BrowserProfile): InternalTab => {
    const partition = browserPartitionFor(profile?.id)
    const part = ensureSession(partition)
    tabSeq += 1
    const id = `tab_${String(tabSeq)}`
    const view = new WebContentsView({
      webPreferences: {
        partition,
        session: part,
        contextIsolation: true,
        nodeIntegration: false,
        sandbox: true,
        spellcheck: false
      }
    })
    view.setBounds(DEFAULT_OFFSCREEN_BOUNDS)

    const tab: InternalTab = {
      id,
      view,
      title: 'New Tab',
      url: '',
      loading: false,
      personOrigins: new Set<string>(),
      consoleEntries: [],
      networkEntries: [],
      lastElements: [],
      ...(profile === undefined ? {} : { profile })
    }

    const wc: WebContents = view.webContents

    wc.setWindowOpenHandler(({ url }) => {
      const check = resolveBrowserUrl(url, { workspaceRoot: options.getWorkspaceRoot() })
      if (check.ok) {
        void navigateTab(tab, check.url, { initiatedByPerson: activeToolRun === undefined })
      } else {
        lastError = check.message
        notifyState()
      }
      return { action: 'deny' }
    })

    const guardNavigation = (event: Electron.Event, targetUrl: string): void => {
      if (targetUrl === 'about:blank') return
      const check = resolveBrowserUrl(targetUrl, { workspaceRoot: options.getWorkspaceRoot() })
      if (!check.ok) {
        event.preventDefault()
        lastError = check.message
        notifyState()
        return
      }
      if (activeToolRun !== undefined) {
        const decision = classifyBrowserAction({
          action: 'open',
          currentUrl: tab.url,
          targetUrl: check.url,
          personOrigins: tab.personOrigins
        })
        if (decision !== undefined) {
          const gateKey = `${decision.gate}:${check.origin}`
          if (!activeToolRun.approvedGates.has(gateKey)) {
            event.preventDefault()
            const run = activeToolRun
            if (run.askApproval) {
              void run.askApproval(decision).then((res) => {
                if (res.approved) {
                  tab.personOrigins.add(check.origin)
                  if (res.always) run.approvedGates.add(gateKey)
                  void wc.loadURL(check.url).catch(() => undefined)
                } else {
                  lastError = `Navigation to ${check.origin} was declined. The current page stayed open.`
                  notifyState()
                }
              })
            }
          }
        }
      }
    }

    wc.on('will-navigate', guardNavigation)
    wc.on('will-redirect', guardNavigation)

    wc.on('did-start-loading', () => {
      tab.loading = true
      notifyState()
    })

    wc.on('did-stop-loading', () => {
      tab.loading = false
      tab.url = wc.getURL() === 'about:blank' ? tab.url : wc.getURL()
      tab.title = wc.getTitle() || tab.title || tab.url || 'New Tab'
      syncViewAttachment()
      notifyState()
      void refreshTabPreview(tab)
      probePointing(tab)
    })

    wc.on('before-input-event', (event, input) => {
      if (input.type !== 'keyDown' || input.isAutoRepeat) return
      const chord = (input.control || input.meta) && input.shift && !input.alt && input.code === 'KeyC'
      if (!chord) return
      event.preventDefault()
      options.emitPointHotkey?.()
    })

    wc.on('page-title-updated', (_event, title) => {
      tab.title = title || tab.url || 'New Tab'
      notifyState()
    })

    wc.on('did-navigate', (_event, url) => {
      if (url !== 'about:blank') tab.url = url
      tab.title = wc.getTitle() || tab.title || tab.url || 'New Tab'
      syncViewAttachment()
      notifyState()
    })

    wc.on('did-navigate-in-page', (_event, url) => {
      if (url !== 'about:blank') tab.url = url
      notifyState()
    })

    wc.on('console-message', (event) => {
      const level = event.level === 'error' ? 'error' : event.level === 'warning' ? 'warning' : 'info'
      pushBounded(tab.consoleEntries, {
        level,
        message: String(event.message ?? '').slice(0, 2000),
        ...(event.sourceId ? { source: event.sourceId } : {}),
        ...(typeof event.lineNumber === 'number' ? { line: event.lineNumber } : {}),
        at: now().toISOString()
      })
    })

    tabs.set(id, tab)
    activeTabId = id
    syncViewAttachment()
    notifyState()
    return tab
  }

  // For a run, only a tab in its own sign-ins, and a new one in them when it has none (0.839).
  const ensureActiveTab = (requestedTabId?: string, run?: BrowserRunContext): InternalTab => {
    const requested = requestedTabId === undefined ? undefined : tabs.get(requestedTabId)
    if (requested !== undefined && ownTab(requested, run)) {
      activeTabId = requested.id
      syncViewAttachment()
      return requested
    }
    const active = activeTabId === undefined ? undefined : tabs.get(activeTabId)
    if (active !== undefined && ownTab(active, run)) {
      return active
    }
    const first = [...tabs.values()].find((tab) => ownTab(tab, run))
    if (first) {
      activeTabId = first.id
      syncViewAttachment()
      return first
    }
    return createTabInternal(run?.profile)
  }

  const navigateTab = async (
    tab: InternalTab,
    rawUrl: string,
    opts: { readonly initiatedByPerson: boolean }
  ): Promise<{ readonly ok: boolean; readonly message?: string }> => {
    const resolved = resolveBrowserUrl(rawUrl, { workspaceRoot: options.getWorkspaceRoot() })
    if (!resolved.ok) {
      lastError = resolved.message
      notifyState()
      return { ok: false, message: resolved.message }
    }
    lastError = undefined
    if (opts.initiatedByPerson || isLoopbackOrWorkspaceOrigin(resolved.origin)) {
      tab.personOrigins.add(resolved.origin)
    }
    tab.url = resolved.url
    syncViewAttachment()
    notifyState()
    try {
      await tab.view.webContents.loadURL(resolved.url)
      tab.url = tab.view.webContents.getURL() || resolved.url
      tab.title = tab.view.webContents.getTitle() || resolved.url
      syncViewAttachment()
      await refreshTabPreview(tab)
      notifyState()
      return { ok: true }
    } catch (err) {
      const message = err instanceof Error && err.message ? err.message : 'The page did not finish loading. The browser tab remained open.'
      lastError = message
      notifyState()
      return { ok: false, message }
    }
  }

  const captureSnapshot = async (tab: InternalTab): Promise<BrowserPageSnapshot> => {
    try {
      const raw = (await tab.view.webContents.executeJavaScript(SNAPSHOT_SCRIPT, true)) as {
        readonly title?: string
        readonly url?: string
        readonly text?: string
        readonly elements?: BrowserElementSnapshot[]
      }
      const elements = Array.isArray(raw?.elements) ? raw.elements : []
      tab.lastElements = elements
      if (raw?.title) tab.title = raw.title
      if (raw?.url && raw.url !== 'about:blank') tab.url = raw.url
      await refreshTabPreview(tab)
      return {
        tabId: tab.id,
        title: tab.title || '(Untitled)',
        url: tab.url || 'about:blank',
        text: typeof raw?.text === 'string' ? raw.text : '',
        elements
      }
    } catch {
      return {
        tabId: tab.id,
        title: tab.title || '(Untitled)',
        url: tab.url || 'about:blank',
        text: '',
        elements: tab.lastElements
      }
    }
  }

  const requireApprovalIfNeeded = async (
    run: RegisteredRun,
    decision: BrowserApprovalDecision | undefined,
    extraKey?: string
  ): Promise<{ readonly ok: true } | { readonly ok: false; readonly message: string }> => {
    if (decision === undefined) return { ok: true }
    const gateKey = `${decision.gate}:${extraKey ?? decision.summary}`
    if (run.approvedGates.has(gateKey)) return { ok: true }
    if (!run.askApproval) {
      return {
        ok: false,
        message: `Blocked (${decision.gate}): ${decision.summary}. This run cannot request interactive approval. Nothing was sent or changed.`
      }
    }
    const answer = await run.askApproval(decision)
    if (!answer.approved) {
      const why = answer.reason ? ` The person said: ${answer.reason}` : ''
      return {
        ok: false,
        message: `The person declined "${decision.summary}" in Locust.${why} Nothing was sent or changed.`
      }
    }
    if (answer.always) {
      run.approvedGates.add(gateKey)
    }
    return { ok: true }
  }

  const executeTool = async (
    run: RegisteredRun,
    toolName: string,
    args: Record<string, unknown>
  ): Promise<BrowserToolResponse> => {
    const refusal = options.policyRefusal?.()
    if (refusal !== undefined) {
      return textAnswer(refusal, true)
    }

    if (!panelOpen) {
      panelOpen = true
      syncViewAttachment()
      notifyState()
    }

    const withActivity = async <T>(actionLabel: string, detail: string | undefined, fn: () => Promise<T>): Promise<T> => {
      const prevRun = activeToolRun
      activeToolRun = run
      currentActivity = {
        label: run.teammateLabel || 'Teammate',
        action: actionLabel,
        ...(detail ? { detail } : {}),
        missionId: run.missionId
      }
      notifyState()
      try {
        return await fn()
      } finally {
        activeToolRun = prevRun
        currentActivity = undefined
        notifyState()
      }
    }

    const requestedTabId = typeof args.tab_id === 'string' && args.tab_id.trim() ? args.tab_id.trim() : undefined
    // Another teammate's own sign-ins, or the shared ones for a teammate with its own: never this run's to use (0.839).
    const named = requestedTabId === undefined ? undefined : tabs.get(requestedTabId)
    if (named !== undefined && !ownTab(named, run)) {
      return textAnswer(`Tab "${named.id}" is signed in as someone else, so it is not yours to use. Open a tab of your own with browser_tabs.`, true)
    }

    if (toolName === 'browser_tabs') {
      const action = typeof args.action === 'string' ? args.action : 'list'
      return withActivity('Managing tabs', action, async () => {
        if (action === 'new') {
          const tab = createTabInternal(run.profile)
          if (typeof args.url === 'string' && args.url.trim()) {
            const nav = await executeTool(run, 'browser_open', { url: args.url, tab_id: tab.id })
            return nav
          }
          return textAnswer(`Opened new tab ${tab.id}.`)
        }
        if (action === 'select' && requestedTabId) {
          if (!tabs.has(requestedTabId)) {
            return textAnswer(`Tab "${requestedTabId}" does not exist. Active tab stayed ${activeTabId ?? 'unchanged'}.`, true)
          }
          activeTabId = requestedTabId
          syncViewAttachment()
          notifyState()
          const snap = await captureSnapshot(tabs.get(requestedTabId)!)
          return textAnswer(formatBrowserPageSnapshot(snap))
        }
        if (action === 'close' && requestedTabId) {
          host.closeTab(requestedTabId)
          return textAnswer(`Closed tab ${requestedTabId}.`)
        }
        const list = [...tabs.values()].filter((t) => ownTab(t, run)).map(
          (t) => `${t.id === activeTabId ? '*' : '-'} ${t.id}: ${t.title || '(Untitled)'} (${t.url || 'about:blank'})`
        )
        return textAnswer(list.length > 0 ? `Open tabs:\n${list.join('\n')}` : 'No browser tabs are open yet.')
      })
    }

    if (toolName === 'browser_open') {
      const rawUrl = typeof args.url === 'string' ? args.url : ''
      const resolved = resolveBrowserUrl(rawUrl, { workspaceRoot: options.getWorkspaceRoot() })
      if (!resolved.ok) {
        return textAnswer(resolved.message, true)
      }
      const tab = requestedTabId === 'new' ? createTabInternal(run.profile) : ensureActiveTab(requestedTabId, run)
      return withActivity('Opening page', resolved.url, async () => {
        const decision = classifyBrowserAction({
          action: 'open',
          currentUrl: tab.url,
          targetUrl: resolved.url,
          personOrigins: tab.personOrigins
        })
        const gate = await requireApprovalIfNeeded(run, decision, resolved.origin)
        if (!gate.ok) return textAnswer(gate.message, true)
        tab.personOrigins.add(resolved.origin)
        const nav = await navigateTab(tab, resolved.url, { initiatedByPerson: false })
        if (!nav.ok) return textAnswer(nav.message ?? 'Navigation failed. The previous page stayed open.', true)
        options.onToolReceipt?.({
          missionId: run.missionId,
          runId: run.runId,
          toolName,
          summary: `Opened ${resolved.url}`
        })
        const snapshot = await captureSnapshot(tab)
        return textAnswer(formatBrowserPageSnapshot(snapshot))
      })
    }

    const tab = ensureActiveTab(requestedTabId, run)

    if (toolName === 'browser_read') {
      return withActivity('Reading page', tab.url || tab.title, async () => {
        const snapshot = await captureSnapshot(tab)
        options.onToolReceipt?.({
          missionId: run.missionId,
          runId: run.runId,
          toolName,
          summary: `Read ${snapshot.url}`
        })
        return textAnswer(formatBrowserPageSnapshot(snapshot))
      })
    }

    if (toolName === 'browser_click') {
      const rawRef = typeof args.ref === 'string' ? args.ref.trim() : ''
      const ref = rawRef ? (rawRef.startsWith('@') ? rawRef : `@${rawRef}`) : ''
      if (!ref) return textAnswer('Missing element ref (e.g. "@e1"). Nothing was clicked.', true)
      return withActivity('Clicking element', ref, async () => {
        if (tab.lastElements.length === 0) await captureSnapshot(tab)
        const el = tab.lastElements.find((item) => item.ref === ref || item.ref === rawRef)
        if (!el) {
          return textAnswer(`Element [${ref}] was not found on the current page. Call browser_read to refresh element refs.`, true)
        }
        const decision = classifyBrowserAction({
          action: 'click',
          currentUrl: tab.url,
          personOrigins: tab.personOrigins,
          element: el
        })
        const gate = await requireApprovalIfNeeded(run, decision, `${ref}:${el.name}`)
        if (!gate.ok) return textAnswer(gate.message, true)
        if (decision?.gate === 'leave-origin' && el.href) {
          try {
            const origin = browserOriginForUrl(new URL(el.href, tab.url).toString())
            if (origin) tab.personOrigins.add(origin)
          } catch {
            // ignore
          }
        }
        const clicked = (await tab.view.webContents.executeJavaScript(
          `(() => {
            const target = document.querySelector('[data-locust-ref=' + JSON.stringify(${JSON.stringify(ref)}) + ']');
            if (!target) return { ok: false };
            target.scrollIntoView({ block: 'center', inline: 'center' });
            if (typeof target.focus === 'function') target.focus();
            target.click();
            return { ok: true };
          })()`,
          true
        )) as { readonly ok?: boolean }
        if (!clicked?.ok) {
          return textAnswer(`Element [${ref}] is no longer attached to the page. Nothing was clicked.`, true)
        }
        await new Promise((r) => setTimeout(r, 80))
        options.onToolReceipt?.({
          missionId: run.missionId,
          runId: run.runId,
          toolName,
          summary: `Clicked [${ref}] ${el.name || el.role}`
        })
        const snapshot = await captureSnapshot(tab)
        return textAnswer(formatBrowserPageSnapshot(snapshot))
      })
    }

    if (toolName === 'browser_type') {
      const rawRef = typeof args.ref === 'string' ? args.ref.trim() : ''
      const ref = rawRef ? (rawRef.startsWith('@') ? rawRef : `@${rawRef}`) : ''
      const text = typeof args.text === 'string' ? args.text : ''
      const submit = args.submit === true
      if (!ref) return textAnswer('Missing element ref (e.g. "@e2"). Nothing was typed.', true)
      return withActivity('Typing into field', ref, async () => {
        if (tab.lastElements.length === 0) await captureSnapshot(tab)
        const el = tab.lastElements.find((item) => item.ref === ref || item.ref === rawRef)
        if (!el) {
          return textAnswer(`Element [${ref}] was not found on the current page. Call browser_read to refresh element refs.`, true)
        }
        const typeDecision = classifyBrowserAction({
          action: 'type',
          currentUrl: tab.url,
          element: el
        })
        const gate = await requireApprovalIfNeeded(run, typeDecision, `${ref}:${el.name}`)
        if (!gate.ok) return textAnswer(gate.message, true)

        if (submit) {
          const submitDecision = classifyBrowserAction({
            action: 'press',
            currentUrl: tab.url,
            element: { ...el, submit: true },
            key: 'Enter'
          })
          const submitGate = await requireApprovalIfNeeded(run, submitDecision, `submit:${ref}:${el.name}`)
          if (!submitGate.ok) return textAnswer(submitGate.message, true)
        }

        const typed = (await tab.view.webContents.executeJavaScript(
          `(() => {
            const target = document.querySelector('[data-locust-ref=' + JSON.stringify(${JSON.stringify(ref)}) + ']');
            if (!target) return { ok: false };
            target.scrollIntoView({ block: 'center', inline: 'center' });
            if (typeof target.focus === 'function') target.focus();
            const value = ${JSON.stringify(text)};
            if ('value' in target) {
              const proto = target.tagName === 'TEXTAREA' ? window.HTMLTextAreaElement.prototype : window.HTMLInputElement.prototype;
              const setter = Object.getOwnPropertyDescriptor(proto, 'value')?.set;
              if (setter) setter.call(target, value);
              else target.value = value;
            } else if (target.isContentEditable) {
              target.innerText = value;
            }
            target.dispatchEvent(new Event('input', { bubbles: true }));
            target.dispatchEvent(new Event('change', { bubbles: true }));
            if (${JSON.stringify(submit)}) {
              const enterDown = new KeyboardEvent('keydown', { key: 'Enter', code: 'Enter', keyCode: 13, which: 13, bubbles: true, cancelable: true });
              target.dispatchEvent(enterDown);
              const form = target.closest('form');
              if (form) {
                if (typeof form.requestSubmit === 'function') form.requestSubmit();
                else form.submit();
              }
            }
            return { ok: true };
          })()`,
          true
        )) as { readonly ok?: boolean }
        if (!typed?.ok) {
          return textAnswer(`Element [${ref}] is no longer attached to the page. Nothing was typed.`, true)
        }
        await new Promise((r) => setTimeout(r, 60))
        options.onToolReceipt?.({
          missionId: run.missionId,
          runId: run.runId,
          toolName,
          summary: `Typed into [${ref}] ${el.name || el.role}`
        })
        const snapshot = await captureSnapshot(tab)
        return textAnswer(formatBrowserPageSnapshot(snapshot))
      })
    }

    if (toolName === 'browser_press') {
      const key = typeof args.key === 'string' ? args.key.trim() : ''
      const rawRef = typeof args.ref === 'string' ? args.ref.trim() : undefined
      const ref = rawRef ? (rawRef.startsWith('@') ? rawRef : `@${rawRef}`) : undefined
      if (!key) return textAnswer('Missing key name. Nothing was pressed.', true)
      return withActivity('Pressing key', key, async () => {
        if (tab.lastElements.length === 0) await captureSnapshot(tab)
        const el = ref ? tab.lastElements.find((item) => item.ref === ref || item.ref === rawRef) : undefined
        const decision = classifyBrowserAction({
          action: 'press',
          currentUrl: tab.url,
          element: el,
          key
        })
        const gate = await requireApprovalIfNeeded(run, decision, `press:${key}:${ref ?? ''}`)
        if (!gate.ok) return textAnswer(gate.message, true)

        await tab.view.webContents.executeJavaScript(
          `(() => {
            const ref = ${JSON.stringify(ref ?? null)};
            const key = ${JSON.stringify(key)};
            const target = (ref ? document.querySelector('[data-locust-ref=' + JSON.stringify(ref) + ']') : document.activeElement) || document.body;
            if (target && typeof target.focus === 'function') target.focus();
            const down = new KeyboardEvent('keydown', { key, code: key, bubbles: true, cancelable: true });
            const up = new KeyboardEvent('keyup', { key, code: key, bubbles: true, cancelable: true });
            target.dispatchEvent(down);
            target.dispatchEvent(up);
            if ((key === 'Enter' || key === 'Return') && target && target.closest) {
              const form = target.closest('form');
              if (form) {
                if (typeof form.requestSubmit === 'function') form.requestSubmit();
                else form.submit();
              }
            }
          })()`,
          true
        )
        await new Promise((r) => setTimeout(r, 60))
        const snapshot = await captureSnapshot(tab)
        return textAnswer(formatBrowserPageSnapshot(snapshot))
      })
    }

    if (toolName === 'browser_select') {
      const rawRef = typeof args.ref === 'string' ? args.ref.trim() : ''
      const ref = rawRef ? (rawRef.startsWith('@') ? rawRef : `@${rawRef}`) : ''
      const value = typeof args.value === 'string' ? args.value : ''
      if (!ref) return textAnswer('Missing element ref. Nothing was selected.', true)
      return withActivity('Selecting option', `${ref} = ${value}`, async () => {
        if (tab.lastElements.length === 0) await captureSnapshot(tab)
        const el = tab.lastElements.find((item) => item.ref === ref || item.ref === rawRef)
        const decision = classifyBrowserAction({
          action: 'select',
          currentUrl: tab.url,
          element: el
        })
        const gate = await requireApprovalIfNeeded(run, decision, `select:${ref}`)
        if (!gate.ok) return textAnswer(gate.message, true)

        const selected = (await tab.view.webContents.executeJavaScript(
          `(() => {
            const target = document.querySelector('[data-locust-ref=' + JSON.stringify(${JSON.stringify(ref)}) + ']');
            if (!target || target.tagName !== 'SELECT') return { ok: false };
            const desired = ${JSON.stringify(value)};
            const option = Array.from(target.options).find((o) => o.value === desired || o.text.trim() === desired);
            if (!option) return { ok: false, missingOption: true };
            target.value = option.value;
            target.dispatchEvent(new Event('input', { bubbles: true }));
            target.dispatchEvent(new Event('change', { bubbles: true }));
            return { ok: true };
          })()`,
          true
        )) as { readonly ok?: boolean; readonly missingOption?: boolean }
        if (!selected?.ok) {
          return textAnswer(
            selected?.missingOption
              ? `Option "${value}" was not found in [${ref}]. Selection stayed unchanged.`
              : `Select element [${ref}] was not found on the page. Selection stayed unchanged.`,
            true
          )
        }
        const snapshot = await captureSnapshot(tab)
        return textAnswer(formatBrowserPageSnapshot(snapshot))
      })
    }

    if (toolName === 'browser_scroll') {
      const direction = typeof args.direction === 'string' ? args.direction : 'down'
      const amount = typeof args.amount === 'number' && Number.isFinite(args.amount) ? args.amount : 500
      return withActivity('Scrolling page', direction, async () => {
        await tab.view.webContents.executeJavaScript(
          `(() => {
            const dir = ${JSON.stringify(direction)};
            const amt = ${JSON.stringify(amount)};
            if (dir === 'top') window.scrollTo({ top: 0, behavior: 'instant' });
            else if (dir === 'bottom') window.scrollTo({ top: document.documentElement.scrollHeight, behavior: 'instant' });
            else if (dir === 'up') window.scrollBy({ top: -amt, behavior: 'instant' });
            else window.scrollBy({ top: amt, behavior: 'instant' });
          })()`,
          true
        )
        const snapshot = await captureSnapshot(tab)
        return textAnswer(formatBrowserPageSnapshot(snapshot))
      })
    }

    if (toolName === 'browser_screenshot') {
      return withActivity('Taking screenshot', tab.url || tab.title, async () => {
        const image = await tab.view.webContents.capturePage()
        const png = image.toPNG()
        const size = image.getSize()
        options.onToolReceipt?.({
          missionId: run.missionId,
          runId: run.runId,
          toolName,
          summary: `Captured screenshot of ${tab.url || 'page'} (${String(size.width)}x${String(size.height)})`
        })
        return {
          content: [
            {
              type: 'image',
              data: png.toString('base64'),
              mimeType: 'image/png'
            },
            {
              type: 'text',
              text: `Screenshot captured (${String(size.width)}x${String(size.height)} PNG) of ${tab.title || '(Untitled)'} (${tab.url || 'about:blank'}).`
            }
          ]
        }
      })
    }

    if (toolName === 'browser_console') {
      return withActivity('Reading console', tab.url || tab.title, async () => {
        if (tab.consoleEntries.length === 0) {
          return textAnswer('Console is empty for this tab.')
        }
        const lines = tab.consoleEntries.map(
          (entry) => `[${entry.level.toUpperCase()}] ${entry.message}${entry.source ? ` (${entry.source}${entry.line ? `:${String(entry.line)}` : ''})` : ''}`
        )
        return textAnswer(lines.join('\n'))
      })
    }

    if (toolName === 'browser_network') {
      return withActivity('Reading network log', tab.url || tab.title, async () => {
        if (tab.networkEntries.length === 0) {
          return textAnswer('No network requests recorded for this tab yet.')
        }
        const lines = tab.networkEntries.map(
          (entry) =>
            `${entry.method} ${entry.url} -> ${entry.status !== undefined ? String(entry.status) : entry.error ?? 'pending'}${entry.resourceType ? ` (${entry.resourceType})` : ''}`
        )
        return textAnswer(lines.join('\n'))
      })
    }

    if (toolName === 'browser_back') {
      return withActivity('Going back', tab.url, async () => {
        if (tab.view.webContents.navigationHistory.canGoBack()) {
          tab.view.webContents.navigationHistory.goBack()
          await new Promise((r) => setTimeout(r, 100))
        }
        const snapshot = await captureSnapshot(tab)
        return textAnswer(formatBrowserPageSnapshot(snapshot))
      })
    }

    if (toolName === 'browser_forward') {
      return withActivity('Going forward', tab.url, async () => {
        if (tab.view.webContents.navigationHistory.canGoForward()) {
          tab.view.webContents.navigationHistory.goForward()
          await new Promise((r) => setTimeout(r, 100))
        }
        const snapshot = await captureSnapshot(tab)
        return textAnswer(formatBrowserPageSnapshot(snapshot))
      })
    }

    return textAnswer(`Unknown browser tool "${toolName}". Nothing was changed.`, true)
  }

  const host: BrowserHost = {
    get port() {
      return port
    },

    get viewCount() {
      return tabs.size
    },

    async start() {
      if (server !== undefined) return
      if (starting !== undefined) return starting
      starting = (async () => {
        const listener = createServer((req, res) => {
          if (req.method !== 'POST' || req.url !== '/browser/call') {
            res.writeHead(404).end()
            return
          }
          const authHeader = typeof req.headers.authorization === 'string' ? req.headers.authorization : ''
          const bearer = authHeader.startsWith('Bearer ') ? authHeader.slice(7).trim() : ''
          let matchedRun: RegisteredRun | undefined
          if (bearer.length > 0) {
            const givenBuf = Buffer.from(bearer, 'utf8')
            for (const [token, candidate] of runsByToken) {
              const tokenBuf = Buffer.from(token, 'utf8')
              if (tokenBuf.length === givenBuf.length && timingSafeEqual(tokenBuf, givenBuf)) {
                matchedRun = candidate
                break
              }
            }
          }
          if (matchedRun === undefined) {
            res.writeHead(401, { 'content-type': 'application/json' }).end(
              JSON.stringify(textAnswer('This browser tool call did not come from an active Locust run. Nothing was changed.', true))
            )
            return
          }
          const chunks: Buffer[] = []
          let size = 0
          req.on('data', (chunk: Buffer) => {
            size += chunk.length
            if (size <= 512 * 1024) chunks.push(chunk)
          })
          req.on('end', () => {
            let parsed: Record<string, unknown> = {}
            try {
              parsed = JSON.parse(Buffer.concat(chunks).toString('utf8')) as Record<string, unknown>
            } catch {
              res.writeHead(400, { 'content-type': 'application/json' }).end(
                JSON.stringify(textAnswer('Malformed browser tool request. Nothing was changed.', true))
              )
              return
            }
            const toolName = typeof parsed.name === 'string' ? parsed.name : ''
            const toolArgs =
              typeof parsed.arguments === 'object' && parsed.arguments !== null
                ? (parsed.arguments as Record<string, unknown>)
                : {}
            void executeTool(matchedRun, toolName, toolArgs)
              .then((answer) => {
                res.writeHead(200, { 'content-type': 'application/json' }).end(JSON.stringify(answer))
              })
              .catch(() => {
                res.writeHead(200, { 'content-type': 'application/json' }).end(
                  JSON.stringify(textAnswer('The browser tool call did not complete. Nothing was retried.', true))
                )
              })
          })
        })
        await new Promise<void>((resolve, reject) => {
          listener.once('error', reject)
          listener.listen(options.port ?? 0, '127.0.0.1', () => resolve())
        })
        const addr = listener.address()
        port = typeof addr === 'object' && addr !== null ? addr.port : 0
        server = listener
      })().finally(() => {
        starting = undefined
      })
      return starting
    },

    attachWindow(win) {
      mainWindow = win
      syncViewAttachment()
    },

    getState() {
      return buildState()
    },

    async openPanel(rawUrl) {
      const refusal = options.policyRefusal?.()
      if (refusal !== undefined) {
        lastError = refusal
        return notifyState()
      }
      panelOpen = true
      const tab = ensureActiveTab()
      syncViewAttachment()
      if (rawUrl && rawUrl.trim().length > 0) {
        await navigateTab(tab, rawUrl, { initiatedByPerson: true })
      }
      return notifyState()
    },

    closePanel() {
      panelOpen = false
      syncViewAttachment()
      return notifyState()
    },

    setBounds(bounds) {
      currentBounds = bounds
      syncViewAttachment()
      if (bounds !== null && activeTabId !== undefined) {
        const active = tabs.get(activeTabId)
        if (active) void refreshTabPreview(active)
      }
    },

    async navigate(rawUrl, tabId) {
      const refusal = options.policyRefusal?.()
      if (refusal !== undefined) {
        lastError = refusal
        return { ok: false, message: refusal, state: notifyState() }
      }
      panelOpen = true
      const tab = ensureActiveTab(tabId)
      syncViewAttachment()
      const res = await navigateTab(tab, rawUrl, { initiatedByPerson: true })
      return { ok: res.ok, ...(res.message ? { message: res.message } : {}), state: buildState() }
    },

    async goBack(tabId) {
      const tab = ensureActiveTab(tabId)
      if (!tab.view.webContents.isDestroyed() && tab.view.webContents.navigationHistory.canGoBack()) {
        tab.view.webContents.navigationHistory.goBack()
      }
      return notifyState()
    },

    async goForward(tabId) {
      const tab = ensureActiveTab(tabId)
      if (!tab.view.webContents.isDestroyed() && tab.view.webContents.navigationHistory.canGoForward()) {
        tab.view.webContents.navigationHistory.goForward()
      }
      return notifyState()
    },

    async reload(tabId) {
      const tab = ensureActiveTab(tabId)
      if (!tab.view.webContents.isDestroyed()) {
        tab.view.webContents.reload()
      }
      return notifyState()
    },

    async stop(tabId) {
      const tab = ensureActiveTab(tabId)
      if (!tab.view.webContents.isDestroyed()) {
        tab.view.webContents.stop()
      }
      return notifyState()
    },

    async newTab(rawUrl, profileId) {
      const refusal = options.policyRefusal?.()
      if (refusal !== undefined) {
        lastError = refusal
        return notifyState()
      }
      const profile = profileId === undefined ? undefined : await options.profileOf?.(profileId).catch(() => undefined)
      if (profileId !== undefined && profile === undefined) {
        lastError = 'That teammate does not have its own browser sign-ins any more, so no tab was opened.'
        return notifyState()
      }
      panelOpen = true
      const tab = createTabInternal(profile)
      if (rawUrl && rawUrl.trim().length > 0) {
        await navigateTab(tab, rawUrl, { initiatedByPerson: true })
      }
      return notifyState()
    },

    selectTab(tabId) {
      if (tabs.has(tabId)) {
        activeTabId = tabId
        lastError = undefined
        syncViewAttachment()
      }
      return notifyState()
    },

    async point() {
      const refused = beginBrowserPoint(options.policyRefusal?.())
      if (refused !== undefined) return refused
      const tab = activeTabId === undefined ? undefined : tabs.get(activeTabId)
      if (tab === undefined || tab.url.length === 0 || tab.view.webContents.isDestroyed()) {
        return { ok: false, message: 'There is no page to point at. Open one in the browser first.' }
      }
      if (pointing) return { ok: false, message: 'Already pointing at the page. Press the button again or Esc to stop.' }
      pointing = true
      const generation = ++pointGeneration
      try {
        const answer = await pointInBrowser(pageOf(tab.view.webContents))
        if (!tab.view.webContents.isDestroyed()) {
          const before = tab.canPoint
          if (!answer.ok && answer.blocked === true) tab.canPoint = false
          else if (answer.ok) tab.canPoint = true
          if (tab.canPoint !== before) notifyState()
        }
        return answer
      } finally {
        if (generation === pointGeneration) pointing = false
      }
    },

    cancelPoint() {
      pointGeneration += 1
      pointing = false
      const tab = activeTabId === undefined ? undefined : tabs.get(activeTabId)
      if (tab === undefined || tab.view.webContents.isDestroyed()) return Promise.resolve()
      return tab.view.webContents.executeJavaScript(BROWSER_POINT_CANCEL).then(() => undefined, () => undefined)
    },

    closeTab(tabId) {
      const tab = tabs.get(tabId)
      if (!tab) return notifyState()
      if (mainWindow && !mainWindow.isDestroyed() && mainWindow.contentView.children.includes(tab.view)) {
        mainWindow.contentView.removeChildView(tab.view)
      }
      tabs.delete(tabId)
      if (!tab.view.webContents.isDestroyed()) {
        tab.view.webContents.close()
      }
      if (activeTabId === tabId) {
        const remaining = [...tabs.keys()]
        activeTabId = remaining[remaining.length - 1]
      }
      if (tabs.size === 0) {
        activeTabId = undefined
      }
      syncViewAttachment()
      return notifyState()
    },

    async forgetProfile(teammateId) {
      for (const tab of [...tabs.values()]) {
        if (tab.profile?.id === teammateId) host.closeTab(tab.id)
      }
      const partition = browserPartitionFor(teammateId)
      const part = sessions.get(partition) ?? session.fromPartition(partition)
      sessions.delete(partition)
      await part.clearStorageData().catch(() => undefined)
      await part.clearCache().catch(() => undefined)
      return notifyState()
    },

    async registerRun(run) {
      if (server === undefined) await host.start()
      const token = randomBytes(32).toString('hex')
      const configDir = await mkdtemp(join(tmpdir(), 'locust-browser-'))
      const configPath = join(configDir, 'mcp.json')
      const copilotConfigPath = join(configDir, 'copilot-mcp.json')
      const url = `http://127.0.0.1:${String(port)}/browser/call`
      const env: Readonly<Record<string, string>> = {
        ELECTRON_RUN_AS_NODE: '1',
        LOCUST_BROWSER_URL: url,
        LOCUST_BROWSER_TOKEN: token,
        LOCUST_BROWSER_MISSION_ID: run.missionId,
        LOCUST_BROWSER_TEAMMATE: run.teammateLabel
      }
      await writeFile(
        configPath,
        createBrowserMcpConfigJson({
          node: options.node,
          bridgePath: options.bridgePath,
          url,
          token,
          missionId: run.missionId,
          teammateLabel: run.teammateLabel
        }),
        'utf8'
      )
      await writeFile(
        copilotConfigPath,
        createBrowserMcpConfigJson({
          node: options.node,
          bridgePath: options.bridgePath,
          url,
          token,
          missionId: run.missionId,
          teammateLabel: run.teammateLabel,
          forCopilot: true
        }),
        'utf8'
      )
      const registered: RegisteredRun = {
        ...run,
        token,
        configDir,
        approvedGates: new Set<string>()
      }
      runsByToken.set(token, registered)
      tokensByRunId.set(run.runId, token)
      return {
        configPath,
        copilotConfigPath,
        command: options.node,
        args: [options.bridgePath],
        env
      }
    },

    async releaseRun(runId) {
      const token = tokensByRunId.get(runId)
      tokensByRunId.delete(runId)
      if (token === undefined) return
      const registered = runsByToken.get(token)
      runsByToken.delete(token)
      if (activeToolRun?.runId === runId) {
        activeToolRun = undefined
        currentActivity = undefined
        notifyState()
      }
      if (registered !== undefined) {
        await rm(registered.configDir, { recursive: true, force: true }).catch(() => undefined)
      }
    },

    async callTool(runId, toolName, args) {
      let token = tokensByRunId.get(runId)
      let registered = token ? runsByToken.get(token) : undefined
      if (!registered) {
        registered = {
          runId,
          missionId: runId,
          teammateLabel: 'Teammate',
          cwd: options.getWorkspaceRoot() ?? process.cwd(),
          token: randomUUID(),
          configDir: '',
          approvedGates: new Set<string>()
        }
      }
      return executeTool(registered, toolName, args)
    },

    async dispose() {
      for (const runId of [...tokensByRunId.keys()]) {
        await host.releaseRun(runId)
      }
      for (const tabId of [...tabs.keys()]) {
        host.closeTab(tabId)
      }
      const listener = server
      server = undefined
      if (listener !== undefined) {
        await new Promise<void>((resolve) => listener.close(() => resolve()))
      }
    }
  }

  return host
}
