import type { Server } from 'node:http'

export interface FakeTelegramUser { readonly id: number; readonly is_bot: boolean; readonly first_name: string; readonly username?: string }
export interface FakeSent {
  readonly message_id: number
  readonly chat: { readonly id: number }
  readonly date: number
  /** What the person reads: the HTML's text. */
  readonly text: string
  html: string
  readonly parse_mode?: string
  reply_markup?: { readonly inline_keyboard: readonly (readonly { readonly text: string; readonly callback_data: string }[])[] }
  readonly reply_to?: number
}
export interface FakeTelegramState {
  webhook: string
  updates: unknown[]
  readonly sent: FakeSent[]
  readonly edits: { readonly message_id: number; readonly html: string; readonly reply_markup?: unknown }[]
  readonly answers: { readonly id: string; readonly text?: string }[]
  readonly actions: { readonly chat_id: number; readonly action: string }[]
  readonly commands: { readonly command: string; readonly description: string }[][]
  readonly refused: { readonly method: string; readonly text: string; readonly problem: string }[]
  polls: number
  confirmed: number
}
export interface FakeTelegram {
  readonly token: string
  readonly bot: FakeTelegramUser & { readonly username: string }
  readonly person: FakeTelegramUser
  readonly state: FakeTelegramState
  readonly fetch: typeof fetch
  say(text: string | undefined, more?: { readonly from?: FakeTelegramUser; readonly chatId?: number; readonly chatType?: string; readonly date?: number; readonly replyTo?: number }): number
  press(messageId: number, data: string, more?: { readonly from?: FakeTelegramUser; readonly chatId?: number }): void
  until<T>(test: (state: FakeTelegramState) => T | undefined | false, ms?: number, what?: string): Promise<T>
  read(): string[]
  listen(port?: number): Promise<Server>
}
export function htmlProblem(html: string): string | undefined
export function fakeTelegram(options?: {
  readonly token?: string
  readonly botName?: string
  readonly username?: string
  readonly personId?: number
  readonly personName?: string
  readonly webhook?: string
  readonly now?: () => number
  readonly maxWaitSeconds?: number
}): FakeTelegram
