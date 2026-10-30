import { machineWords, type MachineWords } from '../../shared/machine-words.js'

/** This window's words for its own computer (0.843): a Mac says Mac, menu bar and log in. */
export const windowWords = (): MachineWords => machineWords(typeof window === 'undefined' ? undefined : window.desktop?.platform)
