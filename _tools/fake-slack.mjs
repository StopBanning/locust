// A STAND-IN FOR SLACK (0.832), for Locust's tests and drives: the Web API methods Locust's Slack link calls and its
// Socket Mode, answered as Slack's are (https://docs.slack.dev), with the person's side played by the test -- a direct
// message, one in a thread, a button pressed -- and everything the app sent kept to be read. Strict where Slack is:
// each token for its own methods, a message's words and blocks within Slack's limits, a thread that exists, and every
// envelope acknowledged.
//
// In memory, `fakeSlack().fetch` stands in for fetch and `.WebSocket` for the WebSocket. As a server
// (`node _tools/fake-slack.mjs --port 47813`), it answers /api/<method>, Socket Mode at /link, and a drive plays the
// person at /control/*. It listens on 127.0.0.1 only.
import { randomBytes } from 'node:crypto'
import { createServer } from 'node:http'
import { fileURLToPath } from 'node:url'

import { acceptUpgrades, memorySocketClass } from './fake-socket.mjs'

export function fakeSlack(options = {}) {
  // Made-up tokens of Slack's shapes, for a made-up workspace -- put together here, so no file holds one whole for a
  // secret scanner to take for a real one.
  const botToken = options.botToken ?? ['xoxb', '1000000000001', '2000000000002', 'fake'.repeat(6)].join('-')
  const appToken = options.appToken ?? ['xapp', '1', 'A0LOCUST01', '3000000000003', 'fake'.repeat(16)].join('-')
  const clock = () => options.now?.() ?? Date.now()
  let counter = 0
  /** A Slack timestamp: seconds, a dot, and six digits that make it unique. */
  const stamp = (at = clock()) => { counter += 1; return `${String(Math.floor(at / 1000))}.${String(counter).padStart(6, '0')}` }
  const team = { id: 'T0LOCUST01', name: 'Locust Test', domain: 'locust-test' }
  const app = { id: 'A0LOCUST01' }
  const bot = { user_id: 'U0BOT00001', bot_id: 'B0BOT00001', name: 'locust' }
  const person = { id: 'U0SAM00001', name: 'sam', real_name: 'Sam Rivers', profile: { display_name: 'Sam', real_name: 'Sam Rivers' } }
  const personChannel = 'D0SAM00001'
  const state = {
    socketBase: options.socketBase ?? 'ws://127.0.0.1:1',
    socketMode: options.socketMode ?? true,
    messages: new Map(),
    sent: [],
    edits: [],
    reactions: [],
    envelopes: [],
    acked: new Set(),
    refused: [],
    hellos: 0,
    connections: 0,
    opened: 0
  }
  const users = new Map([[person.id, person]])
  const channels = new Map([[personChannel, person.id]])
  const tickets = new Set()
  const live = new Set()

  const fail = (error, status = 200) => ({ status, body: { ok: false, error } })
  const ok = (body = {}) => ({ status: 200, body: { ok: true, ...body } })
  const refuse = (method, problem) => { state.refused.push({ method, problem }); return fail(problem) }

  const blocksProblem = (blocks) => {
    if (!Array.isArray(blocks)) return 'invalid_blocks'
    if (blocks.length > 50) return 'invalid_blocks'
    for (const block of blocks) {
      if (block.type === 'section') {
        if (block.text?.type !== 'mrkdwn' && block.text?.type !== 'plain_text') return 'invalid_blocks'
        if (typeof block.text.text !== 'string' || block.text.text.length === 0 || block.text.text.length > 3000) return 'invalid_blocks'
      } else if (block.type === 'actions') {
        if (!Array.isArray(block.elements) || block.elements.length === 0 || block.elements.length > 25) return 'invalid_blocks'
        const ids = new Set()
        for (const element of block.elements) {
          if (element.type !== 'button' || element.text?.type !== 'plain_text') return 'invalid_blocks'
          if (typeof element.text.text !== 'string' || element.text.text.length === 0 || element.text.text.length > 75) return 'invalid_blocks'
          if (typeof element.action_id !== 'string' || element.action_id.length > 255 || ids.has(element.action_id)) return 'invalid_blocks'
          if (element.value !== undefined && String(element.value).length > 2000) return 'invalid_blocks'
          if (element.style !== undefined && element.style !== 'primary' && element.style !== 'danger') return 'invalid_blocks'
          ids.add(element.action_id)
        }
      } else return 'invalid_blocks'
    }
    return undefined
  }

  async function method(name, args, authorization) {
    const token = authorization?.replace(/^Bearer\s+/, '')
    if (token === undefined || token.length === 0) return fail('not_authed')
    // An app token opens connections and nothing else; a bot token does everything but that.
    if (name === 'apps.connections.open') {
      if (token.startsWith('xoxb-')) return fail('not_allowed_token_type')
      if (token !== appToken) return fail('invalid_auth')
      if (!state.socketMode) return fail('internal_error')
      const ticket = randomBytes(12).toString('hex')
      tickets.add(ticket)
      state.opened += 1
      return ok({ url: `${state.socketBase}/link/?ticket=${ticket}&app_id=${app.id}` })
    }
    if (token.startsWith('xapp-')) return fail('not_allowed_token_type')
    if (token !== botToken) return fail('invalid_auth')
    switch (name) {
      case 'auth.test': return ok({ url: `https://${team.domain}.slack.com/`, team: team.name, user: bot.name, team_id: team.id, user_id: bot.user_id, bot_id: bot.bot_id, is_enterprise_install: false })
      case 'users.info': {
        const user = users.get(args.user)
        return user === undefined ? fail('user_not_found') : ok({ user })
      }
      case 'chat.postMessage': {
        if (!channels.has(args.channel)) return fail('channel_not_found')
        const blocks = args.blocks === undefined ? undefined : JSON.parse(args.blocks)
        if ((args.text ?? '').length === 0 && blocks === undefined) return refuse(name, 'no_text')
        if ((args.text ?? '').length > 4000) return refuse(name, 'msg_too_long')
        if (blocks !== undefined) { const problem = blocksProblem(blocks); if (problem !== undefined) return refuse(name, problem) }
        if (args.thread_ts !== undefined) {
          const parent = state.messages.get(`${args.channel}:${args.thread_ts}`)
          if (parent === undefined) return refuse(name, 'thread_not_found')
        }
        const ts = stamp()
        const message = { channel: args.channel, ts, text: args.text ?? '', blocks: blocks ?? [], thread_ts: args.thread_ts, user: bot.user_id, bot_id: bot.bot_id, unfurl_links: args.unfurl_links }
        state.messages.set(`${args.channel}:${ts}`, message)
        state.sent.push(message)
        return ok({ channel: args.channel, ts, message })
      }
      case 'chat.update': {
        const message = state.messages.get(`${args.channel}:${args.ts}`)
        if (message === undefined) return fail('message_not_found')
        if (message.bot_id !== bot.bot_id) return fail('cant_update_message')
        if ((args.text ?? '').length > 4000) return refuse(name, 'msg_too_long')
        const blocks = args.blocks === undefined ? undefined : JSON.parse(args.blocks)
        if (blocks !== undefined && blocks.length > 0) { const problem = blocksProblem(blocks); if (problem !== undefined) return refuse(name, problem) }
        message.text = args.text ?? message.text
        // Words with no blocks: the blocks go, as Slack's do.
        message.blocks = blocks ?? (args.text !== undefined ? [] : message.blocks)
        state.edits.push({ channel: args.channel, ts: args.ts, text: message.text, blocks: message.blocks })
        return ok({ channel: args.channel, ts: args.ts, text: message.text })
      }
      case 'reactions.add':
      case 'reactions.remove': {
        const message = state.messages.get(`${args.channel}:${args.timestamp}`)
        if (message === undefined) return fail('message_not_found')
        const has = state.reactions.some((one) => one.channel === args.channel && one.ts === args.timestamp && one.name === args.name && one.on)
        if (name === 'reactions.add' && has) return fail('already_reacted')
        if (name === 'reactions.remove' && !has) return fail('no_reaction')
        if (name === 'reactions.add') state.reactions.push({ channel: args.channel, ts: args.timestamp, name: args.name, on: true })
        else for (const one of state.reactions) if (one.channel === args.channel && one.ts === args.timestamp && one.name === args.name) one.on = false
        return ok()
      }
      default: return fail('unknown_method')
    }
  }

  const accept = (conn) => {
    const url = new URL(conn.url)
    const ticket = url.searchParams.get('ticket')
    if (!/\/link\/?$/.test(url.pathname) || ticket === null || !tickets.has(ticket)) return false
    // A ticket opens one connection, once.
    tickets.delete(ticket)
    state.connections += 1
    const entry = { conn }
    conn.on('open', () => {
      live.add(entry)
      state.hellos += 1
      conn.send(JSON.stringify({ type: 'hello', num_connections: live.size, debug_info: { host: 'fake-slack', build_number: 1, approximate_connection_time: 18_060 }, connection_info: { app_id: app.id } }))
    })
    conn.on('message', (text) => {
      let reply
      try { reply = JSON.parse(text) } catch { return }
      if (typeof reply.envelope_id === 'string') state.acked.add(reply.envelope_id)
    })
    conn.on('close', () => live.delete(entry))
    return true
  }

  const deliver = (type, payload) => {
    const envelope = { envelope_id: randomBytes(8).toString('hex'), type, payload, accepts_response_payload: false, retry_attempt: 0, retry_reason: '' }
    state.envelopes.push(envelope)
    // Slack sends each envelope on one of the app's connections.
    const [first] = live
    first?.conn.send(JSON.stringify(envelope))
    return envelope
  }

  const fake = {
    botToken,
    appToken,
    team,
    app,
    bot,
    person,
    personChannel,
    state,
    WebSocket: memorySocketClass(accept),
    fetch: async (url, init = {}) => {
      const parsed = new URL(url)
      const name = parsed.pathname.replace(/^\/api\//, '')
      const headers = new Headers(init.headers)
      const type = headers.get('content-type') ?? ''
      const args = typeof init.body === 'string' && init.body.length > 0
        ? (type.includes('application/json') ? JSON.parse(init.body) : Object.fromEntries(new URLSearchParams(init.body)))
        : {}
      const { status, body } = await method(name, args, headers.get('authorization') ?? undefined)
      return new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })
    },
    /** The person writes to the app. A stranger is another user, with their own direct message; `thread` answers in one. */
    say(text, more = {}) {
      const user = more.from ?? person
      if (more.from !== undefined) users.set(user.id, user)
      const channel = more.channel ?? (user === person ? personChannel : `D${user.id.slice(1)}`)
      channels.set(channel, user.id)
      const ts = stamp(more.at)
      const event = {
        type: 'message', channel, user: user.id, text: text ?? '', ts, event_ts: ts, channel_type: more.channelType ?? 'im', team: team.id,
        ...(more.thread === undefined ? {} : { thread_ts: more.thread, parent_user_id: bot.user_id }),
        ...(text === undefined ? { subtype: 'file_share', files: [{ id: 'F0FILE0001', name: 'picture.png' }] } : {}),
        ...(more.subtype === undefined ? {} : { subtype: more.subtype }),
        ...(more.botId === undefined ? {} : { bot_id: more.botId })
      }
      state.messages.set(`${channel}:${ts}`, { ...event, from: 'person' })
      deliver('events_api', { token: 'verification-token', team_id: team.id, api_app_id: app.id, event, type: 'event_callback', event_id: `Ev${randomBytes(5).toString('hex')}`, event_time: Math.floor(clock() / 1000) })
      return ts
    },
    /** Slack sends the same event again, as it does when an acknowledgement is late. */
    again(envelope) {
      const [first] = live
      first?.conn.send(JSON.stringify({ ...envelope, retry_attempt: 1, retry_reason: 'timeout' }))
    },
    /** The person presses a button under one of the app's messages. */
    press(ts, actionId, more = {}) {
      const user = more.from ?? person
      const channel = more.channel ?? personChannel
      const message = state.messages.get(`${channel}:${ts}`)
      return deliver('interactive', {
        type: 'block_actions', user: { id: user.id, username: user.name, name: user.name, team_id: team.id }, api_app_id: app.id, team: { id: team.id, domain: team.domain },
        container: { type: 'message', message_ts: ts, channel_id: channel, is_ephemeral: false }, channel: { id: channel, name: 'directmessage' },
        message: { ts, text: message?.text ?? '', type: 'message', user: bot.user_id },
        actions: [{ action_id: actionId, block_id: 'b1', value: actionId, type: 'button', action_ts: stamp() }],
        response_url: 'https://hooks.slack.invalid/actions/fake', trigger_id: randomBytes(8).toString('hex')
      })
    },
    /** Slack asks the app to move to a new connection (a refresh), or tells it Socket Mode was turned off. */
    disconnect(reason = 'refresh_requested') { for (const { conn } of live) conn.send(JSON.stringify({ type: 'disconnect', reason, debug_info: { host: 'fake-slack' } })) },
    drop() { for (const { conn } of [...live]) conn.close(1006, 'dropped') },
    /** What the person reads in the app's direct messages, in order: its messages, a thread's included. */
    read: (channel = personChannel) => state.sent.filter((one) => one.channel === channel).map((one) => one.text),
    async until(test, ms = 5_000, what = 'the fake Slack to see it') {
      const started = Date.now()
      for (;;) {
        const found = test(state)
        if (found) return found
        if (Date.now() - started > ms) throw new Error(`Timed out waiting for ${what}`)
        await new Promise((resolve) => setTimeout(resolve, 20))
      }
    },
    listen(port = 0) {
      const server = createServer((req, res) => {
        const chunks = []
        req.on('data', (chunk) => chunks.push(chunk))
        req.on('end', async () => {
          const text = Buffer.concat(chunks).toString('utf8')
          const parsed = new URL(req.url, 'http://127.0.0.1')
          const reply = (status, data) => { res.writeHead(status, { 'content-type': 'application/json' }).end(JSON.stringify(data)) }
          const json = () => (text.length > 0 ? JSON.parse(text) : {})
          if (parsed.pathname === '/control/state') return reply(200, { team, app, bot, person, personChannel, ...state, messages: [...state.messages.values()], acked: [...state.acked] })
          if (parsed.pathname === '/control/say') { const body = json(); return reply(200, { ts: fake.say(body.text, body) }) }
          if (parsed.pathname === '/control/press') { const body = json(); return reply(200, fake.press(body.ts, body.actionId, body)) }
          if (parsed.pathname === '/control/disconnect') { fake.disconnect(json().reason); return reply(200, { ok: true }) }
          const type = String(req.headers['content-type'] ?? '')
          const args = text.length === 0 ? {} : type.includes('application/json') ? JSON.parse(text) : Object.fromEntries(new URLSearchParams(text))
          const { status, body } = await method(parsed.pathname.replace(/^\/api\//, ''), args, req.headers.authorization)
          reply(status, body)
        })
      })
      acceptUpgrades(server, accept)
      return new Promise((resolve) => server.listen(port, '127.0.0.1', () => {
        state.socketBase = `ws://127.0.0.1:${String(server.address().port)}`
        resolve(server)
      }))
    }
  }
  return fake
}

if (process.argv[1] !== undefined && fileURLToPath(import.meta.url) === process.argv[1]) {
  const at = process.argv.indexOf('--port')
  const port = at > 0 ? Number(process.argv[at + 1]) : 47813
  const fake = fakeSlack()
  const server = await fake.listen(port)
  console.log(`fake Slack on http://127.0.0.1:${String(server.address().port)}`)
}
