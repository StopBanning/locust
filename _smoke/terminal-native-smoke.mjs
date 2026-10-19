// Load the PACKAGED module under its own Electron, then run a real pseudo-terminal. No model or account.
// node _smoke/terminal-native-smoke.mjs <packaged executable>
import { spawnSync } from 'node:child_process'
import { dirname, join, resolve } from 'node:path'
import assert from 'node:assert/strict'

const executable = process.argv[2] === undefined ? undefined : resolve(process.argv[2])
assert.ok(executable, 'a packaged executable is required')
const asar = process.platform === 'darwin' ? join(dirname(executable), '..', 'Resources', 'app.asar') : join(dirname(executable), 'resources', 'app.asar')
const code = `
const { createRequire } = require('node:module');
const pty = createRequire(process.argv[1] + '/package.json')('@lydell/node-pty');
const win = process.platform === 'win32';
const child = pty.spawn(win ? process.env.ComSpec : '/bin/sh', win ? ['/d', '/c', 'echo LOCUST_PTY_SMOKE'] : ['-c', 'printf LOCUST_PTY_SMOKE'], { cwd: process.cwd(), env: { ...process.env, TERM: 'xterm-256color' }, cols: 80, rows: 24 });
let output = '';
const timer = setTimeout(() => { child.kill(); console.error('PTY timed out: ' + output); process.exit(1) }, 15000);
child.onData(data => output += data);
child.onExit(({ exitCode }) => { clearTimeout(timer); try { child.kill() } catch {} if (exitCode !== 0 || !output.includes('LOCUST_PTY_SMOKE')) { console.error({ exitCode, output }); process.exit(1) } process.stdout.write('PACKAGED PTY PASS: ' + process.platform + '-' + process.arch + '\\n', () => process.exit(0)); });
`
const result = spawnSync(executable, ['-e', code, asar], { env: { ...process.env, ELECTRON_RUN_AS_NODE: '1' }, encoding: 'utf8', timeout: 25000, windowsHide: true })
process.stdout.write(result.stdout ?? '')
process.stderr.write(result.stderr ?? '')
assert.equal(result.status, 0, result.error?.message ?? 'packaged native module must spawn and read a PTY')
