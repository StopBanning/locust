import { renderToStaticMarkup } from 'react-dom/server'
import { afterEach, describe, expect, it } from 'vitest'

import { WINDOW_ONLY_SAYS } from '../../shared/desk-calls.js'
import { fileMenuChoices } from '../../shared/file-menu.js'
import type { AppUpdateState, PublicTeammate, TeammateRoute } from '../../shared/ipc.js'
import type { TeammateKeysApi } from '../../shared/teammate-keys.js'
import type { VoiceCallApi } from '../../shared/voice-call.js'
import { Composer, type ComposerProps } from './components/Composer.js'
import { PagePreview } from './components/DocPreview.js'
import { KeyCard, TeammateKeysField } from './components/KeyCard.js'
import { NewTeammateDialog } from './components/NewTeammateDialog.js'
import { PetPickTiles, PetRemovalControls, type PetRemoval } from './components/PetPicks.js'
import { PlusMenu } from './components/PlusMenu.js'
import { SaveRecordDialog } from './components/SaveRecordDialog.js'
import { UpdateBanner } from './components/Screens.js'
import { ShareTeamDialog } from './components/TeamCard.js'
import { VoiceCallButton } from './components/VoiceCall.js'
import APP from './App.tsx?raw'
import ACTIVITY from './components/ActivityCard.tsx?raw'
import CHANGES from './components/ChangesPanel.tsx?raw'
import FIRST_LAUNCH from './components/FirstLaunch.tsx?raw'
import INSPECTOR from './components/Inspector.tsx?raw'
import REPLY_PAGE from './components/ReplyPage.tsx?raw'
import THREAD from './components/Thread.tsx?raw'
import VIEWER from './components/FileViewer.tsx?raw'

/**
 * THE WINDOW IN A BROWSER SAYS WHAT STAYS ON THE COMPUTER (0.850). After Settings (0.849), the rest of the window:
 * the chat box's microphone, Talk, Attach and Choose folder, a conversation's files, the cards in it, and the menus.
 * Each control whose call a browser is refused either works another way there, is held and says where it works,
 * or is not drawn -- never a press that fails, or says something untrue.
 */
const browser = (): void => {
  ;(globalThis as { window?: unknown }).window = {
    desktop: { remote: { computer: 'your computer' }, platform: 'darwin' },
    // The chat box's glow asks for it as it draws.
    matchMedia: () => ({ matches: false, addEventListener: () => undefined, removeEventListener: () => undefined })
  }
}
afterEach(() => {
  delete (globalThis as { window?: unknown }).window
})
const noop = (): void => undefined

describe('the chat box in a browser', () => {
  const props = (over: Partial<ComposerProps> = {}): ComposerProps => ({
    runtimes: [], limitedRuntimes: new Map(), discoveryPhase: 'ready', running: false, cancelling: false, activeRoute: undefined,
    error: undefined, mode: 'accept-edits', onModeChange: noop, route: { runtime: 'codex', model: 'account-default' }, onRouteChange: noop,
    models: [], resolvedModels: new Map(), recentRoutes: [], platform: 'darwin', effort: undefined, onEffortChange: noop, swarm: false,
    onSwarmChange: noop, onStart: async () => true, onCancel: noop, onOpenRoutePicker: noop, onHandOff: noop, handingOff: false,
    workspaceName: 'shop', workspacePath: '/Users/me/shop', onChooseFolder: noop, teammateName: undefined, busyWith: undefined,
    queued: undefined, queuedNote: undefined, onQueue: noop, onUnqueue: noop, onSendQueued: noop, continuationNote: undefined,
    queuedElsewhere: false, ...over
  })

  it("attaches with the browser's own file picker, which only a browser draws", () => {
    expect(renderToStaticMarkup(<Composer {...props()} />)).not.toContain('type="file"')
    browser()
    expect(renderToStaticMarkup(<Composer {...props()} />)).toMatch(/<input type="file" multiple="" class="lc-filepick" tabindex="-1" aria-hidden="true"\/>/)
  })

  it("holds Choose a folder in the + menu, and says where it works", () => {
    const held = renderToStaticMarkup(<PlusMenu disabled={false} folderMissing={false} onAttach={noop} onChooseFolder={noop} folderStays={WINDOW_ONLY_SAYS} />)
    expect(held).toMatch(new RegExp(`data-satellite="folder" aria-label="Choose a folder, on your computer" title="${WINDOW_ONLY_SAYS}" disabled=""`))
    // Attach still works there: its own picker.
    expect(held).not.toMatch(/data-satellite="attach"[^>]*disabled=""/)
    const free = renderToStaticMarkup(<PlusMenu disabled={false} folderMissing={false} onAttach={noop} onChooseFolder={noop} />)
    expect(free).not.toContain('disabled=""')
    browser()
    expect(renderToStaticMarkup(<Composer {...props()} />)).toContain('aria-label="Choose a folder, on your computer"')
  })

  it('holds Talk, which is the computer’s microphone and speakers', () => {
    const api: VoiceCallApi = {
      available: async () => ({ available: true }), start: async () => ({ ok: false, message: 'not in a test' }), stop: async () => undefined,
      answer: async () => undefined, voice: async () => 'cove' as const, setVoice: async (voice) => voice, onEvent: () => () => undefined
    }
    const talk = (): string => renderToStaticMarkup(<VoiceCallButton platform="darwin" api={api} availability={{ available: true }} onText={noop} />)
    expect(talk()).not.toContain('disabled=""')
    browser()
    expect(talk()).toContain('disabled=""')
    expect(talk()).toContain('aria-label="Talk, on your computer"')
    expect(talk()).toContain(`title="${WINDOW_ONLY_SAYS}"`)
  })
})

describe("a conversation's files in a browser", () => {
  it('offers neither the default app nor the file manager on a right-click: both open on the computer', () => {
    const ids = (inBrowser: boolean): readonly string[] =>
      fileMenuChoices({ path: '/Users/me/shop/notes.md', folder: '/Users/me/shop', platform: 'darwin', editors: [], inBrowser }).map((choice) => choice.id)
    expect(ids(false)).toEqual(['attach', 'default', 'reveal', 'copy-path', 'copy-relative', 'copy-name'])
    expect(ids(true)).toEqual(['attach', 'copy-path', 'copy-relative', 'copy-name'])
    expect(APP).toContain('inBrowser: inABrowser()')
    // Its copies go to the browser's own clipboard, not the computer's.
    expect(APP).toMatch(/if \(inABrowser\(\)\) \{\s*await copyText\(text\)\s*return\s*\}\s*const wrote = await window\.desktop\?\.writeClipboard\(text\)/)
  })

  it("draws no folder button on a file's row, in the viewer, or in the header, and holds a sent file without a panel", () => {
    expect(ACTIVITY.match(/elsewhere === undefined && revealHere && \(/g)).toHaveLength(2)
    expect(INSPECTOR).toContain('{onTheComputer(WORKSPACE_REVEAL_CHANNEL) === undefined && <button')
    expect(VIEWER).toContain('{onTheComputer(WORKSPACE_REVEAL_CHANNEL) === undefined && (')
    expect(VIEWER).toContain('{onTheComputer(WORKSPACE_SAVE_COPY_CHANNEL) === undefined && (')
    expect(APP).toContain('{workspacePath !== undefined && onTheComputer(WORKSPACE_OPEN_CHANNEL) === undefined && (')
    expect(THREAD).toContain('{saveStays === undefined && (')
    expect(THREAD).toContain('disabled={revealStays !== undefined && onOpenFile === undefined}')
  })

  it('says a running page is served from the computer, and keeps its code', () => {
    expect(renderToStaticMarkup(<PagePreview path="/Users/me/shop/game.html" name="game.html" />)).toContain('Opening the page…')
    browser()
    expect(renderToStaticMarkup(<PagePreview path="/Users/me/shop/game.html" name="game.html" />)).toContain('This page runs in Locust on your computer.')
    expect(REPLY_PAGE).toContain("setRefused('This page runs in Locust on your computer. Its code is in the Code tab.')")
  })

  it('holds Walk me through it, which runs a teammate on the computer', () => {
    expect(CHANGES).toContain('disabled={onTheComputer(WALKTHROUGH_CHANNEL) !== undefined}')
  })
})

describe('the cards and dialogs in a browser', () => {
  const keys: TeammateKeysApi = {
    list: async () => ({ ok: true, names: [] }), set: async () => ({ ok: true, names: [] }), remove: async () => ({ ok: true, names: [] })
  }

  it('asks for a key without a box to type it into: keys are given on the computer alone', () => {
    const card = (): string => renderToStaticMarkup(<KeyCard teammateId="tm_wren" name="Wren" requests={[{ name: 'STRIPE_KEY', why: 'for the shop' }]} reaches api={keys} />)
    expect(card()).toContain('type="password"')
    browser()
    expect(card()).not.toContain('<input')
    expect(card()).toContain('Keys are given in Locust on your computer, never from a browser.')
    const field = renderToStaticMarkup(<TeammateKeysField teammateId="tm_wren" name="Wren" api={keys} />)
    expect(field).not.toContain('<input')
    expect(field).toContain(WINDOW_ONLY_SAYS)
  })

  it('holds what saves through the computer’s own dialogs, and says so before a press', () => {
    browser()
    for (const html of [
      renderToStaticMarkup(<SaveRecordDialog missionId="m_1" onClose={noop} />),
      renderToStaticMarkup(<ShareTeamDialog teammates={[]} onClose={noop} />)
    ]) {
      expect(html).toMatch(new RegExp(`class="lc-primarybutton" disabled="" title="${WINDOW_ONLY_SAYS}"`))
      expect(html).toContain(`role="status">${WINDOW_ONLY_SAYS}</p>`)
    }
  })

  it('tells a browser an update is ready on the computer, with Restart and install held', () => {
    const update = { phase: 'ready', currentVersion: '0.850.0', availableVersion: '0.851.0' } as AppUpdateState
    expect(renderToStaticMarkup(<UpdateBanner update={update} onInstall={async () => ({ ok: true }) as never} />)).not.toContain('disabled=""')
    browser()
    expect(renderToStaticMarkup(<UpdateBanner update={update} onInstall={async () => ({ ok: true }) as never} />)).toContain(`disabled="" title="${WINDOW_ONLY_SAYS}"`)
    expect(FIRST_LAUNCH).toContain('disabled={waiting || npmMissing || onTheComputer(RUNTIME_INSTALL_CHANNEL) !== undefined}')
  })

  it('holds the pets a browser would have to download, and removing any', () => {
    const removal = { ticked: undefined, start: noop } as unknown as PetRemoval
    expect(renderToStaticMarkup(<PetRemovalControls removal={removal} />)).not.toContain('disabled=""')
    expect(renderToStaticMarkup(<PetPickTiles selected={undefined} installed={[]} onWear={noop} onNotice={noop} />)).not.toContain('disabled=""')
    browser()
    expect(renderToStaticMarkup(<PetRemovalControls removal={removal} />)).toContain(`disabled="" title="${WINDOW_ONLY_SAYS}"`)
    const tiles = renderToStaticMarkup(<PetPickTiles selected={undefined} installed={[]} onWear={noop} onNotice={noop} />)
    expect(tiles).toContain(WINDOW_ONLY_SAYS)
    expect(tiles).not.toMatch(/data-here="no"(?![^>]*disabled="")/)
  })

  it("holds what a teammate may reach and spend in its editor, and keeps the rest of it", () => {
    const wren = {
      teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code', avatar: { shape: 'classic', eyes: 'dots', antenna: 'single', mouth: 'smile' },
      createdAt: '2026-09-05T05:00:00.000Z', monthlyLimitUsd: 5
    } as unknown as PublicTeammate
    const chatBox: TeammateRoute = { runtime: 'claude', model: 'sonnet', mode: 'accept-edits' }
    const picker = { runtimes: [], models: [], resolvedModels: new Map<string, string>(), recentRoutes: [], limitedRuntimes: new Map<string, string>() }
    const editor = (): string => renderToStaticMarkup(
      <NewTeammateDialog
        initial={wren} mode="accept-edits" error={undefined} onCancel={noop} onCreate={noop} composerRoute={chatBox} picker={picker}
        onChooseFolder={noop} connectors={[{ name: 'gmail', status: 'connected', location: 'user' }] as never} onSetConnectors={noop} onSetOwnBrowser={noop}
      />
    )
    expect(editor()).not.toContain('lc-oncomputer')
    browser()
    const html = editor()
    expect(html.match(/<fieldset class="lc-oncomputer" disabled="">/g)).toHaveLength(3)
    expect(html).toMatch(/<fieldset class="lc-oncomputer" disabled=""><div class="lc-folderrow">/)
    expect(html).toMatch(/<fieldset class="lc-oncomputer" disabled=""><div class="lc-connectorgrid"/)
    expect(html).toMatch(/<fieldset class="lc-oncomputer" disabled=""><div class="lc-field lc-field--switch">/)
    expect(html).toMatch(/inputMode="decimal"[^>]*disabled=""/)
    // The name and the instructions still change from here.
    expect(html).not.toMatch(/aria-label="Instructions for this teammate"[^>]*disabled=""/)
  })
})

describe('the menus and sends in a browser', () => {
  it('offers neither Open in terminal nor Send feedback, and holds Save the record and Import with the reason', () => {
    expect(APP).toContain('&& onTheComputer(OPEN_IN_TERMINAL_CHANNEL) === undefined) {')
    expect(APP).toContain('if (liveRun !== undefined && onTheComputer(FEEDBACK_CHANNEL) === undefined) {')
    expect(APP).toContain('{ disabledReason: onTheComputer(MISSION_RECORD_SAVE_CHANNEL)! }')
    expect(APP).toContain('{ disabledReason: onTheComputer(SESSION_IMPORT_CHANNEL)! }')
  })

  it("offers no background turn, and says the makers' clouds start from the computer", () => {
    expect(APP).toContain("composerRoute.runtime === 'claude' && comparing === undefined && onTheComputer(BACKGROUND_START_CHANNEL) === undefined")
    expect(APP).toContain('? { refusal: onTheComputer(CLOUD_START_CHANNEL)! }')
  })

  it('says a message to a cloud session was not sent, never that it is unknown, when a browser was refused it', () => {
    const send = APP.slice(APP.indexOf('onSend: async (id, message) => {'))
    expect(send.indexOf('onTheComputer(CLAUDE_CLOUD_SEND_CHANNEL)')).toBeGreaterThan(-1)
    expect(send.indexOf('onTheComputer(CLAUDE_CLOUD_SEND_CHANNEL)')).toBeLessThan(send.indexOf('.sendClaudeCloud(id, message)'))
  })
})
