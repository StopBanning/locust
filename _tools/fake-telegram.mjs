// A STAND-IN FOR TELEGRAM'S BOT API (0.831), for Locust's tests and drives: the methods Locust's Telegram link calls,
// answered as the Bot API answers them (https://core.telegram.org/bots/api), with the person's side played by the
// test -- a message said, a button pressed -- and everything the bot sent kept to be read. Its HTML check is as strict
// as Telegram's parser on the tags Locust uses, so markup Telegram would refuse is refused here first.
//
// In memory, `fakeTelegram().fetch` stands in for fetch. As a server (`node _tools/fake-telegram.mjs --port 47811`),
// it answers the bot's methods at /bot<token>/<method> and a drive plays the person at /control/*. It listens on
// 127.0.0.1 only.
import { createServer } from 'node:http'
import { fileURLToPath } from 'node:url'

const ALLOWED = new Set(['b', 'strong', 'i', 'em', 'u', 'ins', 's', 'strike', 'del', 'a', 'code', 'pre', 'blockquote', 'tg-spoiler', 'span'])

/** Why Telegram would refuse this HTML, or undefined when it would take it. */
export function htmlProblem(html) {
  const open = []
  const tag = /<(\/?)([a-z-]+)((?:\s+[a-z-]+="[^"<>]*")*)\s*>|<|&(?!(?:lt|gt|amp|quot|#\d{1,7});)/g
  let match
  while ((match = tag.exec(html)) !== null) {
    if (match[0] === '<') return `Can't find end of the entity starting at byte offset ${match.index}`
    if (match[0] === '&') return `Unsupported start tag "&" at byte offset ${match.index}`
    const [, closing, name] = match
    if (!ALLOWED.has(name)) return `Unsupported start tag "${name}" at byte offset ${match.index}`
    if (closing === '/') {
      if (open.pop() !== name) return `Unexpected end tag at byte offset ${match.index}`
    } else open.push(name)
  }
  return open.length === 0 ? undefined : `Can't find end tag corresponding to start tag "${open.at(-1)}"`
}

const visible = (html) => html.replace(/<[^>]+>/g, '').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&amp;/g, '&')

export function fakeTelegram(options = {}) {
  const token = options.token ?? '123456789:AAHfakefakefakefakefakefakefakefake1'
  const bot = { id: Number(token.split(':')[0]), is_bot: true, first_name: options.botName ?? 'Locust test bot', username: options.username ?? 'locust_test_bot' }
  const person = { id: options.personId ?? 7001, is_bot: false, first_name: options.personName ?? 'Sam', username: 'sam_here' }
  const state = {
    webhook: options.webhook ?? '',
    updates: [],
    nextUpdate: 1,
    nextMessage: 100,
    sent: [],
    edits: [],
    answers: [],
    actions: [],
    commands: [],
    refused: [],
    polls: 0,
    confirmed: 0
  }
  const waiters = new Set()
  const wake = () => { for (const waiter of [...waiters]) waiter() }
  const now = () => Math.floor((options.now?.() ?? Date.now()) / 1000)

  const fail = (code, description, extra = {}) => ({ status: code, body: { ok: false, error_code: code, description, ...extra } })
  const ok = (result) => ({ status: 200, body: { ok: true, result } })
  const checkText = (text, parseMode) => {
    if (typeof text !== 'string' || text.length === 0) return 'Bad Request: message text is empty'
    if (parseMode === 'HTML') {
      const problem = htmlProblem(text)
      if (problem !== undefined) return `Bad Request: can't parse entities: ${problem}`
      if (visible(text).length > 4096) return 'Bad Request: message is too long'
    } else if (text.length > 4096) return 'Bad Request: message is too long'
    return undefined
  }

  async function method(name, params, signal) {
    switch (name) {
      case 'getMe': return ok(bot)
      case 'getWebhookInfo': return ok({ url: state.webhook, has_custom_certificate: false, pending_update_count: state.updates.length })
      case 'getUpdates': {
        state.polls += 1
        if (state.webhook !== '') return fail(409, "Conflict: can't use getUpdates method while webhook is active; use deleteWebhook to delete the webhook first")
        const offset = typeof params.offset === 'number' ? params.offset : 0
        // An offset confirms every update before it, as Telegram's does.
        const before = state.updates.length
        state.updates = state.updates.filter((update) => update.update_id >= offset)
        state.confirmed += before - state.updates.length
        if (state.updates.length === 0 && Number(params.timeout) > 0) {
          await new Promise((resolve) => {
            const done = () => { waiters.delete(done); clearTimeout(timer); resolve(undefined) }
            const timer = setTimeout(done, Math.min(Number(params.timeout), options.maxWaitSeconds ?? 50) * 1000)
            waiters.add(done)
            signal?.addEventListener('abort', done)
          })
        }
        if (signal?.aborted) throw Object.assign(new Error('The operation was aborted.'), { name: 'AbortError' })
        return ok(state.updates.slice(0, Math.min(100, Number(params.limit) || 100)))
      }
      case 'sendMessage': {
        const problem = checkText(params.text, params.parse_mode)
        if (problem !== undefined) { state.refused.push({ method: name, text: params.text, problem }); return fail(400, problem) }
        const message = { message_id: state.nextMessage++, chat: { id: params.chat_id, type: 'private' }, date: now(), text: params.parse_mode === 'HTML' ? visible(params.text) : params.text }
        state.sent.push({ ...message, html: params.text, parse_mode: params.parse_mode, reply_markup: params.reply_markup, reply_to: params.reply_parameters?.message_id })
        return ok(message)
      }
      case 'editMessageText': {
        const problem = checkText(params.text, params.parse_mode)
        if (problem !== undefined) { state.refused.push({ method: name, text: params.text, problem }); return fail(400, problem) }
        const message = state.sent.find((one) => one.message_id === params.message_id)
        if (message === undefined) return fail(400, 'Bad Request: message to edit not found')
        state.edits.push({ message_id: params.message_id, html: params.text, reply_markup: params.reply_markup })
        message.html = params.text
        message.reply_markup = params.reply_markup
        return ok({ ...message })
      }
      case 'answerCallbackQuery': state.answers.push({ id: params.callback_query_id, text: params.text }); return ok(true)
      case 'sendChatAction': state.actions.push({ chat_id: params.chat_id, action: params.action }); return ok(true)
      case 'setMyCommands': {
        const commands = params.commands ?? []
        if (!Array.isArray(commands) || commands.length > 100) return fail(400, 'Bad Request: too many commands')
        for (const one of commands) {
          if (!/^[a-z0-9_]{1,32}$/.test(one.command ?? '')) return fail(400, 'Bad Request: BOT_COMMAND_INVALID')
          if (typeof one.description !== 'string' || one.description.length < 1 || one.description.length > 256) return fail(400, 'Bad Request: BOT_COMMAND_DESCRIPTION_INVALID')
        }
        state.commands.push(commands)
        return ok(true)
      }
      default: return fail(404, 'Not Found')
    }
  }

  async function answer(path, body, signal) {
    const match = /^\/bot([^/]+)\/([A-Za-z]+)$/.exec(path)
    if (match === null) return fail(404, 'Not Found')
    if (match[1] !== token) return fail(401, 'Unauthorized')
    return method(match[2], body ?? {}, signal)
  }

  const push = (update) => { state.updates.push({ update_id: state.nextUpdate++, ...update }); wake() }
  const fake = {
    token,
    bot,
    person,
    state,
    /** As fetch: what Locust's link calls. */
    fetch: async (url, init = {}) => {
      const parsed = new URL(url)
      const body = typeof init.body === 'string' && init.body.length > 0 ? JSON.parse(init.body) : {}
      const { status, body: data } = await answer(parsed.pathname, body, init.signal)
      return new Response(JSON.stringify(data), { status, headers: { 'content-type': 'application/json' } })
    },
    /** The person writes. Who it is from, and the chat, default to the person; a stranger is another id. */
    say(text, more = {}) {
      const from = more.from ?? person
      push({ message: { message_id: state.nextMessage++, from, chat: { id: more.chatId ?? from.id, type: more.chatType ?? 'private', first_name: from.first_name }, date: more.date ?? now(), ...(text === undefined ? {} : { text }), ...(more.replyTo === undefined ? {} : { reply_to_message: { message_id: more.replyTo } }) } })
      return state.nextMessage - 1
    },
    /** The person presses a button under one of the bot's messages. */
    press(messageId, data, more = {}) {
      const from = more.from ?? person
      push({ callback_query: { id: `cb${String(state.nextUpdate)}`, from, message: { message_id: messageId, chat: { id: more.chatId ?? from.id, type: 'private' } }, data } })
    },
    /** Resolves once the test holds of the state, or rejects after the time given. */
    async until(test, ms = 5_000, what = 'the fake Telegram to see it') {
      const started = Date.now()
      for (;;) {
        const found = test(state)
        if (found) return found
        if (Date.now() - started > ms) throw new Error(`Timed out waiting for ${what}`)
        await new Promise((resolve) => setTimeout(resolve, 20))
      }
    },
    /** The bot's messages to the chat, as the person reads them. */
    read: () => state.sent.map((one) => one.text),
    listen(port = 0) {
      const server = createServer((req, res) => {
        const chunks = []
        req.on('data', (chunk) => chunks.push(chunk))
        req.on('end', async () => {
          const text = Buffer.concat(chunks).toString('utf8')
          const body = text.length > 0 ? JSON.parse(text) : {}
          const reply = (status, data) => { res.writeHead(status, { 'content-type': 'application/json' }).end(JSON.stringify(data)) }
          const path = new URL(req.url, 'http://fake').pathname
          if (path === '/control/state') return reply(200, { bot, person, ...state })
          if (path === '/control/say') return reply(200, { message_id: fake.say(body.text, body) })
          if (path === '/control/press') { fake.press(body.messageId, body.data, body); return reply(200, { ok: true }) }
          const aborted = new AbortController()
          // The response's close, not the request's: a request closes once its body is read.
          res.on('close', () => { if (!res.writableEnded) aborted.abort() })
          try {
            const { status, body: data } = await answer(path, body, aborted.signal)
            reply(status, data)
          } catch { if (!res.writableEnded) res.destroy() }
        })
      })
      return new Promise((resolve) => server.listen(port, '127.0.0.1', () => resolve(server)))
    }
  }
  return fake
}

if (process.argv[1] !== undefined && fileURLToPath(import.meta.url) === process.argv[1]) {
  const at = process.argv.indexOf('--port')
  const port = at > 0 ? Number(process.argv[at + 1]) : 47811
  const fake = fakeTelegram()
  const server = await fake.listen(port)
  console.log(`fake Telegram on http://127.0.0.1:${String(server.address().port)} token ${fake.token}`)
}
