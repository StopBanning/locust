import { execFile } from 'node:child_process'

import { awayConfigOf, awayLook } from './scout-away-look.js'

/*
 * The away look's one file, as the workflow runs it: `node .github/locust/scout-away.mjs`, with the LOCUST_SCOUT
 * secret, the workflow's own token (GH_TOKEN) and the repository (GH_REPO), so `gh` reads that repository with that
 * token. Built with `node scripts/build-scout-away.mjs` into resources/scout-away.mjs.
 */
const config = awayConfigOf(process.env.LOCUST_SCOUT)
const repo = process.env.GH_REPO ?? process.env.GITHUB_REPOSITORY ?? ''
if (config === undefined) {
  console.log('The LOCUST_SCOUT secret is missing or cannot be read, so Scout does not look. Turn “Look while my computer is off” on in Locust to set it.')
} else if (!/^[\w.-]+\/[\w.-]+$/.test(repo)) {
  console.log('This look does not know its repository.')
  process.exitCode = 1
} else {
  const gh = (args: readonly string[]): Promise<string> => new Promise((resolve, reject) => {
    execFile('gh', [...args], { env: { ...process.env, GH_REPO: repo, GH_PROMPT_DISABLED: '1', NO_COLOR: '1' }, maxBuffer: 16 * 1024 * 1024, timeout: 60_000, windowsHide: true }, (error, stdout) => {
      if (error) reject(error)
      else resolve(stdout)
    })
  })
  const result = await awayLook({ config, repo, gh, fetch })
  console.log(result.said)
  if (!result.ok) process.exitCode = 1
}
