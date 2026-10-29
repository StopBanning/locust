/**
 * OTHER APPS, FROM ANYWHERE (0.841, main/remote-mcp.ts): Claude on the web, Claude Duet and other apps that take a
 * remote MCP server, using the team here through Locust's relay. What Settings and the window's card are told.
 */
export const REMOTE_MCP_GET = 'remote-mcp:get'
export const REMOTE_MCP_SET = 'remote-mcp:set'
export const REMOTE_MCP_REMOVE = 'remote-mcp:remove'
export const REMOTE_MCP_DECIDE = 'remote-mcp:decide'
export const REMOTE_MCP_CHANGED = 'remote-mcp:changed'

/** An app with keys: what it called itself, where it sends the person back, and when. */
export interface RemoteMcpApp {
  readonly id: string
  readonly name: string
  readonly host: string
  readonly added: string
  readonly used?: string
}

/** A sign-in waiting on the person's card. */
export interface RemoteMcpAsking {
  readonly request: string
  readonly name: string
  /** Where the app sends the person back, as a host: the app is who that is. */
  readonly host: string
  /** Six digits the sign-in page shows too. */
  readonly match: string
  readonly until: number
}

export interface RemoteMcpState {
  readonly enabled: boolean
  /** The address an app is given, while this and From anywhere are both on. */
  readonly address?: string
  readonly apps: readonly RemoteMcpApp[]
  readonly asking: readonly RemoteMcpAsking[]
  readonly message?: string
}

export interface RemoteMcpApi {
  settings(): Promise<RemoteMcpState>
  setEnabled(enabled: boolean): Promise<RemoteMcpState>
  remove(id: string): Promise<RemoteMcpState>
  decide(request: string, allow: boolean): Promise<RemoteMcpState>
  onChanged(listener: (state: RemoteMcpState) => void): () => void
}
