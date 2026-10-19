// Packaged Windows terminal: real keyboard input, screen text, hide/reopen, tab close, and graceful-quit process cleanup.
// No agent turn, sign-in, global configuration or spending. Fails on the base build (there is no toggle).
import { execFileSync } from 'node:child_process'
import { writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import assert from 'node:assert/strict'
import { startDrive, scratchRepository, sleep } from './drive-lib.mjs'
const arg = name => process.argv[process.argv.indexOf(name) + 1]
const packaged = process.argv.includes('--packaged') ? arg('--packaged') : undefined
assert.equal(process.platform, 'win32', 'this drive checks Windows PowerShell and Windows process trees')
const workspace = await scratchRepository('locust-terminal-ws-')
const drive = await startDrive({ name: 'terminal-panel', port: 9883, workspace, sendsNothing: true, focused: true, ...(packaged ? { packaged } : {}),
  seed: { schemaVersion: 1, teammates: [{ teammateId: 'tm_test', name: 'Test', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-10-18T00:00:00.000Z' }], missionOwners: {}, settings: { swarm: false, relay: false, memoryMode: 'off' } } })
const checks = []
const check = (claim, actual) => { checks.push({ claim, actual }); assert.ok(actual, claim); console.log(`PASS: ${claim}`) }
const screen = `document.querySelector('.lc-terminal__screen:not([hidden]) .xterm-accessibility-tree')?.innerText ?? ''`
const command = async text => {
  await drive.evaluate(`document.querySelector('.lc-terminal__screen:not([hidden]) .xterm-helper-textarea')?.focus()`)
  for (const character of text) await drive.send('Input.dispatchKeyEvent', { type: 'char', text: character, unmodifiedText: character })
  await drive.send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13 })
  await drive.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13 })
}
const shortcut = async () => {
  await drive.send('Input.dispatchKeyEvent', { type: 'keyDown', key: '`', code: 'Backquote', modifiers: 2, windowsVirtualKeyCode: 192 })
  await drive.send('Input.dispatchKeyEvent', { type: 'keyUp', key: '`', code: 'Backquote', modifiers: 2, windowsVirtualKeyCode: 192 })
}
const processes = () => JSON.parse(execFileSync('powershell.exe', ['-NoProfile', '-Command', 'ConvertTo-Json -Compress -InputObject @(Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,Name)'], { encoding: 'utf8', windowsHide: true }))
const descendants = (rows, root) => {
  const ids = new Set([root]); let added = true
  while (added) { added = false; for (const row of rows) if (ids.has(row.ParentProcessId) && !ids.has(row.ProcessId)) { ids.add(row.ProcessId); added = true } }
  return rows.filter(row => ids.has(row.ProcessId))
}
try {
  await drive.ready()
  await drive.resize(1200, 720)
  if (process.argv.includes('--without-toggle')) await drive.evaluate(`document.querySelector('button[aria-label="Terminal"]')?.remove()`)
  check('Terminal is available before a message', await drive.evaluate(`Boolean(document.querySelector('button[aria-label="Terminal"]'))`))
  await drive.evaluate(`document.querySelector('button[aria-label="Terminal"]').click()`)
  await drive.waitFor(`Boolean(document.querySelector('.lc-terminal__screen .xterm-helper-textarea'))`, { timeoutMs: 20000, what: 'first terminal' })
  await sleep(1800)
  await command("Write-Output hello; Write-Output ('LC_PID=' + $PID); Write-Output ('LC_FOLDER=' + $PWD.Path)")
  await drive.waitFor(`(${screen}).split(/\\n/).some(line => line.trim() === 'hello')`, { timeoutMs: 15000, what: 'hello output line on screen' })
  const text = String(await drive.evaluate(screen))
  check('Write-Output hello prints hello on screen', text.split('\n').some(line => line.trim() === 'hello'))
  check('the shell is in the conversation folder', text.replace(/\\/g, '/').replace(/\s/g, '').includes(workspace.replace(/\\/g, '/').replace(/\s/g, '')))
  const pid = Number(/LC_PID=(\d+)/.exec(text)?.[1])
  check('the screen identifies its real shell process', pid > 0)
  await drive.capture('PowerShell prints hello', async () => text)
  const width = await drive.evaluate(`document.querySelector('.xterm-screen').getBoundingClientRect().width`)
  await drive.resize(1440, 900)
  await sleep(300)
  check('xterm fits the resized panel', Number(await drive.evaluate(`document.querySelector('.xterm-screen').getBoundingClientRect().width`)) > Number(width))
  await drive.resize(1200, 720)
  await drive.evaluate(`window.desktop.setTheme('light')`)
  await drive.waitFor(`document.documentElement.dataset.theme === 'light'`, { timeoutMs: 5000, what: 'light theme' })
  await sleep(300)
  await drive.capture('terminal follows the light theme', async () => drive.evaluate(`getComputedStyle(document.querySelector('.lc-terminal')).backgroundColor`))
  check('terminal remains readable with retained text in light theme', String(await drive.evaluate(screen)).includes('LC_PID=' + pid))
  await drive.evaluate(`window.desktop.setTheme('dark')`)
  const prior = new Set(descendants(processes(), drive.pid).map(row => row.ProcessId))
  await drive.evaluate(`(() => { const picker = document.querySelector('[aria-label="New terminal with shell"]'); picker.value = 'cmd'; picker.dispatchEvent(new Event('change', { bubbles: true })); })()`)
  await drive.waitFor(`document.querySelectorAll('.lc-terminal__screen').length === 2`, { timeoutMs: 10000, what: 'Command Prompt tab' })
  await sleep(800)
  await command('echo cmd-tab-ok')
  await drive.waitFor(`(${screen}).split(/\\n/).some(line => line.trim() === 'cmd-tab-ok')`, { timeoutMs: 10000, what: 'Command Prompt output' })
  const cmd = descendants(processes(), drive.pid).find(row => row.Name.toLowerCase() === 'cmd.exe' && !prior.has(row.ProcessId))
  check('the shell menu opens a working second tab', cmd !== undefined)
  await drive.evaluate(`document.querySelector('[aria-label="Close Command Prompt terminal"]').click()`)
  await drive.waitFor(`document.querySelectorAll('.lc-terminal__screen').length === 1`, { timeoutMs: 10000, what: 'second tab closed' })
  check('closing the second tab leaves the first shell intact', !processes().some(row => row.ProcessId === cmd.ProcessId) && processes().some(row => row.ProcessId === pid))
  await drive.evaluate(`document.querySelector('.lc-terminal__screen:not([hidden]) .xterm-helper-textarea').focus()`)
  await shortcut()
  const shortcutHidden = await drive.waitFor(`Boolean(document.querySelector('.lc-terminal[hidden]'))`, { timeoutMs: 5000, what: 'shortcut hides the focused terminal' })
  check('Ctrl backtick also hides a focused terminal', shortcutHidden)
  await shortcut()
  await drive.waitFor(`Boolean(document.querySelector('.lc-terminal:not([hidden])'))`, { timeoutMs: 5000, what: 'shortcut reopens the terminal' })
  await drive.evaluate(`document.querySelector('[aria-label="Hide terminal"]').click()`)
  check('hiding the panel leaves the shell alive', processes().some(row => row.ProcessId === pid))
  await shortcut()
  await drive.waitFor(`Boolean(document.querySelector('.lc-terminal:not([hidden])'))`, { timeoutMs: 5000, what: 'keyboard toggle' })
  check('Ctrl backtick restores the same tab and scrollback', String(await drive.evaluate(screen)).includes('LC_PID=' + pid))
  await drive.evaluate(`document.querySelector('[aria-label^="Close PowerShell"], [aria-label^="Close Windows PowerShell"]').click()`)
  await drive.waitFor(`document.querySelectorAll('.lc-terminal__screen').length === 0`, { timeoutMs: 10000, what: 'closed tab' })
  check('closing the tab ends its real shell', !processes().some(row => row.ProcessId === pid))
  await drive.evaluate(`document.querySelector('[aria-label="New terminal"]').click()`)
  await drive.waitFor(`Boolean(document.querySelector('.lc-terminal__screen .xterm-helper-textarea'))`, { timeoutMs: 15000, what: 'second terminal' })
  await sleep(1500)
  await command("$p = Start-Process -WindowStyle Hidden -FilePath $env:ComSpec -ArgumentList '/d','/c','ping -n 300 127.0.0.1 > nul' -PassThru; Write-Output ('LC_CHILD=' + $p.Id)")
  await drive.waitFor(`/LC_CHILD=\\d+/.test(${screen})`, { timeoutMs: 15000, what: 'background child pid' })
  const child = Number(/LC_CHILD=(\d+)/.exec(String(await drive.evaluate(screen)))?.[1])
  const owned = descendants(processes(), drive.pid)
  check('the shell started a real background process', owned.some(row => row.ProcessId === child))
  await drive.capture('terminal with a background child before quitting', async () => JSON.stringify(owned))
  await drive.evaluate(`window.desktop.close()`).catch(() => undefined)
  for (let i = 0; i < 40 && processes().some(row => row.ProcessId === drive.pid); i++) await sleep(250)
  const remaining = processes().filter(row => owned.some(one => one.ProcessId === row.ProcessId))
  check('ordinary app quit leaves no shell, child, or app helper behind', remaining.length === 0)
} catch (error) {
  await drive.capture('failure details', async () => JSON.stringify({ screen: await drive.evaluate(screen), html: await drive.evaluate(`document.querySelector('.lc-terminal')?.outerHTML`) }))
  throw error
} finally {
  await writeFile(join(drive.out, 'checks.json'), JSON.stringify(checks, null, 2))
  await drive.finish({ intro: 'Packaged terminal panel. No model turn or spending.', extra: JSON.stringify(checks, null, 2) })
}
