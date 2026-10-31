import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { expect, it, vi } from 'vitest'
const source = readFileSync(new URL('./index.ts', import.meta.url), 'utf8')
const start = source.indexOf('    const trayIcon = () =>')
const body = source.slice(start, source.indexOf('    /**', start))
it('on darwin the tray uses the template image marked as a template and elsewhere the old icons', () => {
  for (const platform of ['darwin', 'linux', 'win32']) for (const isPackaged of [true, false]) {
    const image = { setTemplateImage: vi.fn(), resize: vi.fn() }
    const createFromPath = vi.fn(() => image)
    const result = new Function('process', 'app', 'nativeImage', 'join', '__dirname', `${body}; return trayIcon()`)(
      { platform, resourcesPath: '/resources' }, { isPackaged }, { createFromPath }, join, '/dev/out/main')
    const file = platform === 'darwin' ? 'trayTemplate.png' : platform === 'linux' ? 'icon-32.png' : isPackaged ? 'icon.ico' : 'icon-512.png'
    expect(createFromPath).toHaveBeenCalledWith(isPackaged ? join('/resources', file) : join('/dev/resources', file))
    expect(result).toBe(image)
    expect(image.resize).not.toHaveBeenCalled()
    expect(image.setTemplateImage.mock.calls).toEqual(platform === 'darwin' ? [[true]] : [])
  }
  const builder = readFileSync(new URL('../../electron-builder.yml', import.meta.url), 'utf8')
  for (const name of ['trayTemplate.png', 'trayTemplate@2x.png']) {
    expect(builder).toContain(`from: resources/${name}\n    to: ${name}`)
    const png = readFileSync(new URL(`../../resources/${name}`, import.meta.url))
    expect(png.readUInt32BE(16)).toBe(name.includes('@2x') ? 32 : 16)
    expect(png.readUInt32BE(20)).toBe(name.includes('@2x') ? 32 : 16)
  }
})
