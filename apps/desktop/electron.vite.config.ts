import { createRequire } from 'node:module'
import { resolve } from 'node:path'
import { defineConfig, externalizeDepsPlugin } from 'electron-vite'
import react from '@vitejs/plugin-react'
import type { Plugin } from 'vite'

/*
 * LOCUST IN A BROWSER'S WAY IN (0.830, src/renderer/src/desk-remote.ts), as one plain script: desk.html loads it
 * before the window's own module, so window.desktop is there when the window's first line reads it. As a module it
 * was bundled behind the window's own code, which then ran first. esbuild comes with vite.
 */
function deskRemote(): Plugin {
  return {
    name: 'locust-desk-remote',
    apply: 'build',
    async generateBundle() {
      const esbuild = createRequire(createRequire(import.meta.url).resolve('vite/package.json'))('esbuild') as {
        build: (options: Record<string, unknown>) => Promise<{ readonly outputFiles: readonly { readonly text: string }[] }>
      }
      const built = await esbuild.build({
        entryPoints: [resolve('src/renderer/src/desk-remote.ts')],
        bundle: true,
        format: 'iife',
        platform: 'browser',
        target: 'es2022',
        minify: true,
        keepNames: true,
        legalComments: 'none',
        write: false
      })
      this.emitFile({ type: 'asset', fileName: 'desk-remote.js', source: built.outputFiles[0]!.text })
    }
  }
}

export default defineConfig({
  main: {
    plugins: [externalizeDepsPlugin({
      exclude: ['@teammate/runtime-adapters', '@teammate/mission-store']
    })]
  },
  preload: {
    plugins: [externalizeDepsPlugin()],
    build: {
      rollupOptions: {
        output: {
          format: 'cjs',
          entryFileNames: '[name].cjs',
          chunkFileNames: '[name]-[hash].cjs'
        }
      }
    }
  },
  renderer: {
    resolve: {
      alias: {
        '@renderer': resolve('src/renderer/src')
      }
    },
    plugins: [react(), deskRemote()],
    /*
     * MINIFIED, which electron-vite 5 does not do for the renderer by default.
     *
     * The bundle shipped as written: 1.47 MB of JavaScript and 352 KB of CSS,
     * half of it comments, parsed by BOTH windows at every launch -- the
     * loading window and the app behind it -- while the runtime probes were
     * starting on the same cores (measured 2026-09-22: the probes ran ~0.2 s
     * slower once they overlapped the parse). `keepNames` keeps function
     * names in stack traces, so an error a person reports still says where.
     */
    build: {
      minify: 'esbuild',
      cssMinify: true,
      rollupOptions: {
        input: {
          index: resolve('src/renderer/index.html'),
          // The page that opens an attached PDF for the agent (0.713,
          // src/pdfReader.ts): its own entry, so pdf.js is never in the app's
          // bundle and the app never runs in the view that reads.
          pdf: resolve('src/renderer/pdf.html'),
          // The page that draws a diagram in a reply (0.770, src/diagramPage.ts): its own entry too, so Mermaid
          // is never in the app's bundle and the app never runs in the view that draws.
          diagram: resolve('src/renderer/diagram.html'),
          // Locust in a browser (0.830, src/desk-remote.ts): the same window, with its way into Locust through the
          // relay first. Served by the relay from each release's own build, never loaded in this app.
          desk: resolve('src/renderer/desk.html')
        }
      }
    },
    esbuild: {
      keepNames: true,
      // Licence notices kept in the bundle (0.713): a minified bundle dropped
      // every one, PDF.js's Apache notice among them.
      legalComments: 'inline'
    }
  }
})
