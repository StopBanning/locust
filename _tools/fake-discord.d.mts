import type { Server } from 'node:http'

export interface FakeDiscordUser { readonly id: string; readonly username: string; readonly global_name?: string | null; readonly discriminator?: string; readonly bot?: boolean }
export interface FakeDiscordButton { readonly type: 2; readonly style: number; readonly label: string; readonly custom_id: string }
export interface FakeDiscordRow { readonly type: 1; readonly components: readonly FakeDiscordButton[] }
export interface FakeDiscordMessage {
  readonly id: string
  readonly channel_id: string
  readonly author: FakeDiscordUser
  content: string
  components: readonly FakeDiscordRow[]
  readonly timestamp: string
  readonly flags?: number
  readonly allowed_mentions?: { readonly parse: readonly string[] }
  readonly message_reference?: { readonly message_id: string }
  /** How it was made: a channel message, or an interaction's answer. */
  readonly via: 'channel' | 'interaction'
  loading?: boolean
}
export interface FakeDiscordState {
  gateway: string
  readonly messages: Map<string, FakeDiscordMessage>
  readonly sent: (FakeDiscordMessage & { readonly reply_to?: string })[]
  readonly edits: { readonly id: string; readonly channel_id: string; readonly content: string; readonly components: readonly FakeDiscordRow[] }[]
  readonly responses: { readonly id: string; readonly interaction: string; readonly content: string; readonly components: readonly FakeDiscordRow[] }[]
  readonly callbacks: { readonly id: string; readonly kind: number; readonly type: number }[]
  readonly typing: { readonly channel_id: string; readonly at: number }[]
  readonly commands: { readonly name: string; readonly description: string; readonly contexts?: readonly number[]; readonly options?: readonly { readonly name: string; readonly type: number; readonly description: string }[] }[][]
  readonly refused: { readonly path: string; readonly problem: string }[]
  identified: number
  resumed: number
  heartbeats: number
  readonly intents: number[]
  connections: number
}
export interface FakeDiscordInteraction { readonly id: string; readonly token: string; readonly type: number }
export interface FakeDiscord {
  readonly token: string
  readonly bot: FakeDiscordUser
  readonly person: FakeDiscordUser
  readonly application: { readonly id: string; readonly name: string }
  readonly personChannel: string
  readonly state: FakeDiscordState
  readonly WebSocket: typeof WebSocket
  readonly fetch: typeof fetch
  say(text: string | undefined, more?: { readonly from?: FakeDiscordUser; readonly channelId?: string; readonly guildId?: string; readonly replyTo?: string; readonly timestamp?: string }): string
  command(name: string, words?: string, more?: { readonly from?: FakeDiscordUser; readonly channelId?: string; readonly guildId?: string }): FakeDiscordInteraction
  press(messageId: string, customId: string, more?: { readonly from?: FakeDiscordUser; readonly channelId?: string; readonly guildId?: string }): FakeDiscordInteraction
  reconnect(): void
  drop(code?: number): void
  read(channel?: string): string[]
  until<T>(test: (state: FakeDiscordState) => T | undefined | false, ms?: number, what?: string): Promise<T>
  listen(port?: number): Promise<Server>
}
export function fakeDiscord(options?: {
  readonly token?: string
  readonly appName?: string
  readonly username?: string
  readonly gateway?: string
  readonly channels?: readonly string[]
  readonly blocked?: boolean
  readonly heartbeatMs?: number
  readonly now?: () => number
}): FakeDiscord
