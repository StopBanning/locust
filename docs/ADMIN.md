# Organization policy

An administrator can restrict Locust with one machine-wide `policy.json`.
Locust reads it at launch and never writes it. Settings > General shows
**Managed by your organization**, its path, and the settings it supplies.
A person's choices can be more restrictive, but cannot loosen the policy.
Restart Locust after deploying or changing the file.

| System | File |
| --- | --- |
| Windows | `%ProgramData%\Locust\policy.json` (normally `C:\ProgramData\Locust\policy.json`) |
| macOS | `/Library/Application Support/Locust/policy.json` |
| Linux | `/etc/locust/policy.json` |

Windows resolves the operating system's CommonApplicationData known folder
from the machine's registry (`reg.exe`, falling back to Windows PowerShell
without a profile or a window), using the tools in `C:\Windows\System32`.
Changing the process's `ProgramData` or `SystemRoot` variable cannot select a
different policy.
Protect both the directory and file: administrators own them, ordinary users
can read them, and ordinary users cannot write, replace or delete them.
Locust does not set those permissions or deploy the file itself.

No file means no organization restrictions. An unreadable file, invalid JSON,
an unsupported version, or a malformed known setting blocks turns and
connections. The window names the file and the problem; the app remains open
so the person can see it. Fix the file and restart. Unknown top-level keys
are ignored, with one line per key in the profile's `locust-errors.log`.

## Version 1 settings

The file is a JSON object. Every key is optional; an absent key imposes no
restriction. `version`, when supplied, must be `1`. JSON comments and trailing
commas are not accepted. Use UTF-8; a UTF-8 byte order mark is accepted. The
file is limited to 1 MB.

```json
{
  "version": 1,
  "allowedAgents": ["codex", "claude", "opencode"],
  "allowedModels": {
    "codex": ["gpt-*"],
    "claude": ["sonnet", "claude-sonnet-*"],
    "opencode": ["opencode/*-free"]
  },
  "highestMode": "approve-each",
  "monthlyLimitUsd": 50,
  "otherApps": false,
  "phone": false,
  "chatApps": false,
  "cloud": false,
  "terminal": false,
  "browser": false
}
```

Use the model IDs your installed agents report in Locust's picker. The example
patterns illustrate the syntax; they do not install agents, grant account
access, or promise that an agent supports a model or mode.

| Key | Meaning |
| --- | --- |
| `version` | Optional format version, currently `1`. |
| `allowedAgents` | Allowed runtime IDs: `codex`, `claude`, `cursor`, `gemini`, `opencode`, `copilot`, `antigravity`, `muse`, `omniroute`, `acp`. An empty array denies all agents. `acp` covers every agent the person adds under Your own agents. Listing a runtime does not enable a runtime Locust cannot run. |
| `allowedModels` | An object from runtime ID to an array of allowed model IDs or patterns. Matching is case-sensitive; `*` matches any number of characters, including `/`. All other characters are literal. A runtime absent from this object has no model restriction; an empty array denies all its models. |
| `highestMode` | Permission ceiling, ordered `ask`, `plan`, `approve-each`, `accept-edits`, `auto`. Ask and Plan are read-only. Approve each asks before consequential actions; Accept edits permits workspace changes without each prompt; Auto permits unrestricted actions. Runtime-specific restrictions still apply. A ceiling below Auto holds the Auto switch off. |
| `monthlyLimitUsd` | Maximum effective monthly limit per teammate, in USD: `0` or an amount from `0.01` through `100000`. A lower personal limit still applies. An absent or higher personal limit is capped automatically; a person cannot remove or raise the effective cap. `0` refuses new turns. |
| `otherApps` | `false` keeps Let your other AI apps use Locust off, including its local MCP listener, the `locust` command, and editors. `true` leaves the person's opt-in choice available. |
| `phone` | `false` keeps phone pairing off and prevents Claude Code Remote Control from being started through Locust. `true` leaves those choices available. |
| `chatApps` | `false` keeps teammates out of chat apps: Telegram cannot be connected, and a bot connected before stops being read. `true` leaves it available. |
| `cloud` | `false` prevents cloud computer setup and sign-in, cloud conversations, and legacy Codex Cloud and Claude cloud tasks. Existing computers can still be stopped or deleted. `true` leaves cloud choices available. |
| `terminal` | `false` disables the in-window Terminal panel. `true` leaves it available. The panel is also disabled by an agent/model allowlist, a highest mode below Auto, or an organization monthly cap: an unrestricted personal shell cannot enforce those restrictions. Closing the panel keeps shells running; closing tabs or quitting ends them. No terminal output is sent to an AI agent. |
| `accounts` | `false` prevents adding, signing in to, renaming or running added Claude Code and Codex accounts. The machine's existing sign-in is unchanged. Cancel and Remove remain available. |
| `browser` | `false` disables the in-window Browser panel and teammates' per-run browser tools. `true` leaves them available. |

A finite model allowlist requires an explicit model: an unresolved account
default is refused because Locust cannot know which model the runtime would
choose. `["*"]` allows defaults as well. For `acp`, patterns match the model ID
reported by that agent, after the internal `agent-id::` prefix; an agent with
no explicit model needs `["*"]`. Cursor's effort variants are model IDs too;
the picker removes blocked variants and the host checks the selected variant.

## What is enforced

The main process checks starts from the window, other apps, the command and
editors, routines and their handoff chains, relays, rooms, comparisons and their
judges, side questions, carry-on, resumes, and voice calls. A denied handoff is
checked before the existing turn is stopped. Refusals name the setting and
policy path. Controls remain visible and say **Set by your organization**;
the model picker leaves out disallowed models and the mode menu stops at the
ceiling. Personal opt-ins, approval cards, runtime permission restrictions and
the existing Auto switch still apply.

Some surfaces cannot guarantee a selected model or permission mode. Interactive
agent terminals and Remote Control are treated as Auto with an unknown model.
Legacy cloud tasks are treated as Auto; Codex Cloud and legacy continuations
have an unknown model. Voice calls use Codex with an unknown model and Approve
each, because connectors may ask to act; voice previews use Ask. These surfaces
are refused when their effective mode or unknown model conflicts with policy.
Handing work from a voice call to a teammate checks that teammate's turn again.

A plain cloud terminal cannot constrain which installed agent or model runs
there; an agent or model allowlist therefore keeps that terminal disabled.

Monthly limits use the existing ledger's reported dollar receipts, in this
computer's calendar month. Subscription token estimates, Copilot premium
requests and unpriced usage are not dollars. A running turn can cross its
limit; the next turn is refused. This is a start-time spending guard, not a
provider billing cutoff or a reservation for concurrent turns. An unreadable
spending record blocks starts when an organization cap applies.

For work without a teammate, such as comparison columns, judges and Scout, the
cap is checked against the whole month's recorded dollars. Omitting or naming
a missing teammate cannot bypass the cap. Interactive agent terminals, Remote
Control and legacy cloud sessions do not return complete cost receipts to
Locust, so an organization monthly cap prevents those launches. Managed cloud
computer conversations use the ordinary mission guard and ledger instead.

This policy governs turns and connections Locust starts. It is not an operating
system sandbox, an agent's internal model-routing policy, a tamper-proof billing
ledger, or a restriction on running an agent independently of Locust.

## Deploying the file

Keep policy deployment separate from the user's profile and backups. Use your
management system's privileged device context, protect the directory first,
validate the JSON, and replace the file atomically. Do not store account tokens
or credentials in policy. Removing the file removes its restrictions at the
next launch, so deployment rollback should replace it with the intended prior
policy rather than remove it accidentally.

### Intune (Windows)

Deploy a device PowerShell script or a Win32 app containing your approved JSON.
Run as SYSTEM, in 64-bit PowerShell, rather than the logged-on user. Resolve
`[Environment]::GetFolderPath('CommonApplicationData')`, create `Locust` there,
and restrict its ACL to SYSTEM and Administrators with full control and Users
with read/execute. Ordinary users must not retain inherited write, delete or
ownership rights. Write the validated JSON to a temporary file in that protected
directory and rename it to `policy.json`. Use a detection rule for the expected
content and ACL, not only file existence. Restart Locust through your normal
rollout process after replacement.

### Jamf (macOS)

Ship the file in a package or a root-run management policy. Create
`/Library/Application Support/Locust` owned by `root:wheel`, mode `0755`, and
install `policy.json` owned by `root:wheel`, mode `0644`. Stage and rename the
replacement inside that directory. A user preference or a file under the user's
Library does not supply machine policy. Check ownership, permissions and the
JSON as part of the Jamf policy, then restart Locust.

### Linux package or configuration management

Install `/etc/locust` as `root:root`, mode `0755`, and `policy.json` as
`root:root`, mode `0644`, using the package's configuration-file mechanism or
your configuration management system. Protect replacement and removal as well
as content writes. Preserve the administrator's policy across ordinary Locust
package upgrades. Stage the next file in `/etc/locust` and atomically rename
it, then restart Locust.

## Development tests

Only an unpackaged build reads `LOCUST_POLICY_FILE`, an absolute path to a test
file. A packaged build ignores that variable and always resolves the machine
path above. Automated tests use temporary files and fake agents; no test needs
to write the real machine-wide path or start a paid model turn.
