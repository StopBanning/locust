import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

import { WINDOW_ONLY } from '../../shared/desk-calls.js'
import { desktopApiOver, type DesktopIpc } from '../../shared/desktop-api.js'

/**
 * EVERY CALL A BROWSER IS REFUSED IS DECIDED WHERE THE WINDOW MAKES IT (0.850). desk-calls.ts says which calls a
 * computer answers from Locust in a browser; this says the window knows it. Colin found dead controls in Safari by
 * hand (2026-11-01): Settings rows that failed after a press, an Always that a browser may never give. Each was a call
 * the window made as if it were on the computer. Now a call to a computer-only channel is made only where it was
 * decided: held with the words (onTheComputer, OnTheComputer), left out, made to work another way, or quiet for a
 * reason written below. A new one fails here until someone decides it.
 */
const here = (path: string): string => fileURLToPath(new URL(path, import.meta.url))

/** Each of the window's calls, by its name on window.desktop, and the channel it makes: read off the API itself. */
function channels(): ReadonlyMap<string, string> {
  let seen: string[] = []
  const ipc: DesktopIpc = {
    invoke: (channel: string) => { seen.push(channel); return Promise.resolve(undefined) },
    send: () => undefined,
    on: () => ipc,
    removeListener: () => ipc
  } as unknown as DesktopIpc
  const made = new Map<string, string>()
  const walk = (from: Record<string, unknown>, prefix: string, depth: number): void => {
    for (const [key, value] of Object.entries(from)) {
      const path = prefix === '' ? key : `${prefix}.${key}`
      if (typeof value === 'function') {
        seen = []
        try {
          const answer: unknown = (value as (...args: unknown[]) => unknown)(undefined, undefined, undefined)
          if (answer instanceof Promise) answer.catch(() => undefined)
        } catch { /* It made its call, or none, before failing on no arguments. */ }
        if (seen[0] !== undefined) made.set(path, seen[0])
      } else if (value !== null && typeof value === 'object' && depth < 2) walk(value as Record<string, unknown>, path, depth + 1)
    }
  }
  walk(desktopApiOver(ipc, { platform: 'darwin', initialTheme: 'dark' }) as unknown as Record<string, unknown>, '', 0)
  return made
}

/** The window's own files: this folder, its tests left out. */
function windowFiles(): ReadonlyMap<string, string> {
  const root = here('.')
  const files = new Map<string, string>()
  const walk = (folder: string): void => {
    for (const name of readdirSync(folder)) {
      const full = join(folder, name)
      if (statSync(full).isDirectory()) walk(full)
      else if (/\.tsx?$/.test(name) && !/\.test\./.test(name)) files.set(full.slice(root.length).replaceAll('\\', '/'), readFileSync(full, 'utf8'))
    }
  }
  walk(root)
  return files
}

/** The shared channel names, by the constant the window's code spells them with. */
function channelNames(): ReadonlyMap<string, string> {
  const folder = here('../../shared/')
  const names = new Map<string, string>()
  for (const name of readdirSync(folder)) {
    if (!name.endsWith('.ts') || name.includes('.test.')) continue
    for (const match of readFileSync(join(folder, name), 'utf8').matchAll(/export const ([A-Z][A-Z0-9_]*) = '([^']+)'/g)) names.set(match[1]!, match[2]!)
  }
  return names
}

/** The bracketed text from an opening bracket to its match. */
function bracketed(text: string, open: number): string {
  let depth = 0
  for (let at = open; at < text.length; at += 1) {
    const one = text[at]
    if (one === '(' || one === '{' || one === '[') depth += 1
    else if (one === ')' || one === '}' || one === ']') {
      depth -= 1
      if (depth === 0) return text.slice(open, at + 1)
    }
  }
  return text.slice(open)
}

/** How many times a file calls each method: `.name(`, `.space.name(`, and through `api = window.desktop?.space`. */
function callsIn(text: string): ReadonlyMap<string, number> {
  const calls = new Map<string, number>()
  const count = (key: string): void => { calls.set(key, (calls.get(key) ?? 0) + 1) }
  for (const match of text.matchAll(/\.([A-Za-z_$][\w$]*)\??\.?\(/g)) count(match[1]!)
  for (const match of text.matchAll(/\.([A-Za-z_$][\w$]*)\??\.([A-Za-z_$][\w$]*)\??\.?\(/g)) count(`${match[1]!}.${match[2]!}`)
  for (const alias of text.matchAll(/\b([A-Za-z_$][\w$]*)\s*=\s*(?:typeof window === 'undefined' \? undefined : )?window\.desktop\?\.([A-Za-z_$][\w$]*)\b(?!\s*\()/g)) {
    for (const match of text.matchAll(new RegExp(`\\b${alias[1]!}\\??\\.([A-Za-z_$][\\w$]*)\\??\\.?\\(`, 'g'))) count(`${alias[2]!}.${match[1]!}`)
  }
  return calls
}

/**
 * The computer-only calls the window makes with no browser decision naming their channel, and why that is right. An
 * entry goes when its call does, or when a decision names it.
 */
const QUIET: Readonly<Record<string, string>> = {
  "applyCloudTask": "the cloud panel's buttons say the refusal (failureSaid); the makers' clouds are refused on the chat box's choice",
  "applyRestoredFolders": "on the restored folders' card, which a browser never gets (restoredFolders)",
  "bringClaudeCloudHome": "the cloud panel's buttons say the refusal (failureSaid); the makers' clouds are refused on the chat box's choice",
  "browser.cancelPoint": "the browser panel, which a browser cannot open: opening it is held in the header (BROWSER_OPEN_PANEL_CHANNEL)",
  "browser.closePanel": "the browser panel, which a browser cannot open: opening it is held in the header (BROWSER_OPEN_PANEL_CHANNEL); a refusal is caught and quiet",
  "browser.navigate": "the browser panel, which a browser cannot open: opening it is held in the header (BROWSER_OPEN_PANEL_CHANNEL)",
  "browser.reload": "the browser panel, which a browser cannot open: opening it is held in the header (BROWSER_OPEN_PANEL_CHANNEL)",
  "browser.setBounds": "the browser panel, which a browser cannot open: opening it is held in the header (BROWSER_OPEN_PANEL_CHANNEL)",
  "browser.stop": "the browser panel, which a browser cannot open: opening it is held in the header (BROWSER_OPEN_PANEL_CHANNEL)",
  "cancelPagePick": "Point at a page needs the page running, served from the computer (pageUrlFor), so a browser never shows the button",
  "catchUpTerminal": "automatic: it reads the computer's terminal sessions; refused, it is caught and quiet",
  "combineTuneEdits": "a kept tune's Combine says the refusal (failureSaid); a tune itself starts only on the computer",
  "deskTakeBack": "the computer's own window while a browser has Locust (ElsewhereApp): never drawn in a browser",
  "deskWhere": "the computer's own window while a browser has Locust (ElsewhereApp): never drawn in a browser",
  "dismissBackground": "background turns are not offered in a browser (BACKGROUND_START_CHANNEL); the panel's buttons say the refusal (failureSaid)",
  "importRoutine": "a template's import dialog says the refusal in its own words (its error is the refusal's message)",
  "installedEditors": "automatic at start: refused, there are no editors, so the right-click menu offers none",
  "locustLinks.take": "automatic: a Locust link opens on the computer; refused, it is caught and quiet",
  "locustMcp.restartClaudeApp": "Let your other AI apps use Locust is drawn as OnTheComputerOnly in a browser",
  "locustMcp.setClaudeApp": "Let your other AI apps use Locust is drawn as OnTheComputerOnly in a browser",
  "newRoutineHook": "a called link is read and made only on the computer (RoutineDialog checks `here`)",
  "openBackground": "background turns are not offered in a browser (BACKGROUND_START_CHANNEL); the panel's buttons say the refusal (failureSaid)",
  "openFile": "the right-click menu offers no editor and no default app in a browser (fileMenuChoices, inBrowser)",
  "ownCommands.run": "a browser's ! command is refused before it leaves, and its output says so (failureSaid)",
  "ownCommands.stop": "a command of yours never starts from a browser, so none runs there to stop",
  "phone.anywhere": "Your phone is drawn as OnTheComputerOnly in a browser",
  "pickInPage": "Point at a page needs the page running, served from the computer (pageUrlFor), so a browser never shows the button",
  "pickRestoredFolder": "on the restored folders' card, which a browser never gets (restoredFolders)",
  "readRoutineHook": "a called link is read and made only on the computer (RoutineDialog checks `here`)",
  "remoteMcp.decide": "an app from anywhere is let in only on the computer (0.841): refused, its card reads no asks, so none shows in a browser",
  "remoteMcp.settings": "an app from anywhere is let in only on the computer (0.841): refused, its card reads no asks, so none shows in a browser",
  "restoredFolders": "automatic after a restore on the computer; refused, a browser never gets the restored folders' card",
  "setUpBackground": "background turns are not offered in a browser (BACKGROUND_START_CHANNEL); the panel's buttons say the refusal (failureSaid)",
  "stopBackground": "background turns are not offered in a browser (BACKGROUND_START_CHANNEL); the panel's buttons say the refusal (failureSaid)",
  "takeLastProfileRestore": "automatic at start: a restore happens on the computer; refused, nothing is said",
  "talkShortcut.pending": "automatic: Talk's keys are the computer's; refused, it is caught and quiet",
  "terminal.close": "the terminal panel, which a browser cannot open: opening it is held in the header (TERMINAL_CREATE)",
  "terminal.copy": "the terminal panel, which a browser cannot open: opening it is held in the header (TERMINAL_CREATE)",
  "terminal.input": "the terminal panel, which a browser cannot open: opening it is held in the header (TERMINAL_CREATE)",
  "terminal.openLink": "the terminal panel, which a browser cannot open: opening it is held in the header (TERMINAL_CREATE)",
  "terminal.resize": "the terminal panel, which a browser cannot open: opening it is held in the header (TERMINAL_CREATE)",
  "terminal.shells": "the terminal panel, which a browser cannot open: opening it is held in the header (TERMINAL_CREATE)",
  "voiceCall.answer": "it answers a connector during a call, and Talk is held in a browser",
  "writeClipboard": "the right-click menu's copies use the browser's own clipboard first (inABrowser)",
}

/**
 * Where the window makes each computer-only call: file, call, how many times. A call added anywhere fails until it is
 * decided -- hold it (onTheComputer or OnTheComputer), leave it out of a browser, make it work there, or give QUIET
 * its reason -- and its line is added here. openLink is not here: a browser opens links itself (desk-remote.ts).
 */
const CALLED = `
App.tsx addTeamFromCard 1
App.tsx allowCloudComputers 1
App.tsx applyClaudeCloud 1
App.tsx applyCloudTask 1
App.tsx applyCloudTurn 1
App.tsx applyRestoredFolders 1
App.tsx backupProfile 1
App.tsx bringClaudeCloudHome 1
App.tsx browser.closePanel 2
App.tsx browser.openPanel 1
App.tsx catchUpTerminal 1
App.tsx chooseTeammateFolder 1
App.tsx chooseWorkspace 1
App.tsx clearMemories 1
App.tsx confirmWorkspace 1
App.tsx continueClaudeCloudInTerminal 1
App.tsx discardCloudTurn 1
App.tsx dismissBackground 1
App.tsx draftTune 1
App.tsx emptyTrash 1
App.tsx exportRoutine 1
App.tsx forgetClaudeCloud 1
App.tsx importRoutine 1
App.tsx importSession 1
App.tsx installRuntime 1
App.tsx installUpdate 1
App.tsx installedEditors 1
App.tsx locustLinks.take 1
App.tsx noteTuneLesson 1
App.tsx openBackground 1
App.tsx openFile 1
App.tsx openInTerminal 1
App.tsx openWorkspaceFolder 1
App.tsx ownCommands.allow 4
App.tsx ownCommands.run 4
App.tsx ownCommands.stop 1
App.tsx pageUrlFor 1
App.tsx pickProfileFolder 1
App.tsx pickRestoredFolder 1
App.tsx previewProfileRestore 1
App.tsx previewRoutineImport 1
App.tsx pruneMissions 2
App.tsx pullRequestCloudTurn 1
App.tsx removeTeammateSkill 1
App.tsx removeTuneLesson 1
App.tsx removeWorktree 1
App.tsx restoreProfile 1
App.tsx restoredFolders 1
App.tsx revealFile 2
App.tsx ruleFromApprovalCard 1
App.tsx saveCopy 1
App.tsx scout.turnOn 1
App.tsx sendClaudeCloud 1
App.tsx setKeepRunning 1
App.tsx setLoginItem 1
App.tsx setRuntimeUpdates 1
App.tsx setStayAwake 1
App.tsx setTeammateConnectors 1
App.tsx setTeammateOwnBrowser 1
App.tsx setUpBackground 1
App.tsx setUpCloudComputer 1
App.tsx setUpdateLane 1
App.tsx signInCloudAgent 1
App.tsx startBackground 1
App.tsx startClaudeCloud 1
App.tsx startCloudTask 1
App.tsx stopBackground 1
App.tsx stopCloudSignIn 1
App.tsx takeLastProfileRestore 1
App.tsx talkShortcut.pending 1
App.tsx updateRuntimeNow 1
App.tsx useTuneSkill 1
App.tsx writeClipboard 1
App.tsx writeWorkspaceSettings 14
ElsewhereApp.tsx deskTakeBack 1
ElsewhereApp.tsx deskWhere 1
components/AcpAgentsSettings.tsx acpAgents.remove 1
components/ActivityCard.tsx revealFile 1
components/AddConnector.tsx addConnector 1
components/AddConnector.tsx removeConnector 1
components/BrowserPanel.tsx browser.cancelPoint 7
components/BrowserPanel.tsx browser.navigate 5
components/BrowserPanel.tsx browser.reload 5
components/BrowserPanel.tsx browser.setBounds 11
components/BrowserPanel.tsx browser.stop 5
components/ChangesPanel.tsx walkThroughChange 1
components/ChatCommands.tsx ownCommands.openFolder 1
components/CloudComputers.tsx allowCloudComputers 1
components/CloudComputers.tsx openCloudComputerTerminal 1
components/CloudComputers.tsx removeCloudComputer 1
components/CloudComputers.tsx signInCloudAgent 1
components/CloudComputers.tsx signOutCloudAgent 1
components/CloudComputers.tsx stopCloudComputer 1
components/CloudComputers.tsx stopCloudSignIn 1
components/Composer.tsx attachFiles 1
components/DocPreview.tsx pageUrlFor 1
components/FeedbackDialog.tsx emailFeedback 1
components/FeedbackDialog.tsx saveFeedbackFile 1
components/FeedbackDialog.tsx sendFeedback 1
components/FileViewer.tsx cancelPagePick 2
components/FileViewer.tsx pickInPage 1
components/GitHubAccount.tsx githubInstall 1
components/GitHubAccount.tsx githubSignIn 1
components/GitHubAccount.tsx githubSignInCancel 1
components/GitHubAccount.tsx githubUpdate 1
components/Inspector.tsx revealFile 1
components/KeptInstructions.tsx combineTuneEdits 1
components/KeptInstructions.tsx noteTuneLesson 1
components/KeptInstructions.tsx removeTuneLesson 1
components/KeptInstructions.tsx useTuneSkill 1
components/KeyCard.tsx teammateKeys.list 4
components/KeyCard.tsx teammateKeys.set 2
components/LocustMcpSettings.tsx locustMcp.restartClaudeApp 1
components/LocustMcpSettings.tsx locustMcp.setClaudeApp 1
components/OwnModels.tsx addOwnModel 1
components/OwnModels.tsx removeOwnModel 1
components/OwnModels.tsx setOwnModelChatOnly 1
components/OwnModels.tsx testOwnModel 2
components/PetPicks.tsx addPet 1
components/PetPicks.tsx removePet 1
components/PhoneSettings.tsx phone.anywhere 1
components/PhoneSettings.tsx phone.settings 1
components/RemoteConnectCard.tsx remoteMcp.decide 1
components/RemoteConnectCard.tsx remoteMcp.settings 1
components/RemoteControlSetting.tsx setRemoteControl 1
components/RemoteMcpSettings.tsx remoteMcp.settings 1
components/ReplyPage.tsx replyPageUrl 1
components/RoutineDialog.tsx newRoutineHook 1
components/RoutineDialog.tsx readRoutineHook 1
components/RoutineInputs.tsx chooseRoutineFolder 1
components/SaveRecordDialog.tsx saveMissionRecord 1
components/SavedApprovalRules.tsx removeAllApprovalRules 1
components/SavedApprovalRules.tsx removeApprovalRule 1
components/ScoutSetting.tsx scout.turnOn 1
components/Screens.tsx revealDiagnostics 1
components/SignInButton.tsx signInRuntime 1
components/SkillsFromGitHub.tsx installSkillLibrary 1
components/SkillsFromGitHub.tsx removeSkillLibrary 1
components/TalkShortcutSetting.tsx talkShortcut.set 1
components/TalkVoice.tsx voiceCall.setVoice 1
components/TeamCard.tsx saveTeamCard 1
components/TerminalPanel.tsx terminal.close 4
components/TerminalPanel.tsx terminal.copy 3
components/TerminalPanel.tsx terminal.create 3
components/TerminalPanel.tsx terminal.input 3
components/TerminalPanel.tsx terminal.openLink 3
components/TerminalPanel.tsx terminal.resize 3
components/TerminalPanel.tsx terminal.shells 3
components/Thread.tsx revealFile 4
components/VoiceButton.tsx voice.allowOpenAI 1
components/VoiceButton.tsx voice.download 1
components/VoiceCall.tsx voiceCall.answer 3
components/VoiceCall.tsx voiceCall.start 1
components/VoiceSettings.tsx voice.saveSettings 1
components/YourAccountSettings.tsx phone.account 2
`.trim().split('\n')

const made = channels()
const computerOnly = new Set(WINDOW_ONLY)
const files = windowFiles()
const names = channelNames()

describe('every call a browser is refused', () => {
  const sites: string[] = []
  const called = new Set<string>()
  const decided = new Set<string>()
  for (const [file, text] of files) {
    for (const opening of text.matchAll(/onTheComputer\(|channel=\{/g)) {
      for (const name of bracketed(text, opening.index + opening[0].length - 1).matchAll(/\b[A-Z][A-Z0-9_]{2,}\b/g)) {
        const channel = names.get(name[0])
        if (channel !== undefined) decided.add(channel)
      }
    }
    const calls = callsIn(text)
    for (const [path, channel] of made) {
      if (!computerOnly.has(channel) || path === 'openLink') continue
      const times = calls.get(path) ?? 0
      if (times === 0) continue
      sites.push(`${file} ${path} ${String(times)}`)
      called.add(path)
    }
  }

  it('reads every call off the API, and finds the window making them', () => {
    expect(made.size).toBeGreaterThan(300)
    expect(made.get('revealFile')).toBe('workspace:reveal')
    expect(made.get('voice.transcribe')).toBe('voice:transcribe')
    expect(called.size).toBeGreaterThan(80)
  })

  it('is decided where the window makes it: held, left out, made to work, or quiet for a reason written here', () => {
    const undecided = [...called].filter((path) => !decided.has(made.get(path)!) && QUIET[path] === undefined).sort()
    expect(undecided, 'Decide what this does in Locust in a browser: hold it with onTheComputer, leave it out, make it work, or say in QUIET why it is quiet').toEqual([])
    const stale = Object.keys(QUIET).filter((path) => !called.has(path) || decided.has(made.get(path) ?? '')).sort()
    expect(stale, 'No longer called, or now decided where it is made: take it out of QUIET').toEqual([])
    for (const why of Object.values(QUIET)) expect(why.length).toBeGreaterThan(30)
  })

  it('is made only where it was looked at: a new call to one fails until it is decided and listed', () => {
    expect(sites.sort(), 'A computer-only call was added, moved or removed: decide what it does in a browser, then update CALLED').toEqual([...CALLED].sort())
  })
})
