/**
 * THE FOLDER A CONVERSATION WORKS IN, WHEN AN APP NAMED IT (0.869; carried on 0.872).
 *
 * An app may name a folder for a conversation it starts through Locust's MCP door; the turn records it as
 * `startedBy.folder`. Every later turn of that conversation stands in the same folder: an app's follow-up, and a turn
 * Locust carries on with another agent when the first one's plan ran out. That carried turn records the folder too, so
 * the turns after it keep finding it. Before 0.872 a carried turn stood in the teammate's own folder (found 9 Nov: a
 * review in native-review/m1 carried on to Copilot worked in now-coding and wrote its attachment there).
 */
export function conversationFolder(startedBy: { readonly kind: string; readonly folder?: string } | undefined): string | undefined {
  if (startedBy === undefined || (startedBy.kind !== 'mcp' && startedBy.kind !== 'carry-on')) return undefined
  return typeof startedBy.folder === 'string' && startedBy.folder.length > 0 ? startedBy.folder : undefined
}
