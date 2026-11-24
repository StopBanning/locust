//! Electron's `shared/a-command-says-what-it-does-in-plain-words.test.ts` (14 cases), on `plain_command`, which the
//! approval card (`approval::plain_action`), the chat apps and the commands card call. Written out from Electron's test
//! line for line; `view` is the three fields Electron compares (`said`, `careful`, `onlyLooks`).
use super::*;

fn said(command: &str) -> Option<String> {
    plain_command(command).map(|p| p.said)
}

fn view(command: &str) -> Option<(String, bool, bool)> {
    plain_command(command).map(|p| (p.said, p.careful, p.only_looks))
}

// case: says what a push does, and that a forced one loses commits on the other end
#[test]
fn plain_words_says_what_a_push_does_and_that_a_forced_one_loses_commits_on() {
    assert_eq!(said("git push origin main").as_deref(), Some("Sends the commits on main to origin."));
    assert_eq!(said("git push").as_deref(), Some("Sends this branch’s new commits to the repository this branch is linked to."));
    assert_eq!(said("git push -u origin feature/cart").as_deref(), Some("Sends the commits on feature/cart to origin."));
    let forced = view("git push --force origin main").unwrap();
    assert_eq!(forced.0, "Replaces main on origin with this computer’s version: commits there that are not on this computer are lost.");
    assert_eq!(forced.1, true);
    assert_eq!(said("git push origin +main").as_deref(), Some(forced.0.as_str()));
    assert_eq!(said("git push -f").as_deref(), Some("Replaces this branch on the repository this branch is linked to with this computer’s version: commits there that are not on this computer are lost."));
    assert!(said("git push --force-with-lease origin main").unwrap_or_default().contains("unless someone else changed it there since you last fetched"));
    assert_eq!(said("git push origin --delete old-branch").as_deref(), Some("Deletes the branch old-branch on origin."));
}

// case: reads a chain of steps, each in turn
#[test]
fn plain_words_reads_a_chain_of_steps_each_in_turn() {
    assert_eq!(said("git add -A && git commit -m \"Fix the cart total\" && git push").as_deref(), Some("Marks every change in this folder to go in the next commit. Saves the marked changes as a commit with the message “Fix the cart total”, on this computer only. Sends this branch’s new commits to the repository this branch is linked to."));
    assert_eq!(said("cd apps/desktop && pnpm test").as_deref(), Some("Goes into apps/desktop. Runs this project’s “test” script: whatever its package.json says that is."));
    assert_eq!(said("cd .. && git status && exit 0").as_deref(), Some("Goes up one folder. Shows which files have changed. It only looks."));
    assert_eq!(said("npm test || echo failed").as_deref(), Some("Runs this project’s “test” script: whatever its package.json says that is. If that fails, prints a line of text."));
}

// case: says when it only looks, once
#[test]
fn plain_words_says_when_it_only_looks_once() {
    let looks = view("git status && git diff").unwrap();
    assert_eq!(Some(looks.clone()), Some(("Shows which files have changed. Shows what has changed in the files. It only looks.".to_owned(), false, true)));
    assert_eq!(said("git log --oneline -20 | head -5").as_deref(), Some("Lists recent commits. It only looks."));
    assert_eq!(said("Get-ChildItem -Recurse src | Select-Object Name").as_deref(), Some("Lists the files in src and every folder inside it. It only looks."));
    assert_eq!(said("rg -n \"cartTotal\" src").as_deref(), Some("Searches src for “cartTotal”. It only looks."));
    assert_eq!(said("ls -la").as_deref(), Some("Lists the files in this folder. It only looks."));
    assert_eq!(said("node --version").as_deref(), Some("Shows which version of node is installed. It only looks."));
}

// case: says a delete skips the Recycle Bin, on Windows and elsewhere
#[test]
fn plain_words_says_a_delete_skips_the_recycle_bin_on_windows_and_elsewhere() {
    let gone = view("rm -rf node_modules dist").unwrap();
    assert_eq!(gone.0, "Deletes node_modules and dist and everything inside them. Nothing goes to the Recycle Bin or Trash.");
    assert_eq!(gone.1, true);
    assert_eq!(said("Remove-Item -Path .\\build -Recurse -Force -ErrorAction SilentlyContinue").as_deref(), Some("Deletes .\\build and everything inside it. Nothing goes to the Recycle Bin or Trash."));
    // -Force has an r in it, and is not -Recurse.
    assert_eq!(said("Remove-Item notes.txt -Force").as_deref(), Some("Deletes notes.txt. Nothing goes to the Recycle Bin or Trash."));
    assert_eq!(said("rmdir /s /q build").as_deref(), Some("Deletes build and everything inside it. Nothing goes to the Recycle Bin or Trash."));
    assert_eq!(said("del /s /q *.tmp").as_deref(), Some("Deletes *.tmp in this folder and every folder inside it. Nothing goes to the Recycle Bin or Trash."));
    // Only an empty folder, and a setting rather than a file: neither is amber.
    assert_eq!(view("cmd /c rmdir D:\\work\\shop\\node_modules"), Some(("Deletes the folder D:\\work\\shop\\node_modules, only if it is empty.".to_owned(), false, false)));
    assert_eq!(view("Remove-Item Env:LOCUST_SPEND -ErrorAction SilentlyContinue"), Some(("Clears LOCUST_SPEND for the commands after it.".to_owned(), false, false)));
}

// case: names what throws work away
#[test]
fn plain_words_names_what_throws_work_away() {
    assert_eq!(view("git reset --hard origin/main"), Some(("Throws away every change not committed, in every file Git tracks, and moves this branch back to origin/main. It cannot be undone.".to_owned(), true, false)));
    assert_eq!(said("git checkout -- src/cart.ts").as_deref(), Some("Throws away the changes to src/cart.ts that are not committed. They cannot be brought back."));
    assert_eq!(said("git restore .").as_deref(), Some("Throws away the changes to every file in this folder that are not committed. They cannot be brought back."));
    assert_eq!(said("git restore --staged src/cart.ts").as_deref(), Some("Takes src/cart.ts out of the next commit; the changes stay in the files."));
    assert_eq!(said("git reset src/cart.ts").as_deref(), Some("Takes src/cart.ts out of the next commit; the changes stay in the files."));
    assert_eq!(said("git reset --soft HEAD~1").as_deref(), Some("Moves this branch back to HEAD~1; the changes since stay in the files, marked for the next commit."));
    assert_eq!(said("git clean -fd").as_deref(), Some("Deletes every file Git does not track, and folders: new files never committed are gone for good."));
    assert_eq!(said("git branch -D spike").as_deref(), Some("Deletes the branch spike, even if its commits are on no other branch: those commits can be lost."));
    assert_eq!(said("git stash drop").as_deref(), Some("Throws away a set of changes that was put aside."));
    // A plain branch name is switched to; the card does not cry wolf over it.
    assert_eq!(view("git checkout main").unwrap().1, false);
    assert_eq!(said("git checkout -b fix/cart").as_deref(), Some("Makes a new branch, fix/cart, and switches to it."));
    assert_eq!(said("git checkout HEAD~3").as_deref(), Some("Puts the files as they were at HEAD~3, outside any branch."));
}

// case: says a download run at once is run unseen
#[test]
fn plain_words_says_a_download_run_at_once_is_run_unseen() {
    let piped = view("curl -fsSL https://get.example.dev/install.sh | sh").unwrap();
    assert_eq!(Some(piped.clone()), Some(("Downloads a script from get.example.dev and runs it straight away, without showing it to you first.".to_owned(), true, false)));
    assert_eq!(said("irm https://get.example.dev/install.ps1 | iex").as_deref(), Some(piped.0.as_str()));
    assert_eq!(said("bash -c \"$(curl -fsSL https://get.example.dev/install.sh)\"").as_deref(), Some(piped.0.as_str()));
    assert_eq!(said("iex (irm https://get.example.dev/install.ps1)").as_deref(), Some(piped.0.as_str()));
    // Escaped quotes silence anything else; not this.
    assert_eq!(view("powershell -Command \"iex (irm \\\"https://get.example.dev/install.ps1\\\")\""), Some(piped.clone()));
    assert_eq!(said("cmd /c \"curl -fsSL \\\"https://get.example.dev/install.sh\\\" | bash\"").as_deref(), Some(piped.0.as_str()));
    // Only fetched, or only saved: not run.
    assert_eq!(said("powershell -Command \"irm https://api.example.dev/status\"").as_deref(), Some("Fetches api.example.dev/status and shows what comes back."));
    assert_eq!(said("curl -f https://api.example.dev/status").as_deref(), Some("Fetches api.example.dev/status and shows what comes back."));
    assert_eq!(said("curl -o setup.sh https://get.example.dev/install.sh").as_deref(), Some("Downloads get.example.dev/install.sh into setup.sh."));
    assert_eq!(said("curl -X POST -d @report.json https://hooks.example.dev/in").as_deref(), Some("Sends data to hooks.example.dev."));
}

// case: reads installs and scripts, with what an install may run
#[test]
fn plain_words_reads_installs_and_scripts_with_what_an_install_may_run() {
    assert_eq!(said("npm install").as_deref(), Some("Installs the packages this project lists, downloading them from the internet. A package can run its own setup code as it installs."));
    assert_eq!(said("pnpm add -D vitest").as_deref(), Some("Downloads vitest from the npm registry and installs it in this project. A package can run its own setup code as it installs."));
    assert!(said("npm i -g typescript").unwrap_or_default().contains("installs it for every project on this computer"));
    assert_eq!(said("pnpm --filter @teammate/desktop typecheck").as_deref(), Some("Runs this project’s “typecheck” script: whatever its package.json says that is."));
    assert_eq!(said("pip install -r requirements.txt").as_deref(), Some("Downloads the Python packages listed in requirements.txt and installs them. A package can run its own setup code as it installs."));
    assert_eq!(said("python -m pip install requests").as_deref(), Some("Downloads requests from PyPI and installs it. A package can run its own setup code as it installs."));
    assert_eq!(said("python scripts/seed.py").as_deref(), Some("Runs the script scripts/seed.py."));
    assert_eq!(said("npx prettier --check .").as_deref(), Some("Runs prettier, downloading it first if this project does not have it."));
    assert_eq!(said("gh pr create --title \"Fix cart\" --body \"...\"").as_deref(), Some("Opens a pull request on GitHub."));
    assert_eq!(said("gh pr merge 42 --squash").as_deref(), Some("Merges the pull request #42 on GitHub."));
    assert_eq!(said("gh pr view 42").as_deref(), Some("Reads the pull request #42 on GitHub. It only looks."));
    assert_eq!(said("pip install -e .").as_deref(), Some("Installs the Python project in this folder. A package can run its own setup code as it installs."));
}

// case: reads the command inside the shells the runtimes wrap it in
#[test]
fn plain_words_reads_the_command_inside_the_shells_the_runtimes_wrap_it_in() {
    assert_eq!(said("\"C:\\WINDOWS\\System32\\WindowsPowerShell\\v1.0\\powershell.exe\" -Command \"git status\"").as_deref(), Some("Shows which files have changed. It only looks."));
    assert_eq!(said("bash -lc 'git push origin main'").as_deref(), Some("Sends the commits on main to origin."));
    assert_eq!(said("cmd /c \"npm run build\"").as_deref(), Some("Runs this project’s “build” script: whatever its package.json says that is."));
    assert_eq!(said("sudo npm i -g pnpm").as_deref(), Some("Downloads pnpm from the npm registry and installs it for every project on this computer. A package can run its own setup code as it installs. It runs with administrator rights."));
    assert_eq!(said("npm test 2>&1 | tail -20").as_deref(), Some("Runs this project’s “test” script: whatever its package.json says that is."));
    assert_eq!(said("git status 2>&1 | Select-Object -First 20").as_deref(), Some("Shows which files have changed. It only looks."));
}

// case: says nothing unless it can read every part
#[test]
fn plain_words_says_nothing_unless_it_can_read_every_part() {
    // One part it cannot read: a sentence about the rest would stand for the whole.
    assert_eq!(view("npm test && ./deploy.sh"), None);
    assert_eq!(view("git status; terraform apply"), None);
    // Runs what it computes, or writes where the words do not say.
    assert_eq!(view("rm $(git ls-files --deleted)"), None);
    assert_eq!(view("Remove-Item -LiteralPath (Join-Path $env:TEMP \"probe.mjs\")"), None);
    assert_eq!(view("Get-ChildItem | Where-Object { Remove-Item $_ }"), None);
    assert_eq!(view("node build.mjs > $env:TEMP\\build.log"), None);
    assert_eq!(view("git status > a.txt 2> b.txt"), None);
    assert_eq!(view("node -e \"require('fs').rmSync('x')\""), None);
    assert_eq!(view("python -c \"print(1)\""), None);
    // A pipe into something that is not a plain filter.
    assert_eq!(view("git ls-files | xargs rm"), None);
    assert_eq!(view("cat list.txt | sed -i s/a/b/ x"), None);
    assert_eq!(view("git log | sort -o history.txt"), None);
    // Asks where a program is, not to run it.
    assert_eq!(view("command -v git"), None);
    // `type` in a POSIX shell says what a word is.
    assert_eq!(view("type git"), None);
    // More than four steps that change things: the one left unsaid could be the one that deletes.
    assert_eq!(view("git add -A && git commit -m x && git tag v2 && git push && git push --tags"), None);
    // An escaped quote ends a quote here and not in the shell, so the rest could read as a search's words.
    assert_eq!(view("powershell -Command \"rg -n 'a=\\\"b' src; pnpm build > out.log\""), None);
    assert_eq!(view("Write-Output \"say `\"hi`\"\" ; rm -rf dist"), None);
    assert_eq!(view("rg -n 'never closed src; rm -rf dist"), None);
    assert_eq!(view(""), None);
    // (Electron's undefined: the Rust caller always has a command's text.)
    assert_eq!(view("./build.sh"), None);
    assert_eq!(view("git frobnicate"), None);
}

// case: reads the way Codex reads a project: files by -LiteralPath, part of one, a long chain that only looks
#[test]
fn plain_words_reads_the_way_codex_reads_a_project_files_by_literalpath_par() {
    assert_eq!(said("Get-Content -LiteralPath 'docs/PLAN.md' -Encoding UTF8").as_deref(), Some("Shows what is in docs/PLAN.md. It only looks."));
    assert_eq!(said("Get-Content gate.log -Tail 20").as_deref(), Some("Shows the end of gate.log. It only looks."));
    assert_eq!(said("Get-Content src/App.tsx | Select-Object -Skip 760 -First 40").as_deref(), Some("Shows part of src/App.tsx. It only looks."));
    assert_eq!(said("rg --files -g AGENTS.md -g CLAUDE.md").as_deref(), Some("Lists the files in this folder named like AGENTS.md or CLAUDE.md. It only looks."));
    assert_eq!(said("Select-String -LiteralPath report.mjs -Pattern 'Final'").as_deref(), Some("Searches report.mjs for “Final”. It only looks."));
    assert_eq!(said("git branch --show-current").as_deref(), Some("Shows which branch this is. It only looks."));
    assert_eq!(said("git show main:src/cart.ts").as_deref(), Some("Shows src/cart.ts as it was at main. It only looks."));
    assert_eq!(said("node --check app.js").as_deref(), Some("Checks app.js for mistakes in how it is written, without running it. It only looks."));
    assert_eq!(said("node --test --test-concurrency=2 tools/*.test.mjs").as_deref(), Some("Runs the tests in tools/*.test.mjs."));
    assert_eq!(said("Test-Path dist -PathType Container; Get-Date; Get-Process -Name node; sleep 240").as_deref(), Some("Checks whether dist is there. Shows the date and time. Looks up the running program node. Waits about 4 minutes. It only looks."));
    let long = view("git status --short; git log --oneline -5; Get-Content AGENTS.md; Get-Content PROJECT.md -TotalCount 100; rg -n \"cart\" src; git diff --stat").unwrap();
    assert_eq!(Some(long.clone()), Some(("Shows which files have changed. Lists recent commits. Shows what is in AGENTS.md. And 3 more steps. It only looks.".to_owned(), false, true)));
}

// case: says a step run again once, with how often
#[test]
fn plain_words_says_a_step_run_again_once_with_how_often() {
    assert_eq!(said("node test/ui.mjs resize 1120 720; node test/ui.mjs elements; node test/ui.mjs shot a.png").as_deref(), Some("Runs the script test/ui.mjs, 3 times."));
    assert_eq!(said("git status && git status").as_deref(), Some("Shows which files have changed, twice. It only looks."));
    // A two-sentence step keeps both: a count after the second would belong to it.
    assert_eq!(said("rm a.txt; rm a.txt").as_deref(), Some("Deletes a.txt. Nothing goes to the Recycle Bin or Trash. Deletes a.txt. Nothing goes to the Recycle Bin or Trash."));
}

// case: says where what it prints is saved, and when that loses a file
#[test]
fn plain_words_says_where_what_it_prints_is_saved_and_when_that_loses_a_fil() {
    assert_eq!(said("node test/mutation.mjs > ../../.tmp/controls.log 2>&1; $code = $LASTEXITCODE; Get-Content ../../.tmp/controls.log -Tail 15; exit $code").as_deref(), Some("Runs the script test/mutation.mjs. What it prints is saved in ../../.tmp/controls.log. Shows the end of ../../.tmp/controls.log."));
    assert_eq!(said("node main.mjs *> \"C:\\Users\\me\\AppData\\Local\\Temp\\run\\drive.log\"").as_deref(), Some("Runs the script main.mjs. What it prints is saved in C:\\Users\\me\\AppData\\Local\\Temp\\run\\drive.log."));
    assert_eq!(said("git log >> history.txt").as_deref(), Some("Lists recent commits. What it prints is added to the end of history.txt."));
    assert_eq!(said("npm test 2> errors.txt").as_deref(), Some("Runs this project’s “test” script: whatever its package.json says that is. Its error messages replace what was in errors.txt."));
    // Replacing a file that is not a log loses what was in it.
    let replaced = view("echo hi > notes.txt").unwrap();
    assert_eq!(Some(replaced.clone()), Some(("Prints a line of text. What it prints replaces what was in notes.txt.".to_owned(), true, false)));
    assert_eq!(view("node build.mjs > build.log").unwrap().1, false);
}

// case: names a setting made for the commands after it, never its value
#[test]
fn plain_words_names_a_setting_made_for_the_commands_after_it_never_its_val() {
    assert_eq!(said("$env:PNPM_CONFIG_STORE_DIR='C:/work/pnpm-store'; pnpm build").as_deref(), Some("Sets PNPM_CONFIG_STORE_DIR for the commands after it. Runs this project’s “build” script: whatever its package.json says that is."));
    assert_eq!(said("NODE_ENV=production node build.mjs").as_deref(), Some("Sets NODE_ENV for this command. Runs the script build.mjs."));
    assert_eq!(said("$ErrorActionPreference = 'Stop'; git status").as_deref(), Some("Shows which files have changed. It only looks."));
    assert!(!said("$env:API_TOKEN=\"abc123\"; gh pr list").unwrap_or_default().contains("abc123"));
    assert_eq!(said("bash _tools/gate.sh").as_deref(), Some("Runs the script _tools/gate.sh."));
    assert_eq!(said("powershell -File setup.ps1").as_deref(), Some("Runs the script setup.ps1."));
}

// case: a quoted arrow or a discarded stream is not a file written
#[test]
fn plain_words_a_quoted_arrow_or_a_discarded_stream_is_not_a_file_written() {
    assert_eq!(said("git log --format=\"%h -> %s\"").as_deref(), Some("Lists recent commits. It only looks."));
    assert_eq!(said("git fetch origin > /dev/null").as_deref(), Some("Gets new commits from origin without changing your files."));
    assert_eq!(said("Remove-Item dist -Recurse 2>$null").as_deref(), Some("Deletes dist and everything inside it. Nothing goes to the Recycle Bin or Trash."));
}

