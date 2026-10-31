// FILMING A DRIVE (launch clip, 2026-10-31): the window's own frames, as Chrome paints them, saved while a drive does
// what a person does, then stitched at the times they were painted. Every frame is the real app; nothing is redrawn.
//
//   const film = await startFilming(drive, folder)       // after drive.ready()
//   ... the drive's steps ...
//   const frames = await film.stop()
//   await stitch(frames, 'out.mp4')                       // constant 30 fps, H.264, the holds kept as long as they were
//
// Chrome sends a frame only when something on screen changes, so a still screen costs nothing and a frame's duration
// is the gap to the next one. Each frame is acknowledged once written, which is what asks Chrome for the next.
import { spawnSync } from 'node:child_process'
import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

const FFMPEG = process.env.FFMPEG ?? '/opt/homebrew/bin/ffmpeg'

export async function startFilming(drive, folder, { quality = 92, maxWidth = 3840, maxHeight = 2400 } = {}) {
  await mkdir(folder, { recursive: true })
  const frames = []
  let writing = Promise.resolve()
  const stopListening = drive.on((message) => {
    if (message.method !== 'Page.screencastFrame') return
    const { data, metadata, sessionId } = message.params
    const file = join(folder, `f${String(frames.length).padStart(6, '0')}.jpg`)
    frames.push({ file, at: metadata.timestamp })
    writing = writing.then(() => writeFile(file, Buffer.from(data, 'base64'))).then(() => drive.send('Page.screencastFrameAck', { sessionId }))
  })
  await drive.send('Page.startScreencast', { format: 'jpeg', quality, maxWidth, maxHeight, everyNthFrame: 1 })
  return {
    /** Marks the moment in the film's own time, for cutting later: seconds since the first frame. */
    mark: () => (frames.length === 0 ? 0 : Date.now() / 1000 - frames[0].at),
    async stop() {
      await drive.send('Page.stopScreencast')
      await writing
      stopListening()
      // Kept beside the frames, so a take can be cut again later at the times it was painted.
      await writeFile(join(folder, 'frames.json'), `${JSON.stringify(frames)}\n`, 'utf8')
      return frames
    }
  }
}

/** The frames as a constant-rate video: each held until the next was painted, the last for `tail` seconds. */
export async function stitch(frames, out, { fps = 30, tail = 1, from = 0, to = Infinity } = {}) {
  if (frames.length === 0) throw new Error('No frames were painted.')
  const start = frames[0].at
  const kept = frames.filter((frame) => frame.at - start >= from && frame.at - start <= to)
  const lines = []
  for (const [index, frame] of kept.entries()) {
    const next = kept[index + 1]
    // No floor on a frame's time: Chrome paints faster than the film's rate, and the fps filter keeps the right ones.
    lines.push(`file '${frame.file}'`, `duration ${(next === undefined ? tail : Math.max(0.0005, next.at - frame.at)).toFixed(4)}`)
  }
  lines.push(`file '${kept[kept.length - 1].file}'`)
  const list = `${out}.frames.txt`
  await writeFile(list, `${lines.join('\n')}\n`, 'utf8')
  const run = spawnSync(FFMPEG, ['-y', '-loglevel', 'error', '-f', 'concat', '-safe', '0', '-i', list, '-vf', `fps=${String(fps)},scale=trunc(iw/2)*2:trunc(ih/2)*2,format=yuv420p`, '-c:v', 'libx264', '-preset', 'slow', '-crf', '18', '-movflags', '+faststart', out], { encoding: 'utf8' })
  if (run.status !== 0) throw new Error(`ffmpeg: ${run.stderr}`)
  return out
}
