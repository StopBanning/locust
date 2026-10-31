// The six density orbs at 26px, shrunk three ways and magnified 8x with no smoothing (2026-10-30): on the graphics
// card (as Locust did through 0.843), in software (as it does since), and the exact 8x average. On the Mac the first is
// 3.2-7.0 out of 255 off the third and the second 0.4-0.7 (Orb.tsx, PaintedDown). Bundle and photograph:
//   esbuild gpu-sheet.js --bundle --format=iife --outfile=gpu-sheet.bundle.js --alias:thinking-orbs/engine=<thinking-orbs dist/engine.es.js>
//   electron gpu-sheet-shot.cjs "$(pwd)" sheet.png
import { resolvePreset, MODE_DRAWS } from 'thinking-orbs/engine'
const STATES = ['composing', 'listening', 'solving', 'searching', 'connecting', 'weaving']
const side = 26, Z = 8, T = 1.9
function painted(state, backing, cpu) {
  const c = document.createElement('canvas'); c.width = backing; c.height = backing
  const ctx = c.getContext('2d', cpu ? { willReadFrequently: true } : {})
  const k = backing / 64; ctx.setTransform(k, 0, 0, k, 0, 0)
  const { mode, opts } = resolvePreset(state, 64); MODE_DRAWS[mode](ctx, 64, T, true, opts); return c
}
function shrink(state, cpu) {
  const big = painted(state, side * 4, cpu)
  const c = document.createElement('canvas'); c.width = side; c.height = side
  const ctx = c.getContext('2d', cpu ? { willReadFrequently: true } : {}); ctx.imageSmoothingEnabled = true; ctx.imageSmoothingQuality = 'high'; ctx.drawImage(big, 0, 0, side, side); return c
}
function ideal(state) {
  const src = painted(state, side * 8, true), d = src.getContext('2d').getImageData(0, 0, side * 8, side * 8).data
  const c = document.createElement('canvas'); c.width = side; c.height = side; const ctx = c.getContext('2d'); const img = ctx.createImageData(side, side)
  for (let y = 0; y < side; y++) for (let x = 0; x < side; x++) { let r = 0, g = 0, b = 0, a = 0
    for (let dy = 0; dy < 8; dy++) for (let dx = 0; dx < 8; dx++) { const i = ((y * 8 + dy) * side * 8 + x * 8 + dx) * 4, al = d[i + 3] / 255; r += d[i] * al; g += d[i + 1] * al; b += d[i + 2] * al; a += al }
    const o = (y * side + x) * 4; img.data[o] = a ? r / a : 0; img.data[o + 1] = a ? g / a : 0; img.data[o + 2] = a ? b / a : 0; img.data[o + 3] = a / 64 * 255 }
  ctx.putImageData(img, 0, 0); return c
}
const sheet = document.getElementById('sheet'); const ctx = sheet.getContext('2d')
const cell = side * Z, gap = 16, left = 110, top = 40
sheet.width = left + 3 * (cell + gap); sheet.height = top + STATES.length * (cell + gap)
ctx.fillStyle = '#121415'; ctx.fillRect(0, 0, sheet.width, sheet.height)
ctx.fillStyle = '#c8ccc8'; ctx.font = '15px -apple-system, sans-serif'
;['Graphics card (to 0.843)', 'Software (since)', 'Perfect'].forEach((t, i) => ctx.fillText(t, left + i * (cell + gap) + 8, 26))
ctx.imageSmoothingEnabled = false
STATES.forEach((state, row) => {
  const y = top + row * (cell + gap)
  ctx.fillStyle = '#c8ccc8'; ctx.fillText(state, 8, y + cell / 2)
  ;[shrink(state, false), shrink(state, true), ideal(state)].forEach((c, i) => ctx.drawImage(c, left + i * (cell + gap), y, cell, cell))
})
window.__done = true
