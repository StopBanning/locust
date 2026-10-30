/**
 * Start Locust when you sign in to Windows, or log in to a Mac.
 *
 * Set with `app.setLoginItemSettings({ openAtLogin, path: process.execPath,
 * args: ['--background'] })`. Read back from `app.getLoginItemSettings()`
 * every time Settings opens, so the switch shows what Windows holds.
 *
 * ON A MAC (0.843, the first Mac run): a login item is the app itself, with no
 * path or arguments to give (macOS 13's SMAppService), so Locust opens with its
 * window. macOS may hold it until the person allows it in System Settings >
 * General > Login Items: that is read back as `approval`, so the switch can say so.
 *
 * Packaged builds only: in development it would register electron.exe.
 */

export const BACKGROUND_ARG = '--background'

export interface LoginItemQuery {
  readonly path: string
  readonly args: string[]
}

/** The slice of Electron's `app` this module needs. Injected so a test can fake it. */
export interface LoginItemHost {
  setLoginItemSettings(settings: { openAtLogin: boolean; path?: string; args?: string[] }): void
  getLoginItemSettings(query?: LoginItemQuery): { readonly openAtLogin: boolean; readonly status?: string }
}

export interface LoginItemOptions {
  readonly host: LoginItemHost
  /** A packaged build. Development must not register electron.exe. */
  readonly packaged: boolean
  readonly execPath: string
  readonly platform: string
}

export interface LoginItemState {
  readonly openAtLogin: boolean
  /** False in development, and off Windows and a Mac: the switch registers nothing. */
  readonly available: boolean
  /** Which of those it is (0.587), so the switch's note can say so. */
  readonly why?: 'development' | 'platform'
  /** A Mac waits for the person to allow it under System Settings > General > Login Items. */
  readonly approval?: true
}

export interface LoginItem {
  read(): LoginItemState
  /** Writes, then reads back, so the answer is what the host reports. */
  set(openAtLogin: boolean): LoginItemState
}

const registers = (platform: string): boolean => platform === 'win32' || platform === 'darwin'

/** Off, and why (0.587): a development copy would register Electron; elsewhere there is nothing to register yet. */
const unavailableFor = (platform: string): LoginItemState => ({ openAtLogin: false, available: false, why: registers(platform) ? 'development' : 'platform' })

export function createLoginItem(options: LoginItemOptions): LoginItem {
  const mac = options.platform === 'darwin'
  const available = options.packaged && registers(options.platform)
  const query = (): LoginItemQuery => ({ path: options.execPath, args: [BACKGROUND_ARG] })
  return {
    read() {
      if (!available) return unavailableFor(options.platform)
      if (!mac) return { openAtLogin: options.host.getLoginItemSettings(query()).openAtLogin === true, available: true }
      const held = options.host.getLoginItemSettings()
      return { openAtLogin: held.openAtLogin === true || held.status === 'enabled', available: true, ...(held.status === 'requires-approval' ? { approval: true as const } : {}) }
    },
    set(openAtLogin) {
      if (!available) return unavailableFor(options.platform)
      options.host.setLoginItemSettings(mac ? { openAtLogin } : { openAtLogin, ...query() })
      return this.read()
    }
  }
}
