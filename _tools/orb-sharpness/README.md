# Orb sharpness at 26px

How the six density orbs reach the live line's 26px box, measured against
the best picture 26 device pixels can hold (the same frame drawn 8x larger
and averaged down exactly).

```
cd _tools/orb-sharpness
node ../../node_modules/.bin/esbuild orb-compare.js --bundle --format=iife \
  --outfile=orb-compare.bundle.js \
  --alias:thinking-orbs/engine=<repo>/node_modules/.pnpm/thinking-orbs@0.3.1_react@19.2.8/node_modules/thinking-orbs/dist/engine.es.js
../../apps/desktop/node_modules/electron/dist/electron.exe orbshot.cjs "$(pwd)" 0.7,1.3,1.9,2.5,3.1,3.7,4.3,4.9
python score.py        # error vs ideal, per orb and method
python sheet.py 1.3    # 6x contact sheet of one moment
```

Result, 2026-09-22, DPR 1 (Colin's display):

| Method | Mean error vs ideal | Frame-to-frame swing |
|---|---|---|
| Library canvas (64) shrunk to 26 by CSS -- shipped through 0.270 | 10.4 | up to 1.3 |
| Engine drawn straight at 26 | 17.7 | -- (sub-pixel dots painted fat) |
| Drawn at exactly 2x, CSS shrink | 5.3 | -- |
| Drawn at 4x, shrunk with `imageSmoothingQuality = 'high'` | **0.9** | **0.08** |

The contact sheet is `docs/orb-sharpness-2026-09-22.png`. The last row's
method is on branch `orb-paint-down` (`PaintedDown` in `Orb.tsx`), not on
main, until Colin has looked at it in the app.

## The Mac, 2026-10-30

On macOS the same 4x paint-down came out 3.2 to 7.0 off the ideal, because Chromium on the graphics card does not
honour `imageSmoothingQuality = 'high'` for a 4:1 shrink there. `PaintedDown` now keeps both canvases in software
(`willReadFrequently`), where it is 0.4 to 0.7 on the Mac. `gpu-sheet.js` draws the three side by side on any
system (its first lines say how), and `_tools/look-orbs.mjs` photographs the live orb in the app.
