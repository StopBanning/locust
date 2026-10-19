/** The person's interactive shells. No commands or output enter a mission or a model. */
export const TERMINAL_SHELLS = 'terminal:shells'
export const TERMINAL_CREATE = 'terminal:create'
export const TERMINAL_INPUT = 'terminal:input'
export const TERMINAL_RESIZE = 'terminal:resize'
export const TERMINAL_CLOSE = 'terminal:close'
export const TERMINAL_EVENT = 'terminal:event'
export const TERMINAL_LINK = 'terminal:link'
export const TERMINAL_COPY = 'terminal:copy'

export interface TerminalContext { readonly missionId?: string; readonly teammateId?: string }
export interface TerminalShell { readonly id: string; readonly name: string }
export interface TerminalTab { readonly id: string; readonly shell: string; readonly folder: string }
export type TerminalResult<T> = { readonly ok: true; readonly data: T } | { readonly ok: false; readonly message: string }
export type TerminalEvent = { readonly id: string; readonly kind: 'data'; readonly data: string } | { readonly id: string; readonly kind: 'exit'; readonly exitCode: number }
export interface TerminalApi {
  shells(): Promise<TerminalResult<readonly TerminalShell[]>>
  create(context: TerminalContext, shell?: string): Promise<TerminalResult<TerminalTab>>
  input(id: string, data: string): Promise<void>
  resize(id: string, cols: number, rows: number): Promise<void>
  close(id: string): Promise<void>
  openLink(url: string): Promise<TerminalResult<void>>
  copy(text: string): Promise<TerminalResult<void>>
  onEvent(listener: (event: TerminalEvent) => void): () => void
}

/** IPC bounds count UTF-16 units; a PTY encodes each write as UTF-8, so never split a surrogate pair. */
export function* terminalInputChunks(text: string): Generator<string> {
  for (let start = 0; start < text.length;) {
    let end = Math.min(start + 64 * 1024, text.length)
    const last = text.charCodeAt(end - 1)
    const next = text.charCodeAt(end)
    if (end < text.length && last >= 0xd800 && last <= 0xdbff && next >= 0xdc00 && next <= 0xdfff) end -= 1
    yield text.slice(start, end)
    start = end
  }
}
