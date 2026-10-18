import { readFileSync } from 'node:fs'
import { posix } from 'node:path'

/**
 * HOW THIS LINUX COPY WAS INSTALLED (0.789), which decides how it updates.
 *
 * An AppImage is one file the person keeps where they like; the AppImage
 * runtime names it in $APPIMAGE, and electron-updater replaces that file with
 * the new one, as the Windows installer does its folder. A .deb or an .rpm
 * belongs to the system's package manager: replacing it takes the person's
 * password, so Locust says a new version is out and hands them the new
 * package to open, the way Ubuntu's App Center installs one. electron-builder
 * marks those two with a `package-type` file beside the app's archive.
 *
 * Anything else -- the unpacked folder a test or a drive runs -- is not an
 * installed copy, and changes nothing on the machine, like a Windows copy
 * without its uninstaller (index.ts, 0.507).
 */
export type LinuxInstall = 'appimage' | 'deb' | 'rpm'

export function linuxInstallOf(
  env: Readonly<Record<string, string | undefined>>,
  resourcesPath: string,
  read: (file: string) => string = (file) => readFileSync(file, 'utf8')
): LinuxInstall | undefined {
  if (typeof env.APPIMAGE === 'string' && env.APPIMAGE.length > 0) return 'appimage'
  try {
    const type = read(posix.join(resourcesPath, 'package-type')).trim()
    return type === 'deb' || type === 'rpm' ? type : undefined
  } catch {
    return undefined
  }
}

/** The package a .deb or .rpm copy is handed for `version`, from the feed's own file list. */
export function linuxPackageUrl(
  install: 'deb' | 'rpm',
  files: readonly { readonly url?: unknown }[] | undefined,
  downloads: string
): string | undefined {
  const name = (files ?? []).map((file) => file.url).find((url): url is string => typeof url === 'string' && url.endsWith(`.${install}`))
  if (name === undefined) return undefined
  try {
    return new URL(name, downloads).toString()
  } catch {
    return undefined
  }
}

/**
 * The Linux CI's check that an AppImage replaces itself (linux-update-smoke.mjs):
 * `LOCUST_LINUX_UPDATE_TEST_FEED` points an AppImage copy at a local generic feed,
 * and `LOCUST_LINUX_UPDATE_TEST_VERSION` names the version the copy electron-updater
 * starts after the swap (`APPIMAGE_SILENT_INSTALL=true`) reports. Never set by the app.
 */
export interface LinuxUpdateTest {
  readonly feedUrl?: string
  readonly version?: string
}

export function linuxUpdateTestOf(
  install: LinuxInstall | undefined,
  env: Readonly<Record<string, string | undefined>>
): LinuxUpdateTest | undefined {
  if (install !== 'appimage') return undefined
  // On this machine only (0.790): an update feed is where a copy takes its next program from, so a variable set
  // anywhere else -- a profile, a launcher -- must never point an installed AppImage at someone else's server.
  const feedUrl = typeof env.LOCUST_LINUX_UPDATE_TEST_FEED === 'string' && onThisMachine(env.LOCUST_LINUX_UPDATE_TEST_FEED)
    ? env.LOCUST_LINUX_UPDATE_TEST_FEED
    : undefined
  const version = env.APPIMAGE_SILENT_INSTALL === 'true' && typeof env.LOCUST_LINUX_UPDATE_TEST_VERSION === 'string' && env.LOCUST_LINUX_UPDATE_TEST_VERSION.length > 0
    ? env.LOCUST_LINUX_UPDATE_TEST_VERSION
    : undefined
  if (feedUrl === undefined && version === undefined) return undefined
  return {
    ...(feedUrl === undefined ? {} : { feedUrl }),
    ...(version === undefined ? {} : { version })
  }
}

/** An http address on this machine's loopback, the only kind of test feed an AppImage copy will read. */
function onThisMachine(address: string): boolean {
  try {
    const url = new URL(address)
    return url.protocol === 'http:' && (url.hostname === '127.0.0.1' || url.hostname === 'localhost') && url.username === '' && url.password === ''
  } catch {
    return false
  }
}
