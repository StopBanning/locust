// Isolated OpenCode evidence for the six skills fixes. No app, no personal config, at most one free turn.
// node _tools/probe-sol-skills-fixes.mjs <opencode.ps1-or-binary> [--turn]
import assert from 'node:assert/strict'
import { spawn, spawnSync } from 'node:child_process'
import { mkdir, mkdtemp, readFile, realpath, rm, writeFile } from 'node:fs/promises'
import { createRequire } from 'node:module'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { setTimeout as pause } from 'node:timers/promises'
import { createOpenCodeRunCommand } from '../packages/runtime-adapters/dist/index.js'

const root = fileURLToPath(new URL('../', import.meta.url))
const desktop = join(root, 'apps', 'desktop')
const require = createRequire(join(desktop, 'package.json'))
const esbuild = createRequire(require.resolve('vite/package.json'))('esbuild')
const cli = process.argv[2]
if (!cli || cli.startsWith('--')) throw new Error('Supply the installed OpenCode launcher; no install is performed.')
const tempBase = await realpath(tmpdir())
const scratch = await mkdtemp(join(tempBase, 'locust-sol-skills-'))
const normalize = (value) => value.replaceAll(scratch.replaceAll('\\', '\\\\'), '<scratch>').replaceAll(scratch, '<scratch>').replaceAll(scratch.replaceAll('\\', '/'), '<scratch>')

async function idle() {
  if (process.platform !== 'win32') throw new Error('This measured probe is for the Windows handoff.')
  const until = Date.now() + 120_000
  for (;;) {
    const check = spawnSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', 'if (Get-Process opencode -ErrorAction SilentlyContinue) { exit 75 }'], { cwd: root, windowsHide: true })
    if (check.status === 0) return
    if (check.status !== 75) throw new Error('Could not check whether OpenCode is idle; no command was started.')
    if (Date.now() >= until) throw new Error('OpenCode is busy; no competing command was started.')
    console.log('Waiting for the existing OpenCode process; it will not be stopped.')
    await pause(3000)
  }
}

try {
  await esbuild.build({
    entryPoints: [join(desktop, 'src/main/opencode-skill-paths.ts'), join(desktop, 'src/main/run-skills.ts')],
    outdir: join(scratch, 'host'), bundle: true, platform: 'node', format: 'cjs', outExtension: { '.js': '.cjs' }, logLevel: 'warning',
    plugins: [{
      name: 'installed-parsers',
      setup(build) {
        build.onResolve({ filter: /^(?:jsonc-parser|yaml)$/ }, (args) => ({ path: require.resolve(args.path), external: true }))
      }
    }]
  })
  const { openCodeSkillPaths } = require(join(scratch, 'host/opencode-skill-paths.cjs'))
  const { prepareRunSkills } = require(join(scratch, 'host/run-skills.cjs'))
  const project = join(scratch, 'project')
  const home = join(scratch, 'home')
  const personal = join(scratch, 'personal-skills')
  const library = join(scratch, 'library')
  const put = async (file, text) => { await mkdir(dirname(file), { recursive: true }); await writeFile(file, text) }
  const configFile = join(project, 'opencode.json')
  const original = JSON.stringify({ $schema: 'https://opencode.ai/config.json', skills: { paths: [personal] } }, null, 2)
  await put(configFile, original)
  await put(join(personal, 'personal-proof/SKILL.md'), '---\nname: personal-proof\ndescription: A personal skill for the config probe.\n---\nThis must remain available.\n')
  await put(join(library, 'copied-proof/SKILL.md'), '---\nname: copied-proof\ndescription: Use when asked for the proof word.\n---\nRead references/check-word.txt relative to this skill and answer with its word. Use Read, not a shell.\n')
  await put(join(library, 'copied-proof/references/check-word.txt'), 'SCARLET-VOLE-628\n')
  const prepared = await prepareRunSkills({ scratchRoot: join(scratch, 'run-skills'), libraryFolder: library })
  assert(prepared)
  // Do not inherit provider keys, user config overrides or personal data/cache locations.
  const env = Object.fromEntries(['PATH', 'Path', 'SystemRoot', 'WINDIR', 'ComSpec', 'PATHEXT', 'PROCESSOR_ARCHITECTURE', 'NUMBER_OF_PROCESSORS'].filter((key) => process.env[key] !== undefined).map((key) => [key, process.env[key]]))
  Object.assign(env, { HOME: home, USERPROFILE: home, APPDATA: join(home, 'AppData/Roaming'), LOCALAPPDATA: join(home, 'AppData/Local'), XDG_CONFIG_HOME: join(home, '.config'), XDG_DATA_HOME: join(home, '.local/share'), XDG_CACHE_HOME: join(home, '.cache'), XDG_STATE_HOME: join(home, '.local/state'), TEMP: scratch, TMP: scratch, OPENCODE_DISABLE_AUTOUPDATE: '1', OPENCODE_DISABLE_LSP_DOWNLOAD: '1', NO_COLOR: '1', GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: join(home, 'gitconfig'), GIT_TEMPLATE_DIR: join(home, 'git-template') })
  await put(env.GIT_CONFIG_GLOBAL, '')
  await mkdir(env.GIT_TEMPLATE_DIR, { recursive: true })
  assert.equal(spawnSync('git', ['init', '--quiet', project], { cwd: root, env, windowsHide: true }).status, 0)
  const paths = await openCodeSkillPaths({ cwd: project, home, env })
  const prompt = 'Use the copied-proof skill. Read its references/check-word.txt with the read tool and reply with only the word it contains. Do not run shell commands.'
  const spec = createOpenCodeRunCommand({ commandName: 'opencode', discoveredPath: cli, executablePath: cli, prefixArgs: [], kind: 'native' }, { workspacePath: project, sandbox: 'read-only', model: 'opencode/deepseek-v4.1-flash-free', prompt, skillsDir: prepared.skillsDir, skillPaths: paths })
  Object.assign(env, spec.env)
  const run = async (args, input = '') => {
    await idle()
    const ps = cli.toLowerCase().endsWith('.ps1')
    const child = spawn(ps ? 'powershell.exe' : cli, ps ? ['-NoProfile', '-NonInteractive', '-File', resolve(cli), ...args] : args, { cwd: project, env, windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] })
    let output = '', error = ''
    let timedOut = false
    const timer = setTimeout(() => { timedOut = true; spawnSync('taskkill.exe', ['/PID', String(child.pid), '/T', '/F'], { cwd: root, windowsHide: true }) }, 120_000)
    child.stdout.on('data', (chunk) => { output += chunk })
    child.stderr.on('data', (chunk) => { error += chunk })
    child.stdin.on('error', () => undefined)
    child.stdin.end(input)
    const status = await new Promise((ok, fail) => { child.on('error', fail); child.on('close', ok) }).finally(() => clearTimeout(timer))
    if (status !== 0 || timedOut) throw new Error(normalize(`OpenCode exited ${status}${timedOut ? ' (timeout)' : ''}: ${error.slice(-3000)} ${output.slice(-3000)}`))
    return output
  }
  console.log(`OpenCode version: ${(await run(['--version'])).trim()}`)
  const debug = JSON.parse(await run(['debug', 'config']))
  assert.deepEqual(debug.skills.paths, [personal, prepared.skillsDir])
  console.log(`debug config skills.paths: ${normalize(JSON.stringify(debug.skills.paths))}`)
  console.log(`debug config external_directory: ${normalize(JSON.stringify(debug.permission.external_directory))}`)
  assert.equal(await readFile(configFile, 'utf8'), original)
  console.log('Project config unchanged: yes')
  if (process.argv.includes('--turn')) {
    // The only model request in this probe. Never retry a refused, interrupted or uncertain turn.
    const events = (await run(spec.args, prompt)).split(/\r?\n/).filter(Boolean).map((line) => JSON.parse(line))
    const read = events.find((event) => event.type === 'tool_use' && event.part?.tool === 'read' && event.part.state?.status === 'completed' && JSON.stringify(event.part.state).includes('SCARLET-VOLE-628'))
    const answer = events.filter((event) => event.type === 'text').map((event) => event.part?.text ?? '').join('')
    console.log(`Ask turn read: ${normalize(JSON.stringify(read?.part ?? null))}`)
    console.log(`Ask turn answer: ${JSON.stringify(answer)}`)
    console.log(`Ask turn errors: ${JSON.stringify(events.filter((event) => event.type === 'error'))}`)
    assert(read, 'No completed read of the supporting file was recorded.')
    assert(answer.includes('SCARLET-VOLE-628'), 'The answer did not contain the word from the supporting file.')
  }
  await prepared.dispose()
} finally {
  // Only this mkdtemp child of the real TEMP is ever removed, including after a failed measurement.
  assert(scratch.startsWith(join(tempBase, 'locust-sol-skills-')))
  await rm(scratch, { recursive: true, force: true })
  console.log('Removed the isolated probe folder and its disposable OpenCode home.')
}
