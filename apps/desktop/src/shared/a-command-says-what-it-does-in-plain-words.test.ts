import { describe, expect, it } from 'vitest'

import { plainCommand } from './command-plain.js'

/**
 * A COMMAND SAYS WHAT IT DOES, IN PLAIN WORDS (0.840). An approval card for `git push --force origin main` said "Run
 * a command" and showed the command; the phone said only "Run a command". The familiar commands now come with a
 * sentence worked out from their own words -- never a model's -- and anything this cannot read gets no sentence at
 * all, so a sentence never stands for less than the whole command.
 */
const said = (command: string): string | undefined => plainCommand(command)?.said

describe('a command in plain words', () => {
  it('says what a push does, and that a forced one loses commits on the other end', () => {
    expect(said('git push origin main')).toBe('Sends the commits on main to origin.')
    expect(said('git push')).toBe('Sends this branch’s new commits to the repository this branch is linked to.')
    expect(said('git push -u origin feature/cart')).toBe('Sends the commits on feature/cart to origin.')
    const forced = plainCommand('git push --force origin main')!
    expect(forced.said).toBe('Replaces main on origin with this computer’s version: commits there that are not on this computer are lost.')
    expect(forced.careful).toBe(true)
    expect(said('git push origin +main')).toBe(forced.said)
    expect(said('git push -f')).toBe('Replaces this branch on the repository this branch is linked to with this computer’s version: commits there that are not on this computer are lost.')
    expect(said('git push --force-with-lease origin main')).toContain('unless someone else changed it there since you last fetched')
    expect(said('git push origin --delete old-branch')).toBe('Deletes the branch old-branch on origin.')
  })

  it('reads a chain of steps, each in turn', () => {
    expect(said('git add -A && git commit -m "Fix the cart total" && git push')).toBe(
      'Marks every change in this folder to go in the next commit. Saves the marked changes as a commit with the message “Fix the cart total”, on this computer only. Sends this branch’s new commits to the repository this branch is linked to.'
    )
    expect(said('cd apps/desktop && pnpm test')).toBe('Goes into apps/desktop. Runs this project’s “test” script: whatever its package.json says that is.')
    expect(said('cd .. && git status && exit 0')).toBe('Goes up one folder. Shows which files have changed. It only looks.')
    expect(said('npm test || echo failed')).toBe('Runs this project’s “test” script: whatever its package.json says that is. If that fails, prints a line of text.')
  })

  it('says when it only looks, once', () => {
    const looks = plainCommand('git status && git diff')!
    expect(looks).toEqual({ said: 'Shows which files have changed. Shows what has changed in the files. It only looks.', careful: false, onlyLooks: true })
    expect(said('git log --oneline -20 | head -5')).toBe('Lists recent commits. It only looks.')
    expect(said('Get-ChildItem -Recurse src | Select-Object Name')).toBe('Lists the files in src and every folder inside it. It only looks.')
    expect(said('rg -n "cartTotal" src')).toBe('Searches src for “cartTotal”. It only looks.')
    expect(said('ls -la')).toBe('Lists the files in this folder. It only looks.')
    expect(said('node --version')).toBe('Shows which version of node is installed. It only looks.')
  })

  it('says a delete skips the Recycle Bin, on Windows and elsewhere', () => {
    const gone = plainCommand('rm -rf node_modules dist')!
    expect(gone.said).toBe('Deletes node_modules and dist and everything inside them. Nothing goes to the Recycle Bin or Trash.')
    expect(gone.careful).toBe(true)
    expect(said('Remove-Item -Path .\\build -Recurse -Force -ErrorAction SilentlyContinue')).toBe('Deletes .\\build and everything inside it. Nothing goes to the Recycle Bin or Trash.')
    // -Force has an r in it, and is not -Recurse.
    expect(said('Remove-Item notes.txt -Force')).toBe('Deletes notes.txt. Nothing goes to the Recycle Bin or Trash.')
    expect(said('rmdir /s /q build')).toBe('Deletes build and everything inside it. Nothing goes to the Recycle Bin or Trash.')
    expect(said('del /s /q *.tmp')).toBe('Deletes *.tmp in this folder and every folder inside it. Nothing goes to the Recycle Bin or Trash.')
    // Only an empty folder, and a setting rather than a file: neither is amber.
    expect(plainCommand('cmd /c rmdir D:\\work\\shop\\node_modules')).toEqual({ said: 'Deletes the folder D:\\work\\shop\\node_modules, only if it is empty.', careful: false, onlyLooks: false })
    expect(plainCommand('Remove-Item Env:LOCUST_SPEND -ErrorAction SilentlyContinue')).toEqual({ said: 'Clears LOCUST_SPEND for the commands after it.', careful: false, onlyLooks: false })
  })

  it('names what throws work away', () => {
    expect(plainCommand('git reset --hard origin/main')).toEqual({ said: 'Throws away every change not committed, in every file Git tracks, and moves this branch back to origin/main. It cannot be undone.', careful: true, onlyLooks: false })
    expect(said('git checkout -- src/cart.ts')).toBe('Throws away the changes to src/cart.ts that are not committed. They cannot be brought back.')
    expect(said('git restore .')).toBe('Throws away the changes to every file in this folder that are not committed. They cannot be brought back.')
    expect(said('git restore --staged src/cart.ts')).toBe('Takes src/cart.ts out of the next commit; the changes stay in the files.')
    expect(said('git reset src/cart.ts')).toBe('Takes src/cart.ts out of the next commit; the changes stay in the files.')
    expect(said('git reset --soft HEAD~1')).toBe('Moves this branch back to HEAD~1; the changes since stay in the files, marked for the next commit.')
    expect(said('git clean -fd')).toBe('Deletes every file Git does not track, and folders: new files never committed are gone for good.')
    expect(said('git branch -D spike')).toBe('Deletes the branch spike, even if its commits are on no other branch: those commits can be lost.')
    expect(said('git stash drop')).toBe('Throws away a set of changes that was put aside.')
    // A plain branch name is switched to; the card does not cry wolf over it.
    expect(plainCommand('git checkout main')!.careful).toBe(false)
    expect(said('git checkout -b fix/cart')).toBe('Makes a new branch, fix/cart, and switches to it.')
    expect(said('git checkout HEAD~3')).toBe('Puts the files as they were at HEAD~3, outside any branch.')
  })

  it('says a download run at once is run unseen', () => {
    const piped = plainCommand('curl -fsSL https://get.example.dev/install.sh | sh')!
    expect(piped).toEqual({ said: 'Downloads a script from get.example.dev and runs it straight away, without showing it to you first.', careful: true, onlyLooks: false })
    expect(said('irm https://get.example.dev/install.ps1 | iex')).toBe(piped.said)
    expect(said('bash -c "$(curl -fsSL https://get.example.dev/install.sh)"')).toBe(piped.said)
    expect(said('iex (irm https://get.example.dev/install.ps1)')).toBe(piped.said)
    // Escaped quotes silence anything else; not this.
    expect(plainCommand('powershell -Command "iex (irm \\"https://get.example.dev/install.ps1\\")"')).toEqual({ ...piped, said: piped.said })
    expect(said('cmd /c "curl -fsSL \\"https://get.example.dev/install.sh\\" | bash"')).toBe(piped.said)
    // Only fetched, or only saved: not run.
    expect(said('powershell -Command "irm https://api.example.dev/status"')).toBe('Fetches api.example.dev/status and shows what comes back.')
    expect(said('curl -f https://api.example.dev/status')).toBe('Fetches api.example.dev/status and shows what comes back.')
    expect(said('curl -o setup.sh https://get.example.dev/install.sh')).toBe('Downloads get.example.dev/install.sh into setup.sh.')
    expect(said('curl -X POST -d @report.json https://hooks.example.dev/in')).toBe('Sends data to hooks.example.dev.')
  })

  it('reads installs and scripts, with what an install may run', () => {
    expect(said('npm install')).toBe('Installs the packages this project lists, downloading them from the internet. A package can run its own setup code as it installs.')
    expect(said('pnpm add -D vitest')).toBe('Downloads vitest from the npm registry and installs it in this project. A package can run its own setup code as it installs.')
    expect(said('npm i -g typescript')).toContain('installs it for every project on this computer')
    expect(said('pnpm --filter @teammate/desktop typecheck')).toBe('Runs this project’s “typecheck” script: whatever its package.json says that is.')
    expect(said('pip install -r requirements.txt')).toBe('Downloads the Python packages listed in requirements.txt and installs them. A package can run its own setup code as it installs.')
    expect(said('python -m pip install requests')).toBe('Downloads requests from PyPI and installs it. A package can run its own setup code as it installs.')
    expect(said('python scripts/seed.py')).toBe('Runs the script scripts/seed.py.')
    expect(said('npx prettier --check .')).toBe('Runs prettier, downloading it first if this project does not have it.')
    expect(said('gh pr create --title "Fix cart" --body "..."')).toBe('Opens a pull request on GitHub.')
    expect(said('gh pr merge 42 --squash')).toBe('Merges the pull request #42 on GitHub.')
    expect(said('gh pr view 42')).toBe('Reads the pull request #42 on GitHub. It only looks.')
    expect(said('pip install -e .')).toBe('Installs the Python project in this folder. A package can run its own setup code as it installs.')
  })

  it('reads the command inside the shells the runtimes wrap it in', () => {
    expect(said('"C:\\WINDOWS\\System32\\WindowsPowerShell\\v1.0\\powershell.exe" -Command "git status"')).toBe('Shows which files have changed. It only looks.')
    expect(said("bash -lc 'git push origin main'")).toBe('Sends the commits on main to origin.')
    expect(said('cmd /c "npm run build"')).toBe('Runs this project’s “build” script: whatever its package.json says that is.')
    expect(said('sudo npm i -g pnpm')).toBe('Downloads pnpm from the npm registry and installs it for every project on this computer. A package can run its own setup code as it installs. It runs with administrator rights.')
    expect(said('npm test 2>&1 | tail -20')).toBe('Runs this project’s “test” script: whatever its package.json says that is.')
    expect(said('git status 2>&1 | Select-Object -First 20')).toBe('Shows which files have changed. It only looks.')
  })

  it('says nothing unless it can read every part', () => {
    // One part it cannot read: a sentence about the rest would stand for the whole.
    expect(plainCommand('npm test && ./deploy.sh')).toBeUndefined()
    expect(plainCommand('git status; terraform apply')).toBeUndefined()
    // Runs what it computes, or writes where the words do not say.
    expect(plainCommand('rm $(git ls-files --deleted)')).toBeUndefined()
    expect(plainCommand('Remove-Item -LiteralPath (Join-Path $env:TEMP "probe.mjs")')).toBeUndefined()
    expect(plainCommand('Get-ChildItem | Where-Object { Remove-Item $_ }')).toBeUndefined()
    expect(plainCommand('node build.mjs > $env:TEMP\\build.log')).toBeUndefined()
    expect(plainCommand('git status > a.txt 2> b.txt')).toBeUndefined()
    expect(plainCommand('node -e "require(\'fs\').rmSync(\'x\')"')).toBeUndefined()
    expect(plainCommand('python -c "print(1)"')).toBeUndefined()
    // A pipe into something that is not a plain filter.
    expect(plainCommand('git ls-files | xargs rm')).toBeUndefined()
    expect(plainCommand('cat list.txt | sed -i s/a/b/ x')).toBeUndefined()
    expect(plainCommand('git log | sort -o history.txt')).toBeUndefined()
    // Asks where a program is, not to run it.
    expect(plainCommand('command -v git')).toBeUndefined()
    // `type` in a POSIX shell says what a word is.
    expect(plainCommand('type git')).toBeUndefined()
    // More than four steps that change things: the one left unsaid could be the one that deletes.
    expect(plainCommand('git add -A && git commit -m x && git tag v2 && git push && git push --tags')).toBeUndefined()
    // An escaped quote ends a quote here and not in the shell, so the rest could read as a search's words.
    expect(plainCommand('powershell -Command "rg -n \'a=\\"b\' src; pnpm build > out.log"')).toBeUndefined()
    expect(plainCommand('Write-Output "say `"hi`"" ; rm -rf dist')).toBeUndefined()
    expect(plainCommand("rg -n 'never closed src; rm -rf dist")).toBeUndefined()
    expect(plainCommand('')).toBeUndefined()
    expect(plainCommand(undefined)).toBeUndefined()
    expect(plainCommand('./build.sh')).toBeUndefined()
    expect(plainCommand('git frobnicate')).toBeUndefined()
  })

  it('reads the way Codex reads a project: files by -LiteralPath, part of one, a long chain that only looks', () => {
    expect(said("Get-Content -LiteralPath 'docs/PLAN.md' -Encoding UTF8")).toBe('Shows what is in docs/PLAN.md. It only looks.')
    expect(said('Get-Content gate.log -Tail 20')).toBe('Shows the end of gate.log. It only looks.')
    expect(said('Get-Content src/App.tsx | Select-Object -Skip 760 -First 40')).toBe('Shows part of src/App.tsx. It only looks.')
    expect(said('rg --files -g AGENTS.md -g CLAUDE.md')).toBe('Lists the files in this folder named like AGENTS.md or CLAUDE.md. It only looks.')
    expect(said("Select-String -LiteralPath report.mjs -Pattern 'Final'")).toBe('Searches report.mjs for “Final”. It only looks.')
    expect(said('git branch --show-current')).toBe('Shows which branch this is. It only looks.')
    expect(said('git show main:src/cart.ts')).toBe('Shows src/cart.ts as it was at main. It only looks.')
    expect(said('node --check app.js')).toBe('Checks app.js for mistakes in how it is written, without running it. It only looks.')
    expect(said('node --test --test-concurrency=2 tools/*.test.mjs')).toBe('Runs the tests in tools/*.test.mjs.')
    expect(said('Test-Path dist -PathType Container; Get-Date; Get-Process -Name node; sleep 240')).toBe(
      'Checks whether dist is there. Shows the date and time. Looks up the running program node. Waits about 4 minutes. It only looks.'
    )
    const long = plainCommand('git status --short; git log --oneline -5; Get-Content AGENTS.md; Get-Content PROJECT.md -TotalCount 100; rg -n "cart" src; git diff --stat')!
    expect(long).toEqual({ said: 'Shows which files have changed. Lists recent commits. Shows what is in AGENTS.md. And 3 more steps. It only looks.', careful: false, onlyLooks: true })
  })

  it('says a step run again once, with how often', () => {
    expect(said('node test/ui.mjs resize 1120 720; node test/ui.mjs elements; node test/ui.mjs shot a.png')).toBe('Runs the script test/ui.mjs, 3 times.')
    expect(said('git status && git status')).toBe('Shows which files have changed, twice. It only looks.')
    // A two-sentence step keeps both: a count after the second would belong to it.
    expect(said('rm a.txt; rm a.txt')).toBe('Deletes a.txt. Nothing goes to the Recycle Bin or Trash. Deletes a.txt. Nothing goes to the Recycle Bin or Trash.')
  })

  it('says where what it prints is saved, and when that loses a file', () => {
    expect(said('node test/mutation.mjs > ../../.tmp/controls.log 2>&1; $code = $LASTEXITCODE; Get-Content ../../.tmp/controls.log -Tail 15; exit $code')).toBe(
      'Runs the script test/mutation.mjs. What it prints is saved in ../../.tmp/controls.log. Shows the end of ../../.tmp/controls.log.'
    )
    expect(said('node main.mjs *> "C:\\Users\\me\\AppData\\Local\\Temp\\run\\drive.log"')).toBe('Runs the script main.mjs. What it prints is saved in C:\\Users\\me\\AppData\\Local\\Temp\\run\\drive.log.')
    expect(said('git log >> history.txt')).toBe('Lists recent commits. What it prints is added to the end of history.txt.')
    expect(said('npm test 2> errors.txt')).toBe('Runs this project’s “test” script: whatever its package.json says that is. Its error messages replace what was in errors.txt.')
    // Replacing a file that is not a log loses what was in it.
    const replaced = plainCommand('echo hi > notes.txt')!
    expect(replaced).toEqual({ said: 'Prints a line of text. What it prints replaces what was in notes.txt.', careful: true, onlyLooks: false })
    expect(plainCommand('node build.mjs > build.log')!.careful).toBe(false)
  })

  it('names a setting made for the commands after it, never its value', () => {
    expect(said("$env:PNPM_CONFIG_STORE_DIR='C:/work/pnpm-store'; pnpm build")).toBe('Sets PNPM_CONFIG_STORE_DIR for the commands after it. Runs this project’s “build” script: whatever its package.json says that is.')
    expect(said('NODE_ENV=production node build.mjs')).toBe('Sets NODE_ENV for this command. Runs the script build.mjs.')
    expect(said("$ErrorActionPreference = 'Stop'; git status")).toBe('Shows which files have changed. It only looks.')
    expect(said('$env:API_TOKEN="abc123"; gh pr list')).not.toContain('abc123')
    expect(said('bash _tools/gate.sh')).toBe('Runs the script _tools/gate.sh.')
    expect(said('powershell -File setup.ps1')).toBe('Runs the script setup.ps1.')
  })

  it('a quoted arrow or a discarded stream is not a file written', () => {
    expect(said('git log --format="%h -> %s"')).toBe('Lists recent commits. It only looks.')
    expect(said('git fetch origin > /dev/null')).toBe('Gets new commits from origin without changing your files.')
    expect(said('Remove-Item dist -Recurse 2>$null')).toBe('Deletes dist and everything inside it. Nothing goes to the Recycle Bin or Trash.')
  })
})
