import { describe, expect, it, vi } from 'vitest'
import { fileURLToPath } from 'node:url'
import type { IPty } from '@lydell/node-pty'
import { createTerminalHost, parseWslDistributions, terminalShells, wslShells } from './terminal-host.js'

const regAnswer = (names: readonly string[]): string => names.map((name, i) => [
  `HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Lxss\\{guid-${String(i)}}`,
  `    DistributionName    REG_SZ    ${name}`,
  '    BasePath    REG_SZ    C:\\Users\\person\\AppData\\Local\\Packages\\distro'
].join('\r\n')).join('\r\n\r\n')

const wslOf = (output: string) => terminalShells('win32', {}, () => output).filter(shell => shell.id.startsWith('wsl:'))

describe('the terminal lists WSL by distribution', () => {
  it('lists none when there are no distributions, and none when the registry read fails', () => {
    expect(parseWslDistributions('')).toEqual([])
    expect(wslOf('')).toEqual([])
    expect(terminalShells('win32', {}, () => { throw new Error('no such key') }).filter(shell => shell.id.startsWith('wsl'))).toEqual([])
    expect(terminalShells('win32', {}, () => '').some(shell => shell.id === 'wsl')).toBe(false)
  })

  // wsl.exe's own path is read off a Windows host's System32 folder and joined by Windows' rules.
  it.skipIf(process.platform !== 'win32')('lists one distribution by name, started with wsl.exe -d <name>', () => {
    expect(wslOf(regAnswer(['Ubuntu']))).toEqual([
      { id: 'wsl:Ubuntu', name: 'Ubuntu (WSL)', file: 'C:\\Windows\\System32\\wsl.exe', args: ['-d', 'Ubuntu'] }
    ])
  })

  it('lists several distributions, including a name with a space', () => {
    const shells = wslOf(regAnswer(['Ubuntu', 'Debian', 'My Distro 22.04']))
    expect(shells.map(shell => shell.name)).toEqual(['Ubuntu (WSL)', 'Debian (WSL)', 'My Distro 22.04 (WSL)'])
    expect(shells.map(shell => shell.args)).toEqual([['-d', 'Ubuntu'], ['-d', 'Debian'], ['-d', 'My Distro 22.04']])
    expect(parseWslDistributions(regAnswer(['My Distro 22.04']))).toEqual(['My Distro 22.04'])
    expect(wslShells(['My Distro 22.04'], 'wsl.exe')[0]?.id).toBe('wsl:My Distro 22.04')
  })

  // The same: it starts wsl.exe from a Windows host's System32 folder.
  it.skipIf(process.platform !== 'win32')('the shell id the window sends still chooses only from what the host listed', async () => {
    const folder = fileURLToPath(new URL('.', import.meta.url))
    const pty = { pid: 123, write: vi.fn(), resize: vi.fn(), kill: vi.fn(), pause: vi.fn(), resume: vi.fn(),
      onData: vi.fn(() => ({ dispose() {} })), onExit: vi.fn(() => ({ dispose() {} })) } as unknown as IPty
    const spawn = vi.fn(async () => pty)
    const host = createTerminalHost({
      folder: async () => folder, refusal: () => undefined, emit: vi.fn(), spawn, end: async () => undefined,
      shells: () => terminalShells('win32', {}, () => regAnswer(['Ubuntu']))
    })
    expect(host.shells()).toContainEqual({ id: 'wsl:Ubuntu', name: 'Ubuntu (WSL)' })
    const tab = await host.create({}, 'wsl:Ubuntu')
    expect(tab.shell).toBe('Ubuntu (WSL)')
    expect(spawn).toHaveBeenCalledWith('C:\\Windows\\System32\\wsl.exe', ['-d', 'Ubuntu'], expect.objectContaining({ cwd: folder }))
    await expect(host.create({}, 'wsl:Debian')).rejects.toThrow('unavailable')
    await expect(host.create({}, 'wsl')).rejects.toThrow('unavailable')
    expect(spawn).toHaveBeenCalledTimes(1)
    await host.dispose()
  })
})
