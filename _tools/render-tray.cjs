// Rebuild from Locust's own mark; no additional image dependencies.
// pnpm --filter @teammate/desktop exec electron ../../_tools/render-tray.cjs
const { app, nativeImage } = require('electron')
const { join } = require('node:path')
const { writeFileSync } = require('node:fs')
app.disableHardwareAcceleration()
app.whenReady().then(() => {
  const resources = join(__dirname, '../apps/desktop/resources')
  const source = nativeImage.createFromPath(join(resources, 'icon-512.png'))
  const { width, height } = source.getSize()
  const pixels = source.toBitmap()
  // Separate the pale insect from the dark tile, then fill enclosed wing
  // details: those tiny holes turn to noise at menu-bar size.
  const mark = new Uint8Array(width * height)
  for (let i = 0; i < mark.length; i++) mark[i] = pixels[i * 4] > 100 ? 1 : 0
  const outside = new Uint8Array(mark.length)
  const queue = []
  const visit = (i) => { if (!mark[i] && !outside[i]) { outside[i] = 1; queue.push(i) } }
  for (let x = 0; x < width; x++) { visit(x); visit((height - 1) * width + x) }
  for (let y = 0; y < height; y++) { visit(y * width); visit(y * width + width - 1) }
  for (let n = 0; n < queue.length; n++) {
    const i = queue[n], x = i % width, y = Math.floor(i / width)
    if (x) visit(i - 1)
    if (x < width - 1) visit(i + 1)
    if (y) visit(i - width)
    if (y < height - 1) visit(i + width)
  }
  const bitmap = Buffer.alloc(pixels.length)
  for (let i = 0; i < mark.length; i++) bitmap[i * 4 + 3] = outside[i] ? 0 : 255
  const silhouette = nativeImage.createFromBitmap(bitmap, { width, height })
  for (const size of [16, 32]) {
    const small = silhouette.resize({ width: size, height: size, quality: 'best' })
    writeFileSync(join(resources, size === 16 ? 'trayTemplate.png' : 'trayTemplate@2x.png'), small.toPNG())
  }
  app.quit()
})
