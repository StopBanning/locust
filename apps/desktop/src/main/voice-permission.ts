/**
 * Where the window may have the microphone at all: Windows, and a Mac (the first Mac run, 2026-10-30). It was
 * Windows only, so on a Mac Talk and Your OpenAI account's voice typing were refused the microphone they ask for.
 */
export const microphoneRunsOn = (platform: string): boolean => platform === 'win32' || platform === 'darwin'

/** Deny unknown media, video, subframes, and every other permission. */
export function voicePermission(ownWindow: boolean, permission: string, mainFrame: boolean, media: readonly string[]): boolean {
  return ownWindow && mainFrame && permission === 'media' && media.length > 0 && media.every((type) => type === 'audio')
}
