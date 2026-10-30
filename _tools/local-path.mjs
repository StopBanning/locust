// The one place a tool turns its own file URL into a path. It has no side effects, so any script may import it.
import { fileURLToPath } from 'node:url'

/**
 * A file URL as a path on this machine. On Windows, as it always was (`/C:/x` less its slash); elsewhere a URL's
 * pathname IS the path, and slicing it made `/Users/x` the relative `Users/x` (the first Mac run, 2026-10-30).
 */
export const localPath = (url) => (process.platform === 'win32' ? url.pathname.slice(1) : fileURLToPath(url))
