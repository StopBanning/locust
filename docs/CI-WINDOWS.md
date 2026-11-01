# The Windows build on GitHub Actions

`.github/workflows/windows-build.yml` builds Locust's unsigned Windows
installer on GitHub's `windows-latest` runner, in the public repository
`StopBanning/locust`. It exists so that a build nobody's
machine touched can be set beside the one the laptop makes, and so that
signing (SignPath, applied for) has somewhere to happen later
(`docs/PLAN-2026-10-05-TO-MARKET.md`, section 3 change 1, phase 1).

## When it runs

- By hand: `gh workflow run windows-build.yml --repo StopBanning/locust -f label=v0.620.0`.
  The label is only shown in the summary; the version comes from
  `apps/desktop/package.json`.
- On a pushed tag `v*`. The tag must be `v` plus that version, or the
  hash step fails: a tag that disagrees with the package would put another
  version's name on the installer.
- Never on an ordinary push: the package takes minutes.

## What it does

The workflow sets up checkout, pnpm (from `packageManager`) and Node 24 with
pnpm's store cached, then runs five steps, each one
`bash _tools/ci-windows-local.sh <step>`:

| Step | What |
|---|---|
| install | `pnpm install --frozen-lockfile` |
| typecheck | builds `packages/runtime-adapters` and `packages/mission-store` (their types feed the desktop's), then `tsc --noEmit` on `apps/desktop`'s node and web configs |
| suites | `npx vitest run` in each folder of `_tools/ci-windows-suites.txt` |
| package | `vendor-npm.mjs`, `vendor-recall.mjs`, `pnpm build`, then `electron-builder --config electron-builder.yml --win --publish never` with `CSC_IDENTITY_AUTO_DISCOVERY=false` |
| hash | checks the installer, its blockmap, `latest.yml` (saying this version) and `app.asar` exist; prints version, commit, size, SHA-256 of the installer and of `app.asar`; writes them to the job summary |

Then `Locust-<version>-setup.exe`, `Locust-<version>-setup.exe.blockmap` and
`latest.yml` are kept as the run's artifact, `Locust-<version>-windows-<run id>`,
for 14 days. Nothing is published, nothing goes to `locust-releases`, no
secret is read, and the token is read-only.

## What it proves

- The commit installs from its lockfile on a clean Windows machine.
- Both desktop typechecks pass there.
- The adapter and mission-store suites pass there.
- The installer can be made there without anything on the laptop: the npm
  it ships and the recall model are fetched and hash-checked by the vendor
  scripts, as on the laptop.

## What it does not prove

- **Unsigned.** Windows SmartScreen says "unknown publisher", as it does for
  the laptop's builds. Signing comes with SignPath.
- **The desktop suite does not run.** `apps/desktop`'s vitest suite drives a
  window (`runtime-setup.test.ts`) and runs the memory-recall model
  (`memory-recall.test.ts`). It still runs only in `_tools/gate.sh` on the
  laptop, and so do the mutation sweeps and the drives in `_tools/drive-*.mjs`.
- **Not `ship.mjs`.** The release gate's other checks -- version not yet
  released, a clean tree, a changelog entry, what is inside `app.asar` -- are
  not repeated. This is a build to compare, not a release.
- **Not byte-identical to the laptop.** An NSIS installer carries its own build
  time, so the two installers of one commit need not hash alike. Compare
  `app.asar`'s hash (printed beside the installer's) and the size; if
  `app.asar` differs, that is worth finding out why.

## The first hour on a bare Windows machine (0.847)

Since 2026-10-30 the Windows release is the one `ci.yml` builds, and the PC
that opened every Windows build by hand (`_tools/install-without-node-drive.mjs`)
is retired. So `ci.yml`'s Windows package job now runs
`_smoke/windows-first-hour-smoke.mjs` on the installer it just made, after the
release's files are uploaded:

1. The installer runs as a person runs it, silently (`/S`), for this account,
   into `%LOCALAPPDATA%\Programs`.
2. The installed app starts with the PATH Explorer gives an account nobody has
   used, and an empty home and AppData. The smoke refuses to measure anything
   if node, npm or opencode can still be found where Locust looks.
3. The first screen, Install on the free runtime, the version it reports, a
   first message on the free model and its answer, each timed. A free model
   that fails or says nothing is switched the way a person would switch it.

Its frames and log are the run's `windows-first-hour-frames` artifact. Like the
Mac's first hour, it is reported, not required: a free provider can be down for
hours. To work on the smoke without a full build, run
`gh workflow run windows-first-hour.yml -f run_id=<a ci.yml run with a Windows build>`.
Given an installer, the smoke runs only on a CI runner, since it replaces the
Locust installed for that account; on a PC, give it an unpacked `Locust.exe`.

## The one difference from `ship.mjs`

`ship.mjs` packages with `pnpm --filter @teammate/desktop package`. Its
electron-builder runs with the default publish policy, and on a runner building
a tag (`GITHUB_REF_TYPE=tag`, which a workflow cannot override) that policy is
"onTag": the builder would try to upload to `locust-releases`. So the package
step calls electron-builder itself with `--publish never`. The package
script's other two parts are left out because neither changes what is
packaged: a second `electron-vite build` (`pnpm build` has just made `out/`)
and the prune of old installers in `release/`.

## Rehearsing it on the laptop

The steps live in `_tools/ci-windows-local.sh`, and the workflow calls that
script rather than repeating its commands, so the rehearsal and the run cannot
drift apart. From the repository root:

    bash _tools/ci-windows-local.sh

runs install, typecheck, suites, package and hash in order, prints the time
each took and ends `CI-WINDOWS LOCAL PASSED` with the installer's hash. On the
laptop the suites step first makes the gate's quiet-machine check and exits 75
(wait and retry, not red) while another vitest is running.

What the rehearsal cannot exercise is what the workflow file itself adds:
`actions/checkout`, `pnpm/action-setup` reading `packageManager`,
`actions/setup-node`'s pnpm cache, the `GITHUB_OUTPUT` and
`GITHUB_STEP_SUMMARY` writes being read back, the tag check (it needs
`GITHUB_REF_TYPE=tag`), and `actions/upload-artifact`. The first run on the
mirror is their test.

To add a suite, add its folder to `_tools/ci-windows-suites.txt`; both read it.

## Getting the file to the mirror

`_tools/publish-mirror.mjs` keeps the mirror's own `.github/workflows/` and the
export leaves this repository's out, so the file never travels on its own. The
main agent (never an executor) copies it by hand into the mirror's checkout:

1. Copy `.github/workflows/windows-build.yml` into the mirror's
   `.github/workflows/`, beside `ci.yml` and `mutation-nightly.yml`.
2. The workflow needs `_tools/ci-windows-local.sh` and
   `_tools/ci-windows-suites.txt`, which the export carries like the rest of
   `_tools/`, and points at this document, which is on the export's
   `DOCS_PUBLISHED` list. Push the mirror's source (publish-mirror.mjs) from a
   commit that has them before the workflow, or its first run fails at install.
3. Commit and push the mirror, then run it once by hand with a label and
   compare the summary's hashes with the laptop's build of the same commit.
