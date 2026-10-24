import type { Server } from 'node:http'

export interface FakeSlackUser { readonly id: string; readonly name: string; readonly real_name?: string; readonly profile?: { readonly display_name?: string; readonly real_name?: string } }
export interface FakeSlackButton { readonly type: 'button'; readonly text: { readonly type: 'plain_text'; readonly text: string }; readonly action_id: string; readonly value?: string; readonly style?: 'primary' | 'danger' }
export interface FakeSlackBlock { readonly type: string; readonly text?: { readonly type: string; readonly text: string }; readonly elements?: readonly FakeSlackButton[] }
export interface FakeSlackMessage {
  readonly channel: string
  readonly ts: string
  text: string
  blocks: readonly FakeSlackBlock[]
  readonly thread_ts?: string
  readonly user: string
  readonly bot_id?: string
}
export interface FakeSlackEnvelope { readonly envelope_id: string; readonly type: string; readonly payload: unknown }
export interface FakeSlackState {
  socketBase: string
  socketMode: boolean
  readonly messages: Map<string, FakeSlackMessage>
  readonly sent: FakeSlackMessage[]
  readonly edits: { readonly channel: string; readonly ts: string; readonly text: string; readonly blocks: readonly FakeSlackBlock[] }[]
  readonly reactions: { readonly channel: string; readonly ts: string; readonly name: string; on: boolean }[]
  readonly envelopes: FakeSlackEnvelope[]
  readonly acked: Set<string>
  readonly refused: { readonly method: string; readonly problem: string }[]
  hellos: number
  connections: number
  opened: number
}
export interface FakeSlack {
  readonly botToken: string
  readonly appToken: string
  readonly team: { readonly id: string; readonly name: string; readonly domain: string }
  readonly app: { readonly id: string }
  readonly bot: { readonly user_id: string; readonly bot_id: string; readonly name: string }
  readonly person: FakeSlackUser
  readonly personChannel: string
  readonly state: FakeSlackState
  readonly WebSocket: typeof WebSocket
  readonly fetch: typeof fetch
  say(text: string | undefined, more?: { readonly from?: FakeSlackUser; readonly channel?: string; readonly channelType?: string; readonly thread?: string; readonly at?: number; readonly subtype?: string; readonly botId?: string }): string
  again(envelope: FakeSlackEnvelope): void
  press(ts: string, actionId: string, more?: { readonly from?: FakeSlackUser; readonly channel?: string }): FakeSlackEnvelope
  disconnect(reason?: string): void
  drop(): void
  read(channel?: string): string[]
  until<T>(test: (state: FakeSlackState) => T | undefined | false, ms?: number, what?: string): Promise<T>
  listen(port?: number): Promise<Server>
}
export function fakeSlack(options?: {
  readonly botToken?: string
  readonly appToken?: string
  readonly socketBase?: string
  readonly socketMode?: boolean
  readonly now?: () => number
}): FakeSlack
