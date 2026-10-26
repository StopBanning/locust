export type ShortcutGroup = 'Conversations' | 'The message box' | 'Panes' | 'Menus'

export interface ShortcutRow {
  readonly group: ShortcutGroup
  readonly keys: string
  readonly does: string
  /** A string in the handler's source, so this list cannot name a key the code dropped. */
  readonly mark: string
  /** Path from this folder's test, where the mark is found. */
  readonly file: string
}

/**
 * Every shortcut a person can press, in the words the sheet uses.
 *
 * Ctrl+W, Ctrl+R and Alt+F4 are not here: the window, a reload, and Windows
 * already use them. Ctrl+Shift+I is DevTools on Chromium's menu; the main
 * process holds that chord for the model menu, and F12 still opens DevTools.
 */
export const SHORTCUTS: readonly ShortcutRow[] = [
  { group: 'Conversations', keys: 'Ctrl+F', does: 'Find in this conversation', mark: 'setFindRequest((count) => count + 1)', file: './App.tsx' },
  { group: 'Conversations', keys: 'Ctrl+K', does: 'Open the command palette', mark: 'setPaletteOpen((open) => !open)', file: './App.tsx' },
  { group: 'Conversations', keys: 'Ctrl+I', does: 'Open the inspector', mark: 'setInspectorOpen((open) => !open)', file: './App.tsx' },
  { group: 'Conversations', keys: 'Alt+Up / Alt+Down', does: 'Previous or next conversation', mark: 'stepConversation(drawn', file: './App.tsx' },
  { group: 'Conversations', keys: 'Alt+Shift+Up / Alt+Shift+Down', does: 'A conversation that is waiting for you', mark: 'event.shiftKey ? (id) => keysOf(id)', file: './App.tsx' },
  { group: 'Conversations', keys: 'Ctrl+1', does: 'Home', mark: "setScreen('missions')", file: './App.tsx' },
  { group: 'Conversations', keys: 'Ctrl+2', does: 'Teammates', mark: "setScreen('teammates')", file: './App.tsx' },
  { group: 'Conversations', keys: 'Ctrl+3', does: 'Settings', mark: "setScreen('settings')", file: './App.tsx' },
  { group: 'Conversations', keys: 'Ctrl+4', does: 'Rooms', mark: "setScreen('rooms')", file: './App.tsx' },
  { group: 'Conversations', keys: 'Ctrl+5', does: 'Memory', mark: "setScreen('memory')", file: './App.tsx' },
  { group: 'Conversations', keys: 'Ctrl+`', does: 'Terminal', mark: 'isTerminalToggle', file: './App.tsx' },
  { group: 'Conversations', keys: 'Ctrl+Alt+Up / Ctrl+Alt+Down', does: 'Move among your own messages', mark: 'promptRailStep', file: './components/PromptRail.tsx' },
  { group: 'Conversations', keys: 'Ctrl+O', does: 'Normal or Verbose steps', mark: 'toggleTranscriptView', file: './App.tsx' },
  { group: 'The message box', keys: 'Enter', does: 'Send', mark: "keyEvent.key === 'Enter' && !keyEvent.shiftKey", file: './components/Composer.tsx' },
  { group: 'The message box', keys: 'Shift+Enter', does: 'A new line', mark: 'shortcut:newline', file: './components/Composer.tsx' },
  { group: 'The message box', keys: 'Ctrl+Enter', does: 'Send now, while a reply is running', mark: 'sendNow.current', file: './components/Composer.tsx' },
  { group: 'The message box', keys: 'Shift+Tab', does: 'Next permission mode', mark: 'shiftTabMode(', file: './components/Composer.tsx' },
  { group: 'The message box', keys: 'Escape', does: 'Stop the reply, when the box is focused and no menu is open', mark: "keyEvent.key === 'Escape' && running", file: './components/Composer.tsx' },
  { group: 'The message box', keys: 'Up / Down', does: 'Recall a sent message from an empty box', mark: 'setRecallAt', file: './components/Composer.tsx' },
  { group: 'Panes', keys: 'Ctrl+/', does: 'Keyboard shortcuts', mark: 'setShortcutsOpen', file: './App.tsx' },
  { group: 'Panes', keys: '+ / − / 0 / 1', does: 'Zoom an image, fit it, or its actual size', mark: "event.key === '+' || event.key === '='", file: './components/ZoomableImage.tsx' },
  { group: 'Panes', keys: 'Escape', does: 'Close a dialog', mark: 'Escape closes it, wherever focus is', file: './useModal.ts' },
  { group: 'Menus', keys: 'Ctrl+Shift+M', does: 'Permission mode', mark: "which === 'mode'", file: './components/Composer.tsx' },
  { group: 'Menus', keys: 'Ctrl+Shift+I', does: 'Model', mark: "which === 'model'", file: './components/Composer.tsx' },
  { group: 'Menus', keys: 'Ctrl+Shift+E', does: 'Effort', mark: "which === 'effort'", file: './components/Composer.tsx' },
  { group: 'Menus', keys: '1–9', does: 'Pick that row in an open menu', mark: 'menuRowFromDigit', file: './components/Select.tsx' },
  { group: 'Menus', keys: '1–9', does: 'Answer a question card', mark: 'numberKeyAnswer', file: './components/DecisionCard.tsx' },
  { group: 'Menus', keys: 'Ctrl+Alt+Space', does: 'Talk, from any app', mark: 'DEFAULT_TALK_SHORTCUT', file: '../../shared/talk-shortcut.ts' }
]

export function shortcutGroups(): readonly { readonly name: ShortcutGroup; readonly rows: readonly ShortcutRow[] }[] {
  const names: readonly ShortcutGroup[] = ['Conversations', 'The message box', 'Panes', 'Menus']
  return names.map((name) => ({ name, rows: SHORTCUTS.filter((row) => row.group === name) }))
}
