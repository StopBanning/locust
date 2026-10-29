export const LOCUST_MCP_GET = 'locust-mcp:get'
export const LOCUST_MCP_SET = 'locust-mcp:set'
export const LOCUST_MCP_SET_OWN_MODE = 'locust-mcp:set-own-mode'
export const LOCUST_MCP_SET_EDITOR_ANSWERS = 'locust-mcp:set-editor-answers'
export const LOCUST_MCP_SET_CONVERSATIONS = 'locust-mcp:set-conversations'
/** Locust in the Claude app's tools (0.740): read where it stands, and add or remove it in one press. */
export const LOCUST_MCP_CLAUDE_APP = 'locust-mcp:claude-app'
export const LOCUST_MCP_CLAUDE_APP_SET = 'locust-mcp:claude-app-set'
export const LOCUST_MCP_CLAUDE_APP_RESTART = 'locust-mcp:claude-app-restart'
export type ClaudeAppConnection =
  | { readonly kind: 'no-app' }
  /** `restart`: Locust was removed while Claude was open, and Claude keeps it until it restarts. */
  | { readonly kind: 'not-added'; readonly restart?: boolean }
  /**
   * `restart`: Claude was open when Locust was added, and reads its settings only when it starts.
   * `stale`: the entry runs a Locust that is not this one, or files that are gone (0.741) -- Update fixes it.
   */
  | { readonly kind: 'added'; readonly restart?: boolean; readonly stale?: boolean }
  /** A different add-on of the person's is named "locust": left alone, never changed or removed (0.741). */
  | { readonly kind: 'taken'; readonly message: string }
  | { readonly kind: 'unreadable'; readonly message: string }
export interface ClaudeAppResult {
  readonly connection: ClaudeAppConnection
  /** The local server's state after the press: Add turns it on. */
  readonly state?: LocustMcpState
  readonly message?: string
}
export interface LocustMcpState {
  readonly enabled: boolean
  /**
   * Off (the default): every turn another app starts is Ask, read only. On:
   * the teammate's own saved mode, with approvals answered only in Locust's
   * window and Auto only where Settings allows it (0.703).
   */
  readonly ownMode: boolean
  /**
   * Off (the default): a card for a turn an editor started through the locust command waits in this window, and
   * only the person answers it. On: that editor may approve or deny the cards of the turns it started (0.790).
   */
  readonly editorAnswers?: boolean
  /**
   * Off (the default): an app sees only the conversations it started. On: it may list, read, continue, stop and carry
   * on the person's own conversations in the open folder too, by the same rules as its own (0.842).
   */
  readonly conversations?: boolean
  /** An editor has connected to this Locust at least once, so Settings shows the switch above. */
  readonly editorSeen?: boolean
  readonly claudeCommand?: string
  readonly codexConfig?: string
  /** OpenClaw's `openclaw mcp add` line (0.754): its chat apps reach the team through it. */
  readonly openclawCommand?: string
  readonly message?: string
}
export interface LocustMcpApi {
  settings(): Promise<LocustMcpState>
  setEnabled(enabled: boolean): Promise<LocustMcpState>
  setOwnMode(ownMode: boolean): Promise<LocustMcpState>
  setEditorAnswers?(editorAnswers: boolean): Promise<LocustMcpState>
  setConversations?(conversations: boolean): Promise<LocustMcpState>
  claudeApp?(): Promise<ClaudeAppConnection>
  setClaudeApp?(added: boolean): Promise<ClaudeAppResult>
  /** Close the Claude app and open it again, so it reads Locust's entry. */
  restartClaudeApp?(): Promise<ClaudeAppResult>
}
