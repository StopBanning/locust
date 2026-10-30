import { machineWords } from '../../shared/machine-words.js'

type ChordEvent = Pick<KeyboardEvent, 'ctrlKey' | 'altKey' | 'metaKey' | 'shiftKey' | 'key'>

/**
 * The Changes panel's "show the files" chord: Cmd+Shift+Y on a Mac, Ctrl+Shift+Y everywhere else. It listened for
 * Ctrl alone, so on a Mac it was the one chord that never took Cmd (F24, the first Mac run, 2026-10-30).
 */
export function isShowFilesChord(event: ChordEvent, platform: string | undefined): boolean {
  const mac = platform === 'darwin'
  const modifier = mac ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey
  return modifier && event.shiftKey && !event.altKey && event.key.toLowerCase() === 'y'
}

/** The chord as the button's title says it on this machine. */
export const showFilesChordLabel = (platform: string | undefined): string => `${machineWords(platform).modifier}+Shift+Y`
