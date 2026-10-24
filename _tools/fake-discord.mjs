// A STAND-IN FOR DISCORD (0.832), for Locust's tests and drives: the REST calls Locust's Discord link makes and its
// gateway, answered as Discord's are (https://docs.discord.com/developers), with the person's side played by the test
// -- a direct message, a slash command, a button pressed -- and everything the bot sent kept to be read. Strict where
// Discord is: 2,000 characters a message, five buttons a row and five rows, a command's name and lengths, an
// interaction answered once and within three seconds, and the token checked on every call and on the gateway.
//
// In memory, `fakeDiscord().fetch` stands in for fetch and `.WebSocket` for the WebSocket. As a server
// (`node _tools/fake-discord.mjs --port 47812`), it answers REST at /api/v10/*, the gateway at /gateway, and a drive
// plays the person at /control/*. It listens on 127.0.0.1 only.
import { randomBytes } from 'node:crypto'
import { createServer } from 'node:http'
import { fileURLToPath } from 'node:url'

import { acceptUpgrades, memorySocketClass } from './fake-socket.mjs'

const EPOCH = 1_420_070_400_000n
const DIRECT_MESSAGES = 1 << 12
const PRIVILEGED = (1 << 1) | (1 << 8) | (1 << 15)

export function fakeDiscord(options = {}) {
  // A made-up token of a bot's shape, for a made-up bot.
  const token = options.token ?? 'MTEwMDAwMDAwMDAwMDAwMDAwMQ.GfakeA.fake_fake_fake_fake_fake_fake_fake_fak'
  const clock = () => options.now?.() ?? Date.now()
  let counter = 0n
  const snowflake = () => { counter += 1n; return String(((BigInt(clock()) - EPOCH) << 22n) + counter) }
  const application = { id: '1100000000000000001', name: options.appName ?? 'Locust test' }
  const bot = { id: '1100000000000000001', username: options.username ?? 'Locust test', discriminator: '4242', global_name: null, bot: true }
  const person = { id: '1200000000000000007', username: 'sam_here', global_name: 'Sam', discriminator: '0' }
  const personChannel = '1300000000000000007'
  const state = {
    gateway: options.gateway ?? 'ws://127.0.0.1:1/gateway',
    messages: new Map(),
    sent: [],
    edits: [],
    responses: [],
    callbacks: [],
    typing: [],
    commands: [],
    refused: [],
    identified: 0,
    resumed: 0,
    heartbeats: 0,
    intents: [],
    connections: 0,
    sessions: new Set(),
    interactions: new Map()
  }
  const live = new Set()
  /** The direct messages anyone opened with the bot: the bot may write in them, and nowhere else. */
  const opened = new Set([personChannel, ...(options.channels ?? [])])
  let seq = 0
  const now = () => new Date(clock()).toISOString()

  const fail = (status, code, message) => ({ status, body: { code, message } })
  const ok = (body) => ({ status: 200, body })
  const refuse = (path, problem) => { state.refused.push({ path, problem }); return fail(400, 50035, `Invalid Form Body: ${problem}`) }

  /** Why Discord would refuse this message's body, or undefined when it would take it. */
  const bodyProblem = (body, editing = false) => {
    if (body.content !== undefined && typeof body.content !== 'string') return 'content: must be a string'
    if (typeof body.content === 'string' && body.content.length > 2000) return `content: Must be 2000 or fewer in length (${body.content.length})`
    if (!editing && (body.content ?? '').length === 0 && (body.components ?? []).length === 0) return 'Cannot send an empty message'
    if (body.allowed_mentions !== undefined && !Array.isArray(body.allowed_mentions.parse)) return 'allowed_mentions.parse: must be a list'
    if (body.flags !== undefined && (body.flags & ~((1 << 2) | (1 << 12))) !== 0) return 'flags: only SUPPRESS_EMBEDS and SUPPRESS_NOTIFICATIONS may be set'
    if (body.components !== undefined) {
      if (!Array.isArray(body.components) || body.components.length > 5) return 'components: Must be 5 or fewer in length'
      const ids = new Set()
      for (const row of body.components) {
        if (row.type !== 1 || !Array.isArray(row.components) || row.components.length === 0 || row.components.length > 5) return 'components.row: an action row holds one to five components'
        for (const button of row.components) {
          if (button.type !== 2 || ![1, 2, 3, 4].includes(button.style)) return 'components.button: a type 2 button of style 1 to 4'
          if (typeof button.label !== 'string' || button.label.length === 0 || button.label.length > 80) return 'components.button.label: 1 to 80 characters'
          if (typeof button.custom_id !== 'string' || button.custom_id.length === 0 || button.custom_id.length > 100) return 'components.button.custom_id: 1 to 100 characters'
          if (ids.has(button.custom_id)) return 'components.button.custom_id: Component custom id cannot be duplicated'
          ids.add(button.custom_id)
        }
      }
    }
    return undefined
  }
  const commandProblem = (list) => {
    if (!Array.isArray(list) || list.length > 100) return 'commands: 100 or fewer'
    const names = new Set()
    for (const command of list) {
      if (typeof command.name !== 'string' || !/^[-_\p{L}\p{N}]{1,32}$/u.test(command.name) || command.name !== command.name.toLowerCase()) return `name: ${String(command.name)} is not a command name`
      if (names.has(command.name)) return `name: ${command.name} twice`
      names.add(command.name)
      if (typeof command.description !== 'string' || command.description.length < 1 || command.description.length > 100) return `description of ${command.name}: 1 to 100 characters`
      for (const option of command.options ?? []) {
        if (option.type !== 3 || !/^[-_\p{L}\p{N}]{1,32}$/u.test(option.name ?? '') || typeof option.description !== 'string' || option.description.length < 1 || option.description.length > 100) return `option of ${command.name}: a string option, named, described in 1 to 100 characters`
        if (option.max_length !== undefined && (option.max_length < 1 || option.max_length > 6000)) return `option of ${command.name}: max_length 1 to 6000`
      }
      if (command.contexts !== undefined && !command.contexts.every((one) => [0, 1, 2].includes(one))) return `contexts of ${command.name}`
    }
    return undefined
  }

  const create = (channelId, body, via) => {
    const message = {
      id: snowflake(), channel_id: channelId, author: bot, content: body.content ?? '', components: body.components ?? [], timestamp: now(),
      flags: body.flags ?? 0, allowed_mentions: body.allowed_mentions, ...(body.message_reference === undefined ? {} : { message_reference: body.message_reference }), via
    }
    state.messages.set(message.id, message)
    state.sent.push({ ...message, reply_to: body.message_reference?.message_id })
    return message
  }

  async function rest(method, path, query, body, authorization) {
    const bearer = authorization === `Bot ${token}`
    const needsBot = !path.startsWith('/interactions/') && !path.startsWith('/webhooks/')
    if (needsBot && !bearer) return fail(401, 0, '401: Unauthorized')
    let match
    if (method === 'GET' && path === '/users/@me') return ok(bot)
    if (method === 'GET' && path === '/applications/@me') return ok({ ...application, bot_public: true, bot_require_code_grant: false, flags: 0 })
    if (method === 'GET' && path === '/gateway/bot') return ok({ url: state.gateway, shards: 1, session_start_limit: { total: 1000, remaining: 990, reset_after: 0, max_concurrency: 1 } })
    if (method === 'POST' && (match = /^\/channels\/(\d+)\/messages$/.exec(path))) {
      const problem = bodyProblem(body)
      if (problem !== undefined) return refuse(path, problem)
      if (!opened.has(match[1]) || options.blocked === true) return fail(403, 50007, 'Cannot send messages to this user')
      return ok(create(match[1], body, 'channel'))
    }
    if (method === 'PATCH' && (match = /^\/channels\/(\d+)\/messages\/(\d+)$/.exec(path))) {
      const message = state.messages.get(match[2])
      if (message === undefined || message.channel_id !== match[1]) return fail(404, 10008, 'Unknown Message')
      if (message.author.id !== bot.id || message.via !== 'channel') return fail(403, 50005, 'Cannot edit a message authored by another user')
      const problem = bodyProblem(body, true)
      if (problem !== undefined) return refuse(path, problem)
      if (body.content !== undefined) message.content = body.content
      if (body.components !== undefined) message.components = body.components
      state.edits.push({ id: message.id, channel_id: message.channel_id, content: message.content, components: message.components })
      return ok(message)
    }
    if (method === 'POST' && (match = /^\/channels\/(\d+)\/typing$/.exec(path))) { state.typing.push({ channel_id: match[1], at: clock() }); return { status: 204 } }
    if (method === 'PUT' && (match = /^\/applications\/(\d+)\/commands$/.exec(path))) {
      if (match[1] !== application.id) return fail(403, 50001, 'Missing Access')
      const problem = commandProblem(body)
      if (problem !== undefined) return refuse(path, problem)
      state.commands.push(body)
      return ok(body.map((one) => ({ ...one, id: snowflake(), application_id: application.id, version: snowflake() })))
    }
    if (method === 'POST' && (match = /^\/interactions\/(\d+)\/([^/]+)\/callback$/.exec(path))) {
      const held = state.interactions.get(match[2])
      if (held === undefined || held.interaction.id !== match[1]) return fail(404, 10062, 'Unknown interaction')
      if (held.acknowledged) return fail(400, 40060, 'Interaction has already been acknowledged.')
      // Three seconds to answer, as Discord gives.
      if (clock() - held.at > 3_000) return fail(404, 10062, 'Unknown interaction')
      if (![4, 5, 6, 7].includes(body.type)) return refuse(path, `type: ${String(body.type)} is not a response here`)
      if ((body.type === 6 || body.type === 7) && held.interaction.type !== 3) return refuse(path, 'type: an update answers only a component')
      held.acknowledged = true
      state.callbacks.push({ id: held.interaction.id, kind: held.interaction.type, type: body.type })
      let message
      if (body.type === 4 || body.type === 5) {
        message = { id: snowflake(), channel_id: held.interaction.channel_id, author: bot, content: body.type === 4 ? body.data?.content ?? '' : '', components: body.data?.components ?? [], timestamp: now(), via: 'interaction', loading: body.type === 5 }
        state.messages.set(message.id, message)
        held.original = message.id
      }
      if (query.get('with_response') !== 'true') return { status: 204 }
      return ok({ interaction: { id: held.interaction.id, type: held.interaction.type, ...(message === undefined ? {} : { response_message_id: message.id, response_message_loading: body.type === 5, response_message_ephemeral: false }) }, ...(message === undefined ? {} : { resource: { type: body.type, message } }) })
    }
    if ((match = /^\/webhooks\/(\d+)\/([^/]+)\/messages\/@original$/.exec(path))) {
      const held = state.interactions.get(match[2])
      if (held === undefined || match[1] !== application.id) return fail(404, 10015, 'Unknown Webhook')
      const message = held.original === undefined ? undefined : state.messages.get(held.original)
      if (message === undefined) return fail(404, 10008, 'Unknown Message')
      // An interaction's token lasts fifteen minutes.
      if (clock() - held.at > 15 * 60_000) return fail(401, 50027, 'Invalid Webhook Token')
      if (method === 'GET') return ok(message)
      if (method === 'PATCH') {
        const problem = bodyProblem(body, true)
        if (problem !== undefined) return refuse(path, problem)
        if (body.content !== undefined) message.content = body.content
        if (body.components !== undefined) message.components = body.components
        message.loading = false
        state.responses.push({ id: message.id, interaction: held.interaction.id, content: message.content, components: message.components })
        return ok(message)
      }
    }
    return fail(404, 0, '404: Not Found')
  }

  /** One gateway connection, played as Discord plays it. */
  const accept = (conn) => {
    if (!/\?v=10&encoding=json$/.test(conn.url)) return false
    state.connections += 1
    const session = { id: randomBytes(8).toString('hex'), identified: false }
    conn.on('open', () => conn.send(JSON.stringify({ op: 10, d: { heartbeat_interval: options.heartbeatMs ?? 41_250 }, s: null, t: null })))
    conn.on('message', (text) => {
      let payload
      try { payload = JSON.parse(text) } catch { conn.close(4002, 'Error while decoding payload.'); return }
      if (payload.op === 1) { state.heartbeats += 1; conn.send(JSON.stringify({ op: 11, d: null, s: null, t: null })); return }
      if (payload.op === 2) {
        if (session.identified) { conn.close(4005, 'Already authenticated.'); return }
        if (payload.d?.token !== token) { conn.close(4004, 'Authentication failed.'); return }
        const intents = Number(payload.d?.intents)
        if (!Number.isInteger(intents) || intents < 0) { conn.close(4013, 'Invalid intent(s).'); return }
        if ((intents & PRIVILEGED) !== 0) { conn.close(4014, 'Disallowed intent(s).'); return }
        state.intents.push(intents)
        session.identified = true
        state.identified += 1
        state.sessions.add(session.id)
        live.add({ conn, session })
        conn.send(JSON.stringify({ op: 0, s: ++seq, t: 'READY', d: { v: 10, user: bot, guilds: [], session_id: session.id, resume_gateway_url: state.gateway, application: { id: application.id, flags: 0 } } }))
        return
      }
      if (payload.op === 6) {
        if (payload.d?.token !== token) { conn.close(4004, 'Authentication failed.'); return }
        if (!state.sessions.has(payload.d?.session_id)) { conn.send(JSON.stringify({ op: 9, d: false, s: null, t: null })); return }
        session.identified = true
        session.id = payload.d.session_id
        state.resumed += 1
        live.add({ conn, session })
        conn.send(JSON.stringify({ op: 0, s: ++seq, t: 'RESUMED', d: {} }))
        return
      }
      if (!session.identified) conn.close(4003, 'Not authenticated.')
    })
    conn.on('close', () => { for (const one of live) if (one.conn === conn) live.delete(one) })
    return true
  }

  const dispatch = (type, data) => {
    for (const { conn } of live) conn.send(JSON.stringify({ op: 0, s: ++seq, t: type, d: data }))
  }
  const userOf = (more) => more.from ?? person
  const interactionOf = (type, more, data, message) => {
    const interaction = {
      id: snowflake(), application_id: application.id, type, token: randomBytes(24).toString('base64url'), version: 1,
      channel_id: more.channelId ?? personChannel, channel: { id: more.channelId ?? personChannel, type: more.guildId === undefined ? 1 : 0 },
      context: more.guildId === undefined ? 1 : 0, app_permissions: '0', locale: 'en-US',
      ...(more.guildId === undefined ? { user: userOf(more) } : { guild_id: more.guildId, member: { user: userOf(more) } }),
      ...(message === undefined ? {} : { message: { id: message } }),
      data
    }
    state.interactions.set(interaction.token, { interaction, at: clock(), acknowledged: false })
    dispatch('INTERACTION_CREATE', interaction)
    return interaction
  }

  const fake = {
    token,
    bot,
    person,
    application,
    personChannel,
    state,
    WebSocket: memorySocketClass(accept),
    /** As fetch: what Locust's link calls. */
    fetch: async (url, init = {}) => {
      const parsed = new URL(url)
      const path = parsed.pathname.replace(/^\/api\/v10/, '')
      const body = typeof init.body === 'string' && init.body.length > 0 ? JSON.parse(init.body) : undefined
      const headers = new Headers(init.headers)
      const { status, body: data } = await rest(init.method ?? 'GET', path, parsed.searchParams, body, headers.get('authorization') ?? undefined)
      return status === 204 ? new Response(null, { status }) : new Response(JSON.stringify(data), { status, headers: { 'content-type': 'application/json' } })
    },
    /** The person writes a direct message. A stranger is another user, in their own channel; a server's message has a guild. */
    say(text, more = {}) {
      const id = snowflake()
      const author = userOf(more)
      if (more.guildId === undefined) opened.add(more.channelId ?? personChannel)
      dispatch('MESSAGE_CREATE', {
        id, channel_id: more.channelId ?? personChannel, author, content: text ?? '', timestamp: more.timestamp ?? now(), type: more.replyTo === undefined ? 0 : 19,
        ...(more.replyTo === undefined ? {} : { message_reference: { message_id: more.replyTo, channel_id: more.channelId ?? personChannel } }),
        attachments: text === undefined ? [{ id: snowflake(), filename: 'picture.png' }] : [], ...(more.guildId === undefined ? {} : { guild_id: more.guildId })
      })
      return id
    },
    /** The person uses a slash command: its name, and the words of its message option. */
    command(name, words, more = {}) {
      return interactionOf(2, more, { id: snowflake(), name, type: 1, ...(words === undefined ? {} : { options: [{ name: 'message', type: 3, value: words }] }) })
    },
    /** The person presses a button under one of the bot's messages. */
    press(messageId, customId, more = {}) {
      return interactionOf(3, more, { custom_id: customId, component_type: 2 }, messageId)
    },
    /** Asks every connection to reconnect, as Discord sometimes does (op 7). */
    reconnect() { for (const { conn } of live) conn.send(JSON.stringify({ op: 7, d: null, s: null, t: null })) },
    /** Drops every connection, as a network does. */
    drop(code = 1006) { for (const { conn } of [...live]) conn.close(code, 'dropped') },
    /** What the person reads in their direct messages with the bot: its messages and its commands' answers, in order. */
    read: (channel = personChannel) => [...state.messages.values()].filter((one) => one.channel_id === channel && one.author.id === bot.id).map((one) => one.content),
    async until(test, ms = 5_000, what = 'the fake Discord to see it') {
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
          const body = text.length > 0 ? JSON.parse(text) : undefined
          const parsed = new URL(req.url, 'http://127.0.0.1')
          const reply = (status, data) => {
            if (status === 204) { res.writeHead(204).end(); return }
            res.writeHead(status, { 'content-type': 'application/json' }).end(JSON.stringify(data))
          }
          const view = () => ({ ...state, messages: [...state.messages.values()], sessions: [...state.sessions], interactions: [...state.interactions.values()].map((one) => one.interaction.id) })
          if (parsed.pathname === '/control/state') return reply(200, { bot, person, application, personChannel, ...view() })
          if (parsed.pathname === '/control/say') return reply(200, { id: fake.say(body?.text, body ?? {}) })
          if (parsed.pathname === '/control/command') return reply(200, fake.command(body.name, body.words, body))
          if (parsed.pathname === '/control/press') return reply(200, fake.press(body.messageId, body.customId, body))
          if (parsed.pathname === '/control/drop') { fake.drop(); return reply(200, { ok: true }) }
          const path = parsed.pathname.replace(/^\/api\/v10/, '')
          const { status, body: data } = await rest(req.method ?? 'GET', path, parsed.searchParams, body, req.headers.authorization)
          reply(status, data)
        })
      })
      acceptUpgrades(server, (conn) => (new URL(conn.url).pathname === '/gateway' || new URL(conn.url).pathname === '/gateway/' ? accept(conn) : false))
      return new Promise((resolve) => server.listen(port, '127.0.0.1', () => {
        state.gateway = `ws://127.0.0.1:${String(server.address().port)}/gateway`
        resolve(server)
      }))
    }
  }
  return fake
}

if (process.argv[1] !== undefined && fileURLToPath(import.meta.url) === process.argv[1]) {
  const at = process.argv.indexOf('--port')
  const port = at > 0 ? Number(process.argv[at + 1]) : 47812
  const fake = fakeDiscord()
  const server = await fake.listen(port)
  console.log(`fake Discord on http://127.0.0.1:${String(server.address().port)} token ${fake.token}`)
}
