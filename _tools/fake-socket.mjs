// A WEBSOCKET FOR LOCUST'S STAND-INS (0.832). Discord's gateway and Slack's Socket Mode are WebSockets, so their
// stand-ins (fake-discord.mjs, fake-slack.mjs) need one too: in memory for a test, as a class shaped like the WebSocket
// Locust's main process uses (Node's, the browser's shape), and on a port for a drive -- the server's half of RFC 6455,
// text frames only, which is all JSON needs. Either way a stand-in talks through one `Connection` per client.
import { createHash } from 'node:crypto'

/** The server's end of one connection: what a stand-in reads and writes. */
export class Connection {
  constructor(url) {
    this.url = url
    this.closed = false
    this.listeners = { open: [], message: [], close: [] }
  }
  on(event, listener) { this.listeners[event].push(listener) }
  emit(event, ...args) { for (const listener of [...this.listeners[event]]) listener(...args) }
  /** Replaced by the side that carries it. */
  send() {}
  close() {}
}

/**
 * A WebSocket class whose every connection is handed to `accept`, in memory. `accept` returning false refuses it, as
 * a server that will not upgrade does.
 */
export function memorySocketClass(accept) {
  return class MemorySocket {
    static CONNECTING = 0
    static OPEN = 1
    static CLOSING = 2
    static CLOSED = 3
    constructor(url) {
      this.url = String(url)
      this.readyState = 0
      this.onopen = null
      this.onmessage = null
      this.onclose = null
      this.onerror = null
      this.listeners = new Map()
      const conn = new Connection(this.url)
      this.conn = conn
      conn.send = (text) => {
        if (this.readyState !== 1) return
        setTimeout(() => { if (this.readyState === 1) this.fire('message', { data: String(text) }) }, 0)
      }
      conn.close = (code = 1000, reason = '') => {
        if (conn.closed) return
        conn.closed = true
        this.readyState = 3
        setTimeout(() => { this.fire('close', { code, reason, wasClean: true }); conn.emit('close', code, reason) }, 0)
      }
      setTimeout(() => {
        if (this.readyState !== 0) return
        let taken = true
        try { taken = accept(conn) !== false } catch { taken = false }
        if (!taken) {
          this.readyState = 3
          conn.closed = true
          this.fire('error', {})
          this.fire('close', { code: 1006, reason: '', wasClean: false })
          return
        }
        this.readyState = 1
        this.fire('open', {})
        conn.emit('open')
      }, 0)
    }
    send(data) {
      if (this.readyState !== 1) throw new Error('InvalidStateError: the socket is not open')
      const text = String(data)
      setTimeout(() => { if (!this.conn.closed) this.conn.emit('message', text) }, 0)
    }
    close(code = 1000, reason = '') {
      if (this.readyState >= 2) return
      this.readyState = 2
      setTimeout(() => {
        this.readyState = 3
        if (!this.conn.closed) { this.conn.closed = true; this.conn.emit('close', code, reason) }
        this.fire('close', { code, reason, wasClean: true })
      }, 0)
    }
    addEventListener(type, listener) {
      if (!this.listeners.has(type)) this.listeners.set(type, new Set())
      this.listeners.get(type).add(listener)
    }
    removeEventListener(type, listener) { this.listeners.get(type)?.delete(listener) }
    fire(type, event) {
      this[`on${type}`]?.(event)
      for (const listener of this.listeners.get(type) ?? []) listener(event)
    }
  }
}

const frame = (opcode, payload) => {
  const length = payload.length
  let head
  if (length < 126) head = Buffer.from([0x80 | opcode, length])
  else if (length < 65_536) head = Buffer.from([0x80 | opcode, 126, length >> 8, length & 0xff])
  else {
    head = Buffer.alloc(10)
    head[0] = 0x80 | opcode
    head[1] = 127
    head.writeBigUInt64BE(BigInt(length), 2)
  }
  return Buffer.concat([head, payload])
}

/** Takes WebSocket upgrades on an HTTP server: each becomes a Connection handed to `accept(conn, request)`. */
export function acceptUpgrades(server, accept) {
  server.on('upgrade', (request, socket) => {
    const key = request.headers['sec-websocket-key']
    if (typeof key !== 'string' || String(request.headers.upgrade ?? '').toLowerCase() !== 'websocket') { socket.destroy(); return }
    const conn = new Connection(new URL(request.url ?? '/', 'http://127.0.0.1').toString())
    conn.send = (text) => { if (!conn.closed && !socket.destroyed) socket.write(frame(0x1, Buffer.from(String(text), 'utf8'))) }
    conn.close = (code = 1000, reason = '') => {
      if (conn.closed) return
      conn.closed = true
      const payload = Buffer.alloc(2 + Buffer.byteLength(reason))
      payload.writeUInt16BE(code, 0)
      payload.write(reason, 2)
      if (!socket.destroyed) { socket.write(frame(0x8, payload)); socket.end() }
      conn.emit('close', code, reason)
    }
    let taken = true
    try { taken = accept(conn, request) !== false } catch { taken = false }
    if (!taken) { socket.end('HTTP/1.1 401 Unauthorized\r\nConnection: close\r\n\r\n'); return }
    const answer = createHash('sha1').update(`${key}258EAFA5-E914-47DA-95CA-C5AB0DC85B11`).digest('base64')
    socket.write(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${answer}\r\n\r\n`)
    let buffer = Buffer.alloc(0)
    let pieces = []
    socket.on('data', (chunk) => {
      buffer = Buffer.concat([buffer, chunk])
      while (buffer.length >= 2) {
        const fin = (buffer[0] & 0x80) !== 0
        const opcode = buffer[0] & 0x0f
        const masked = (buffer[1] & 0x80) !== 0
        let length = buffer[1] & 0x7f
        let at = 2
        if (length === 126) { if (buffer.length < 4) return; length = buffer.readUInt16BE(2); at = 4 }
        else if (length === 127) { if (buffer.length < 10) return; length = Number(buffer.readBigUInt64BE(2)); at = 10 }
        const maskAt = at
        if (masked) at += 4
        if (buffer.length < at + length) return
        let payload = buffer.subarray(at, at + length)
        if (masked) {
          const mask = buffer.subarray(maskAt, maskAt + 4)
          payload = Buffer.from(payload.map((byte, index) => byte ^ mask[index % 4]))
        }
        buffer = buffer.subarray(at + length)
        if (opcode === 0x8) {
          const code = payload.length >= 2 ? payload.readUInt16BE(0) : 1005
          if (!conn.closed) {
            conn.closed = true
            if (!socket.destroyed) { socket.write(frame(0x8, payload.subarray(0, 2))); socket.end() }
            conn.emit('close', code, payload.subarray(2).toString('utf8'))
          }
          return
        }
        if (opcode === 0x9) { socket.write(frame(0xa, payload)); continue }
        if (opcode === 0xa) continue
        pieces.push(payload)
        if (fin) {
          const text = Buffer.concat(pieces).toString('utf8')
          pieces = []
          conn.emit('message', text)
        }
      }
    })
    socket.on('close', () => { if (!conn.closed) { conn.closed = true; conn.emit('close', 1006, '') } })
    socket.on('error', () => undefined)
    setImmediate(() => conn.emit('open'))
  })
}
