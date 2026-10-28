import type { MissionRuntimeId } from '@teammate/runtime-adapters'
import type { BrowserPointResponse } from './browser-point.js'

export const BROWSER_OPEN_PANEL_CHANNEL = 'locust:browser:open-panel'
export const BROWSER_CLOSE_PANEL_CHANNEL = 'locust:browser:close-panel'
export const BROWSER_BOUNDS_CHANNEL = 'locust:browser:bounds'
export const BROWSER_NAVIGATE_CHANNEL = 'locust:browser:navigate'
export const BROWSER_BACK_CHANNEL = 'locust:browser:back'
export const BROWSER_FORWARD_CHANNEL = 'locust:browser:forward'
export const BROWSER_RELOAD_CHANNEL = 'locust:browser:reload'
export const BROWSER_STOP_CHANNEL = 'locust:browser:stop'
export const BROWSER_NEW_TAB_CHANNEL = 'locust:browser:new-tab'
export const BROWSER_SELECT_TAB_CHANNEL = 'locust:browser:select-tab'
export const BROWSER_CLOSE_TAB_CHANNEL = 'locust:browser:close-tab'
export const BROWSER_STATE_CHANNEL = 'locust:browser:state'
export const BROWSER_EVENT_CHANNEL = 'locust:browser:event'
export const BROWSER_POINT_CHANNEL = 'locust:browser:point'
export const BROWSER_POINT_CANCEL_CHANNEL = 'locust:browser:point-cancel'
export const BROWSER_POINT_HOTKEY_CHANNEL = 'locust:browser:point-hotkey'

/**
 * Isolated persistent session for the Browser panel. Never reads or imports
 * the person's Chrome profile, cookies, or saved passwords; logins made in
 * the panel stay across restarts inside Locust's own partition.
 */
export const BROWSER_PARTITION = 'persist:locust-browser'

/**
 * A TEAMMATE'S OWN SIGN-INS (0.839, the OpenBot scrub's take 2). OpenBot gives each agent a named browser profile,
 * colored on its tabs, so two agents can be in two accounts on one site. A teammate with Own browser sign-ins on gets
 * a partition of its own beside the shared one: its tabs carry its name and color in the panel, its tools see and use
 * only its own tabs, and no other teammate's tools ever reach them. The person can open a tab as that teammate to sign
 * it in. Turned off, or the teammate removed, its partition is cleared: those sign-ins are gone.
 */
export interface BrowserProfile {
  /** The teammate's id, which names its partition. */
  readonly id: string
  readonly name: string
  readonly hue: string
}

export function browserPartitionFor(profileId: string | undefined): string {
  return profileId === undefined ? BROWSER_PARTITION : `${BROWSER_PARTITION}-${profileId}`
}

export interface BrowserRect {
  readonly x: number
  readonly y: number
  readonly width: number
  readonly height: number
}

export interface BrowserTabState {
  readonly id: string
  readonly title: string
  readonly url: string
  readonly loading: boolean
  readonly canGoBack: boolean
  readonly canGoForward: boolean
  readonly previewDataUrl?: string
  /** False when the page will not run the picker. Absent until that is known. */
  readonly canPoint?: boolean
  /** Whose own sign-ins the tab is in (0.839). Absent: the shared ones. */
  readonly profile?: BrowserProfile
}

export interface BrowserActivity {
  readonly label: string
  readonly action: string
  readonly detail?: string
  readonly missionId?: string
}

export interface BrowserPanelState {
  readonly open: boolean
  readonly tabs: readonly BrowserTabState[]
  readonly activeTabId?: string
  readonly activity?: BrowserActivity
  readonly error?: string
}

export interface BrowserElementSnapshot {
  readonly ref: string
  readonly role: string
  readonly name: string
  readonly tag: string
  readonly type?: string
  readonly value?: string
  readonly placeholder?: string
  readonly href?: string
  readonly disabled?: boolean
  readonly checked?: boolean
  readonly sensitive?: boolean
  readonly submit?: boolean
  readonly download?: boolean
  readonly formAction?: string
}

export interface BrowserPageSnapshot {
  readonly tabId: string
  readonly title: string
  readonly url: string
  readonly text: string
  readonly elements: readonly BrowserElementSnapshot[]
}

export interface BrowserConsoleEntry {
  readonly level: 'info' | 'warning' | 'error'
  readonly message: string
  readonly source?: string
  readonly line?: number
  readonly at: string
}

export interface BrowserNetworkEntry {
  readonly method: string
  readonly url: string
  readonly status?: number
  readonly resourceType?: string
  readonly ok?: boolean
  readonly error?: string
  readonly at: string
}

export type BrowserApprovalGate =
  | 'credential'
  | 'submit'
  | 'purchase'
  | 'send'
  | 'download'
  | 'leave-origin'

export interface BrowserApprovalDecision {
  readonly gate: BrowserApprovalGate
  readonly title: string
  readonly summary: string
  readonly detail: string
}

export const BROWSER_MCP_RUNTIMES = ['claude', 'codex', 'opencode', 'copilot', 'acp'] as const
export type BrowserMcpRuntimeId = (typeof BROWSER_MCP_RUNTIMES)[number]

export function supportsPerRunBrowserMcp(runtime: MissionRuntimeId | string): runtime is BrowserMcpRuntimeId {
  return (BROWSER_MCP_RUNTIMES as readonly string[]).includes(runtime)
}

const RUNTIME_LABELS: Readonly<Record<string, string>> = {
  cursor: 'Cursor Agent',
  antigravity: 'Antigravity',
  muse: 'Muse',
  gemini: 'Gemini CLI',
  omniroute: 'OmniRoute'
}

export function browserRuntimeUnsupportedNote(runtime: MissionRuntimeId | string, displayName?: string): string | undefined {
  if (supportsPerRunBrowserMcp(runtime)) return undefined
  const label = displayName?.trim() || RUNTIME_LABELS[runtime] || runtime
  return `${label} does not accept per-run MCP tools, and Locust never edits your global agent settings. You can still browse here yourself, or switch this conversation to Claude Code, Codex, OpenCode, Copilot CLI, or an ACP agent when a teammate needs the browser.`
}

export type ResolvedBrowserUrl =
  | { readonly ok: true; readonly url: string; readonly origin: string; readonly filePath?: string }
  | { readonly ok: false; readonly message: string }

export function browserOriginForUrl(rawUrl: string): string | undefined {
  try {
    const parsed = new URL(rawUrl)
    if (parsed.protocol === 'http:' || parsed.protocol === 'https:') {
      return parsed.origin.toLowerCase()
    }
    if (parsed.protocol === 'file:') {
      return 'file://workspace'
    }
    return undefined
  } catch {
    return undefined
  }
}

export function isLoopbackHost(hostname: string): boolean {
  const clean = hostname.toLowerCase().replace(/^\[|\]$/g, '')
  return clean === 'localhost' || clean === '127.0.0.1' || clean === '::1' || clean.endsWith('.localhost')
}

export function isLoopbackOrWorkspaceOrigin(originOrUrl: string): boolean {
  if (originOrUrl === 'file://workspace' || originOrUrl.startsWith('file:')) return true
  try {
    const parsed = new URL(originOrUrl)
    if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') return false
    return isLoopbackHost(parsed.hostname)
  } catch {
    return false
  }
}

const CREDENTIAL_PATTERN =
  /\b(password|passcode|passphrase|secret|api[-_\s]?key|token|otp|2fa|two[-_\s]?factor|verification\s+code|auth\s+code|pin\b|cvv|cvc|csc|card\s*number|cc[-_\s]?number|cc[-_\s]?exp|cc[-_\s]?csc|credit\s*card|debit\s*card|routing\s*number|bank\s*account|iban|ssn|social\s*security|tax\s*id)\b/i

const PURCHASE_PATTERN =
  /\b(buy\s+now|place\s+order|complete\s+(purchase|order)|checkout|check\s+out|pay\s+now|pay\s+\$?\d|confirm\s+(purchase|payment|order)|authorize\s+payment|subscribe\s+now|purchase\s+now|one[-_\s]?click\s+buy)\b/i

const SEND_PATTERN =
  /\b(send(\s+(message|email|mail|reply|invite|invitation|money|payment|transfer|post|dm|note))?|post(\s+(comment|reply|update|message|tweet|status|review))?|publish(\s+(post|article|page|release))?|tweet|submit\s+(comment|reply|review|post|message))\b/i

const SUBMIT_BUTTON_PATTERN =
  /\b(submit|sign\s*in|log\s*in|sign\s*up|register|create\s+account|continue\s+with|authorize|allow\s+access|grant\s+access|confirm|save\s+changes|apply\s+changes|delete\s+account|transfer)\b/i

const SEARCH_SAFE_PATTERN = /^\s*(search|find|filter|query|lookup|go)\s*$/i

function elementDescriptor(el: BrowserElementSnapshot | undefined): string {
  if (!el) return ''
  return [el.name, el.placeholder, el.type, el.role, el.href, el.formAction]
    .filter((part): part is string => typeof part === 'string' && part.trim().length > 0)
    .join(' ')
}

export function isSensitiveField(el: BrowserElementSnapshot | undefined): boolean {
  if (!el) return false
  if (el.sensitive === true) return true
  const type = (el.type ?? '').toLowerCase()
  if (type === 'password') return true
  return CREDENTIAL_PATTERN.test(elementDescriptor(el))
}

export interface ClassifyBrowserActionInput {
  readonly action: 'open' | 'click' | 'type' | 'press' | 'select' | 'download'
  readonly currentUrl?: string
  readonly targetUrl?: string
  readonly personOrigins?: ReadonlySet<string>
  readonly element?: BrowserElementSnapshot
  readonly key?: string
  readonly filename?: string
}

export function classifyBrowserAction(input: ClassifyBrowserActionInput): BrowserApprovalDecision | undefined {
  const { action, currentUrl, targetUrl, personOrigins, element, key, filename } = input
  const pageHost = (() => {
    if (!currentUrl) return 'this page'
    try {
      const u = new URL(currentUrl)
      return u.hostname || u.protocol
    } catch {
      return currentUrl
    }
  })()

  if (action === 'download') {
    const label = filename?.trim() || 'file'
    return {
      gate: 'download',
      title: `Download ${label} from ${pageHost}?`,
      summary: `Save ${label} from ${currentUrl || pageHost}`,
      detail: `Browser download requested on ${currentUrl || pageHost}: ${label}`
    }
  }

  if (action === 'open' && targetUrl) {
    const targetOrigin = browserOriginForUrl(targetUrl)
    const currentOrigin = currentUrl ? browserOriginForUrl(currentUrl) : undefined
    if (
      targetOrigin &&
      !isLoopbackOrWorkspaceOrigin(targetOrigin) &&
      targetOrigin !== currentOrigin &&
      !(personOrigins?.has(targetOrigin) ?? false)
    ) {
      return {
        gate: 'leave-origin',
        title: `Open ${targetOrigin} in the browser?`,
        summary: `Leave ${currentOrigin ?? 'this tab'} for ${targetUrl}`,
        detail: `Teammate wants to open ${targetUrl}, which has not been opened by you in this tab.`
      }
    }
    return undefined
  }

  if (action === 'type' || action === 'select') {
    if (isSensitiveField(element)) {
      const fieldLabel = element?.name || element?.placeholder || element?.type || 'credential or payment field'
      return {
        gate: 'credential',
        title: `Enter text into "${fieldLabel}" on ${pageHost}?`,
        summary: `Type into sensitive field "${fieldLabel}" on ${pageHost} (input value stays hidden)`,
        detail: `Teammate wants to enter a value into "${fieldLabel}" ([ref=${element?.ref ?? '?'}]) on ${currentUrl || pageHost}.`
      }
    }
    return undefined
  }

  if (action === 'click') {
    const desc = elementDescriptor(element)
    const label = element?.name || element?.placeholder || element?.href || element?.tag || 'element'

    if (element?.download) {
      return {
        gate: 'download',
        title: `Download from "${label}" on ${pageHost}?`,
        summary: `Click download link "${label}" on ${pageHost}`,
        detail: `Teammate wants to click download link "${label}" ([ref=${element.ref}]) on ${currentUrl || pageHost}.`
      }
    }

    if (PURCHASE_PATTERN.test(desc)) {
      return {
        gate: 'purchase',
        title: `Click "${label}" on ${pageHost}?`,
        summary: `Checkout or purchase action "${label}" on ${pageHost}`,
        detail: `Teammate wants to click "${label}" ([ref=${element?.ref ?? '?'}]) on ${currentUrl || pageHost}.`
      }
    }

    if (SEND_PATTERN.test(desc)) {
      return {
        gate: 'send',
        title: `Click "${label}" on ${pageHost}?`,
        summary: `Send or post via "${label}" on ${pageHost}`,
        detail: `Teammate wants to click "${label}" ([ref=${element?.ref ?? '?'}]) on ${currentUrl || pageHost}.`
      }
    }

    const isSearchControl =
      SEARCH_SAFE_PATTERN.test(element?.name ?? '') &&
      !CREDENTIAL_PATTERN.test(desc) &&
      !PURCHASE_PATTERN.test(desc) &&
      !SEND_PATTERN.test(desc)

    if (
      !isSearchControl &&
      (element?.submit === true ||
        element?.type?.toLowerCase() === 'submit' ||
        SUBMIT_BUTTON_PATTERN.test(element?.name ?? ''))
    ) {
      return {
        gate: 'submit',
        title: `Submit "${label}" on ${pageHost}?`,
        summary: `Submit form or sign-in control "${label}" on ${pageHost}`,
        detail: `Teammate wants to click "${label}" ([ref=${element?.ref ?? '?'}]) on ${currentUrl || pageHost}.`
      }
    }

    if (element?.href) {
      try {
        const resolvedHref = currentUrl ? new URL(element.href, currentUrl).toString() : element.href
        const hrefOrigin = browserOriginForUrl(resolvedHref)
        const currentOrigin = currentUrl ? browserOriginForUrl(currentUrl) : undefined
        if (
          hrefOrigin &&
          !isLoopbackOrWorkspaceOrigin(hrefOrigin) &&
          hrefOrigin !== currentOrigin &&
          !(personOrigins?.has(hrefOrigin) ?? false)
        ) {
          return {
            gate: 'leave-origin',
            title: `Leave ${pageHost} for ${hrefOrigin}?`,
            summary: `Follow link "${label}" to ${resolvedHref}`,
            detail: `Teammate wants to follow link "${label}" ([ref=${element.ref}]) to ${resolvedHref}, which has not been opened by you in this tab.`
          }
        }
      } catch {
        // Ignore unparseable hrefs here; navigation handler still guards schemes and origins.
      }
    }

    return undefined
  }

  if (action === 'press') {
    const normalizedKey = (key ?? '').trim().toLowerCase()
    if (normalizedKey === 'enter' || normalizedKey === 'return') {
      const desc = elementDescriptor(element)
      const label = element?.name || element?.placeholder || element?.role || 'form'
      if (isSensitiveField(element)) {
        return {
          gate: 'credential',
          title: `Submit credential field "${label}" on ${pageHost}?`,
          summary: `Press Enter in sensitive field "${label}" on ${pageHost}`,
          detail: `Teammate wants to press Enter inside "${label}" on ${currentUrl || pageHost}.`
        }
      }
      if (PURCHASE_PATTERN.test(desc)) {
        return {
          gate: 'purchase',
          title: `Confirm "${label}" on ${pageHost}?`,
          summary: `Press Enter on purchase control "${label}" on ${pageHost}`,
          detail: `Teammate wants to press Enter on "${label}" on ${currentUrl || pageHost}.`
        }
      }
      if (SEND_PATTERN.test(desc)) {
        return {
          gate: 'send',
          title: `Send "${label}" on ${pageHost}?`,
          summary: `Press Enter on send control "${label}" on ${pageHost}`,
          detail: `Teammate wants to press Enter on "${label}" on ${currentUrl || pageHost}.`
        }
      }
      const isSearchBox =
        element?.role === 'searchbox' ||
        element?.type === 'search' ||
        SEARCH_SAFE_PATTERN.test(element?.name ?? '') ||
        SEARCH_SAFE_PATTERN.test(element?.placeholder ?? '')
      if (!isSearchBox && element?.submit === true) {
        return {
          gate: 'submit',
          title: `Submit form via "${label}" on ${pageHost}?`,
          summary: `Press Enter to submit form from "${label}" on ${pageHost}`,
          detail: `Teammate wants to press Enter inside "${label}" on ${currentUrl || pageHost}.`
        }
      }
    }
    return undefined
  }

  return undefined
}

export function formatBrowserPageSnapshot(snapshot: BrowserPageSnapshot): string {
  const lines: string[] = [
    `Title: ${snapshot.title || '(Untitled)'}`,
    `URL: ${snapshot.url || 'about:blank'}`,
    `Tab: ${snapshot.tabId}`
  ]

  if (snapshot.elements.length > 0) {
    lines.push('', 'Interactive elements:')
    for (const el of snapshot.elements) {
      const parts = [`[ref=${el.ref}]`, el.role]
      if (el.name) parts.push(`"${el.name}"`)
      if (el.type && el.type !== el.role) parts.push(`type=${el.type}`)
      if (el.placeholder && el.placeholder !== el.name) parts.push(`placeholder="${el.placeholder}"`)
      if (el.sensitive) {
        parts.push('(sensitive field)')
      } else if (el.value) {
        parts.push(`value="${el.value}"`)
      }
      if (el.checked !== undefined) parts.push(el.checked ? 'checked' : 'unchecked')
      if (el.disabled) parts.push('disabled')
      if (el.submit) parts.push('submits-form')
      if (el.href) parts.push(`-> ${el.href}`)
      lines.push(`- ${parts.join(' ')}`)
    }
  } else {
    lines.push('', 'Interactive elements: (none)')
  }

  const trimmedText = snapshot.text.trim()
  lines.push('', 'Visible text:', trimmedText.length > 0 ? trimmedText : '(empty page)')
  return lines.join('\n')
}

export const BROWSER_TOOL_NAMES = [
  'browser_open',
  'browser_read',
  'browser_click',
  'browser_type',
  'browser_press',
  'browser_select',
  'browser_scroll',
  'browser_screenshot',
  'browser_console',
  'browser_network',
  'browser_back',
  'browser_forward',
  'browser_tabs'
] as const

export type BrowserToolName = (typeof BROWSER_TOOL_NAMES)[number]

export interface BrowserMcpServerConnection {
  readonly nodePath: string
  readonly bridgePath: string
  readonly endpointUrl: string
  readonly token: string
  readonly missionId: string
  readonly teammateLabel: string
}

export interface BrowserApi {
  state(): Promise<BrowserPanelState>
  openPanel(url?: string): Promise<BrowserPanelState>
  closePanel(): Promise<BrowserPanelState>
  setBounds(bounds: BrowserRect | null): Promise<void>
  navigate(url: string, tabId?: string): Promise<{ readonly ok: boolean; readonly message?: string; readonly state: BrowserPanelState }>
  back(tabId?: string): Promise<BrowserPanelState>
  forward(tabId?: string): Promise<BrowserPanelState>
  reload(tabId?: string): Promise<BrowserPanelState>
  stop(tabId?: string): Promise<BrowserPanelState>
  /** `profileId`: open it in that teammate's own sign-ins (0.839). */
  newTab(url?: string, profileId?: string): Promise<BrowserPanelState>
  selectTab(tabId: string): Promise<BrowserPanelState>
  closeTab(tabId: string): Promise<BrowserPanelState>
  point(): Promise<BrowserPointResponse>
  cancelPoint(): Promise<void>
  onPointHotkey(listener: () => void): () => void
  onState(listener: (state: BrowserPanelState) => void): () => void
}
