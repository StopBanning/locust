// For drives of Locust in a browser (0.830): a local stand-in for Locust's downloads, serving one folder with byte
// ranges, as dl.locust.lol does for each release's window zip. A local relay is pointed at it:
//
//   node _tools/serve-window-zip.mjs <folder> [--port 8791]
//   npx wrangler@4 dev --port 8790 --ip 127.0.0.1 --local --var DESK_DOWNLOADS:http://127.0.0.1:8791/
//
// <folder> holds window/Locust-<version>-window.zip (node _tools/window-zip.mjs <version> --out ...).
import { createReadStream, statSync } from 'node:fs'
import { createServer } from 'node:http'
import { join, normalize, sep } from 'node:path'

const folder = process.argv[2]
const port = Number(process.argv.includes('--port') ? process.argv[process.argv.indexOf('--port') + 1] : 8791)
if (folder === undefined) {
  console.error('usage: node _tools/serve-window-zip.mjs <folder> [--port 8791]')
  process.exit(2)
}
createServer((req, res) => {
  const path = normalize(join(folder, decodeURIComponent(new URL(req.url ?? '/', 'http://local').pathname)))
  if (!path.startsWith(normalize(folder) + sep)) { res.writeHead(403).end(); return }
  let size
  try { size = statSync(path).size } catch { res.writeHead(404).end(); return }
  const asked = /^bytes=(\d*)-(\d*)$/.exec(req.headers.range ?? '')
  if (asked === null) {
    res.writeHead(200, { 'content-length': size, 'accept-ranges': 'bytes' })
    createReadStream(path).pipe(res)
    return
  }
  const start = asked[1] === '' ? Math.max(0, size - Number(asked[2])) : Number(asked[1])
  const end = asked[1] === '' ? size - 1 : Math.min(size - 1, asked[2] === '' ? size - 1 : Number(asked[2]))
  if (start > end) { res.writeHead(416, { 'content-range': `bytes */${String(size)}` }).end(); return }
  res.writeHead(206, { 'content-length': end - start + 1, 'content-range': `bytes ${String(start)}-${String(end)}/${String(size)}`, 'accept-ranges': 'bytes' })
  createReadStream(path, { start, end }).pipe(res)
}).listen(port, '127.0.0.1', () => console.log(`Serving ${folder} on http://127.0.0.1:${String(port)}/`))
