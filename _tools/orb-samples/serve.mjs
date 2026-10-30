// A two-file static server, because the browser tool cannot open file:// and
// vite would not start from this workspace root.
import { createServer } from 'node:http'
import { readFileSync } from 'node:fs'
import { localPath } from '../local-path.mjs'

const HERE = localPath(new URL('.', import.meta.url))
const TYPES = { '.html': 'text/html', '.js': 'text/javascript' }

createServer((request, response) => {
  const name = request.url === '/' ? 'index.html' : request.url.slice(1).split('?')[0]
  try {
    const body = readFileSync(HERE + name)
    response.writeHead(200, { 'content-type': TYPES[name.slice(name.lastIndexOf('.'))] ?? 'text/plain' })
    response.end(body)
  } catch {
    response.writeHead(404).end('no')
  }
}).listen(5199, '127.0.0.1', () => console.log('orb samples on http://127.0.0.1:5199/'))
