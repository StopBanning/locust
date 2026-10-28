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
    // Writes a file, or runs what it computes.
    expect(plainCommand('echo hi > notes.txt')).toBeUndefined()
    expect(plainCommand('git log >> history.txt')).toBeUndefined()
    expect(plainCommand('rm $(git ls-files --deleted)')).toBeUndefined()
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
    // More than four steps is more than a sentence or two.
    expect(plainCommand('git status && git status && git status && git status && git status')).toBeUndefined()
    expect(plainCommand('')).toBeUndefined()
    expect(plainCommand(undefined)).toBeUndefined()
    expect(plainCommand('./build.sh')).toBeUndefined()
    expect(plainCommand('git frobnicate')).toBeUndefined()
  })

  it('a quoted arrow or a discarded stream is not a file written', () => {
    expect(said('git log --format="%h -> %s"')).toBe('Lists recent commits. It only looks.')
    expect(said('git fetch origin > /dev/null')).toBe('Gets new commits from origin without changing your files.')
    expect(said('Remove-Item dist -Recurse 2>$null')).toBe('Deletes dist and everything inside it. Nothing goes to the Recycle Bin or Trash.')
  })
})
