// A dependency-free stdio MCP server, spawned by the person's other AI app.
// It never launches Locust, writes another app's config, or follows redirects.
import { readFile } from 'node:fs/promises'
import { request } from 'node:http'

const NOT_RUNNING = 'Locust is not running. Open it and try again.'
const result = (text, isError = false) => ({ content: [{ type: 'text', text }], ...(isError ? { isError: true } : {}) })
const schema = (properties = {}, required = []) => ({ type: 'object', properties, required, additionalProperties: false })
const string = { type: 'string', minLength: 1, maxLength: 200 }
const message = { type: 'string', minLength: 1, maxLength: 8000 }
const effort = { type: 'string', minLength: 1, maxLength: 40 }
const TOOLS = [
  { name: 'list_teammates', description: 'List Locust teammates: id, name, role, runtime, model, whether busy, and runs_in: the mode a turn started from here would use. Read only.', inputSchema: schema() },
  { name: 'start_conversation', description: 'Ask a Locust teammate by name or id. Ask mode (read only), unless the person has turned on "Use each teammate\'s own mode" in Locust; then the mode the teammate is set to there, and any approval is answered by the person in Locust\'s window, never from here. Optionally a model and effort for this turn alone, from list_models; left out, the teammate\'s own. The reply says the mode used. No automatic teammate handoffs. Shares remain visible in Locust. API/account usage and Locust monthly limits apply. Returns a conversation id; use read_reply to poll.', inputSchema: schema({ teammate: string, message, model: string, effort }, ['teammate', 'message']) },
  { name: 'send_message', description: 'Continue a conversation started by this server, in the same kind of mode as start_conversation, with no automatic teammate handoffs. Optionally a model and effort for this turn, as start_conversation takes them. No message is queued while it is working; poll read_reply before sending.', inputSchema: schema({ conversation_id: string, message, model: string, effort }, ['conversation_id', 'message']) },
  { name: 'read_reply', description: 'Read the newest finished answer, current activity while still working (including a wait for the person to approve an action in Locust), or Locust\'s own failure words. Read only.', inputSchema: schema({ conversation_id: string }, ['conversation_id']) },
  { name: 'list_models', description: 'List the models a teammate\'s AI agent offers in this Locust, with the efforts each takes, and the model and effort it runs on now. Read only.', inputSchema: schema({ teammate: string }, ['teammate']) },
  { name: 'list_background_runs', description: 'List Claude turns running in the background. Read only; never starts or stops them.', inputSchema: schema() },
  { name: 'list_routines', description: 'List Locust routines (saved multi-step workflows): each step\'s teammate and mode, the inputs a run asks for, whether it changes files, and whether run_routine can start it from here. Read only.', inputSchema: schema() },
  { name: 'run_routine', description: 'Run a Locust routine by id or name, with values for its inputs. A routine that changes files runs only if the person has turned on "Use each teammate\'s own mode" in Locust; any approval is answered by the person in Locust\'s window, never from here. A routine that asks for a folder runs only from Locust. API/account usage and Locust monthly limits apply. Returns the routine id; poll routine_status.', inputSchema: schema({ routine: string, values: { type: 'object', additionalProperties: { type: 'string', maxLength: 4000 }, maxProperties: 12 } }, ['routine']) },
  { name: 'routine_status', description: 'The step a routine started by run_routine is on, a wait for the person in Locust, or, once it ends, its last answer. Read only.', inputSchema: schema({ routine: string }, ['routine']) }
]
/*
 * Each tool that only reads says so (0.742 for a voice call's copy; 0.754 for every app): a client may run a tool
 * marked read only without asking first -- Codex does -- and OpenClaw's doctor said of the unmarked list "calls require
 * approval". Starting or continuing a conversation, and running a routine, are not marked, and still ask.
 */
const READS = new Set(['list_teammates', 'list_models', 'read_reply', 'list_background_runs', 'list_routines', 'routine_status'])
const MARKED = TOOLS.map(t => READS.has(t.name) ? { ...t, annotations: { readOnlyHint: true } } : t)
/* A voice call's copy (0.742, `call` after the connection file): the hand-off tools, and three that read the folder. */
const CALL_TOOLS = process.argv[3] === 'call' ? [
  ...TOOLS.filter(t => ['list_teammates', 'start_conversation', 'send_message', 'read_reply', 'list_background_runs'].includes(t.name)),
  { name: 'find_files', description: 'List the files in the folder Locust has open, or those whose path contains query. Read only.', inputSchema: schema({ query: { type: 'string', maxLength: 200 } }) },
  { name: 'read_file', description: 'Read a text file in the folder Locust has open, by its path in the folder, 400 lines at a time from from_line. Never a file of secrets. Read only.', inputSchema: schema({ path: { type: 'string', minLength: 1, maxLength: 1000 }, from_line: { type: 'integer', minimum: 1 } }, ['path']) },
  { name: 'search_files', description: 'Find the lines that contain text in the folder Locust has open. Read only.', inputSchema: schema({ text: string }, ['text']) }
].map(t => READS.has(t.name) || ['find_files', 'read_file', 'search_files'].includes(t.name) ? { ...t, annotations: { readOnlyHint: true } } : t) : undefined
const LISTED = CALL_TOOLS ?? MARKED

async function call(name, args) {
  let connection
  try {
    connection = JSON.parse(await readFile(process.argv[2], 'utf8'))
    const url = new URL(connection.url)
    // A profile file can never redirect this bearer token off the machine.
    if (url.protocol !== 'http:' || url.hostname !== '127.0.0.1' || url.pathname !== '/tools/call' || url.search || url.hash || url.username || url.password || !url.port || !/^[0-9a-f]{64}$/.test(connection.token)) throw new Error('invalid')
  } catch { return result(NOT_RUNNING, true) }
  return new Promise(resolve => {
    const payload = JSON.stringify({ name, arguments: args })
    const req = request(connection.url, { method: 'POST', headers: { authorization: `Bearer ${connection.token}`, 'content-type': 'application/json', 'content-length': Buffer.byteLength(payload) }, timeout: 60_000 }, res => {
      if (res.statusCode !== 200) {
        res.resume()
        resolve(result(res.statusCode === 401 ? 'Locust refused this connection. Check the switch in Settings > General and try again.' : 'Locust could not complete that request. Nothing was retried.', true))
        return
      }
      const chunks = []
      let size = 0
      res.on('data', chunk => { size += chunk.length; if (size <= 5 * 1024 * 1024) chunks.push(chunk); else res.destroy() })
      res.on('error', () => resolve(result('Locust\'s answer could not be read. Nothing was retried.', true)))
      res.on('end', () => {
        try {
          const answer = JSON.parse(Buffer.concat(chunks).toString('utf8'))
          resolve(Array.isArray(answer.content) ? answer : result('Locust\'s answer could not be read. Nothing was retried.', true))
        } catch { resolve(result('Locust\'s answer could not be read. Nothing was retried.', true)) }
      })
    })
    req.on('timeout', () => { req.destroy(); resolve(result('Locust did not answer in time. Nothing was retried; check its history before starting again.', true)) })
    req.on('error', error => resolve(result(error.code === 'ECONNREFUSED' ? NOT_RUNNING : 'The connection to Locust ended without an answer. Nothing was retried; check its history before starting again.', true)))
    req.end(payload)
  })
}

const send = (id, result) => process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id, result }) + '\n')
const error = (id, code, message) => process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id, error: { code, message } }) + '\n')
let buffer = ''
process.stdin.setEncoding('utf8')
process.stdin.on('data', chunk => {
  buffer += chunk
  if (Buffer.byteLength(buffer) > 256 * 1024) { error(null, -32600, 'MCP request is too large. Nothing was started.'); process.exitCode = 1; process.stdin.destroy(); return }
  let cut
  while ((cut = buffer.indexOf('\n')) >= 0) {
    const line = buffer.slice(0, cut).trim(); buffer = buffer.slice(cut + 1)
    if (!line) continue
    let rpc
    try { rpc = JSON.parse(line) } catch { error(null, -32700, 'Invalid JSON'); continue }
    if (typeof rpc !== 'object' || rpc === null || rpc.jsonrpc !== '2.0') { error(null, -32600, 'Invalid request'); continue }
    if (rpc.id === undefined) continue // notifications have no response
    if (rpc.method === 'initialize') { send(rpc.id, { protocolVersion: ['2024-11-05', '2025-03-26', '2025-06-18'].includes(rpc.params?.protocolVersion) ? rpc.params.protocolVersion : '2025-06-18', capabilities: { tools: {} }, serverInfo: { name: 'locust', version: '1' } }); continue }
    if (rpc.method === 'ping') { send(rpc.id, {}); continue }
    if (rpc.method === 'tools/list') { send(rpc.id, { tools: LISTED }); continue }
    if (rpc.method === 'tools/call') {
      if (!LISTED.some(t => t.name === rpc.params?.name)) { error(rpc.id, -32602, 'Unknown Locust tool'); continue }
      void call(rpc.params.name, rpc.params.arguments ?? {}).then(answer => send(rpc.id, answer)).catch(() => send(rpc.id, result('Locust could not complete that request. Nothing was retried.', true)))
      continue
    }
    error(rpc.id, -32601, 'Method not found')
  }
})
