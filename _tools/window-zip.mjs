// The window, for Locust in a browser (0.830): every file the built app's renderer folder holds, in one zip the relay
// reads a file at a time (apps/relay/src/window-zip.ts). Stored, not compressed -- the relay's own responses are
// compressed on their way out -- and each file's CRC written as a zip has it.
//
//   node _tools/window-zip.mjs <version> [--from <renderer folder>] [--out <zip>]
//
// The default is the build ship.mjs just packaged, apps/desktop/out/renderer, into
// apps/desktop/release/Locust-<version>-window.zip, which publish-downloads.mjs puts beside the installer as
// window/Locust-<version>-window.zip.
import { mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs'
import { dirname, join, relative, sep } from 'node:path'
import { fileURLToPath } from 'node:url'

const CRC_TABLE = (() => {
  const table = new Uint32Array(256)
  for (let n = 0; n < 256; n += 1) {
    let c = n
    for (let k = 0; k < 8; k += 1) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1
    table[n] = c >>> 0
  }
  return table
})()

export function crc32(bytes) {
  let c = 0xffffffff
  for (const byte of bytes) c = CRC_TABLE[(c ^ byte) & 0xff] ^ (c >>> 8)
  return (c ^ 0xffffffff) >>> 0
}

/** Files, `{ path, bytes }`, as one stored zip. Their order is kept; a fixed date, so the same files make the same zip. */
export function writeZip(files) {
  const locals = []
  const centrals = []
  let offset = 0
  for (const file of files) {
    const name = Buffer.from(file.path, 'utf8')
    const data = Buffer.from(file.bytes)
    const crc = crc32(data)
    const local = Buffer.alloc(30)
    local.writeUInt32LE(0x04034b50, 0)
    local.writeUInt16LE(20, 4) // version needed
    local.writeUInt16LE(0x0800, 6) // UTF-8 names
    local.writeUInt16LE(0, 8) // stored
    local.writeUInt16LE(0, 10) // time: 00:00
    local.writeUInt16LE(0x5821, 12) // date: 2024-01-01
    local.writeUInt32LE(crc, 14)
    local.writeUInt32LE(data.length, 18)
    local.writeUInt32LE(data.length, 22)
    local.writeUInt16LE(name.length, 26)
    local.writeUInt16LE(0, 28)
    locals.push(local, name, data)
    const central = Buffer.alloc(46)
    central.writeUInt32LE(0x02014b50, 0)
    central.writeUInt16LE(20, 4)
    central.writeUInt16LE(20, 6)
    central.writeUInt16LE(0x0800, 8)
    central.writeUInt16LE(0, 10)
    central.writeUInt16LE(0, 12)
    central.writeUInt16LE(0x5821, 14)
    central.writeUInt32LE(crc, 16)
    central.writeUInt32LE(data.length, 20)
    central.writeUInt32LE(data.length, 24)
    central.writeUInt16LE(name.length, 28)
    central.writeUInt16LE(0, 30)
    central.writeUInt16LE(0, 32)
    central.writeUInt16LE(0, 34)
    central.writeUInt16LE(0, 36)
    central.writeUInt32LE(0, 38)
    central.writeUInt32LE(offset, 42)
    centrals.push(central, name)
    offset += 30 + name.length + data.length
  }
  const directory = Buffer.concat(centrals)
  const end = Buffer.alloc(22)
  end.writeUInt32LE(0x06054b50, 0)
  end.writeUInt16LE(files.length, 8)
  end.writeUInt16LE(files.length, 10)
  end.writeUInt32LE(directory.length, 12)
  end.writeUInt32LE(offset, 16)
  return Buffer.concat([...locals, directory, end])
}

/** Every file under `folder`, by its path inside it with forward slashes, in a fixed order. */
export function filesUnder(folder) {
  const found = []
  const walk = (at) => {
    for (const name of readdirSync(at).sort()) {
      const path = join(at, name)
      if (statSync(path).isDirectory()) walk(path)
      else found.push({ path: relative(folder, path).split(sep).join('/'), bytes: readFileSync(path) })
    }
  }
  walk(folder)
  return found
}

if (process.argv[1] !== undefined && fileURLToPath(import.meta.url) === process.argv[1]) {
  const version = process.argv[2]
  const flag = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
  if (version === undefined || !/^\d+\.\d+\.\d+$/.test(version)) {
    console.error('usage: node _tools/window-zip.mjs <version> [--from <renderer folder>] [--out <zip>]')
    process.exit(2)
  }
  const root = join(dirname(fileURLToPath(import.meta.url)), '..', 'apps', 'desktop')
  const from = flag('--from') ?? join(root, 'out', 'renderer')
  const out = flag('--out') ?? join(root, 'release', `Locust-${version}-window.zip`)
  const files = filesUnder(from)
  if (!files.some((file) => file.path === 'desk.html')) {
    console.error(`${from} has no desk.html: build the app first (electron-vite build).`)
    process.exit(1)
  }
  mkdirSync(dirname(out), { recursive: true })
  const zip = writeZip(files)
  writeFileSync(out, zip)
  console.log(`Wrote ${out}: ${String(files.length)} files, ${String(Math.round(zip.length / 1024))} KB.`)
}
