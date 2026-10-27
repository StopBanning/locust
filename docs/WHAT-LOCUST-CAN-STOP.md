# What Locust can stop

<!-- Written from apps/desktop/src/shared/what-locust-can-stop.ts. Edit that file, then run
     `npx vitest run src/main/what-locust-can-stop-is-what-it-asks.test.ts -u` in apps/desktop to rewrite this one. -->

Locust can stop a run only where its AI agent asks first. Where it asks, a card waits for you, your saved rules answer before it reaches you, and the record says who decided. Where it does not, the mode decides what the agent is given.

| AI agent | Can stop | Asks first | Always kept by |
| --- | --- | --- | --- |
| Codex CLI | Each action | in Approve each action | Locust |
| Claude Code | Each action | in Approve each action | Locust |
| OpenCode | Each action | in Approve each action | Locust |
| Copilot CLI | Each action | in Approve each action | Locust |
| Cursor Agent | Nothing | never | — |
| Antigravity | Questions only | its questions | — |
| Kilo | Nothing | never | — |
| Muse Code | Nothing | never | — |
| Your own agent | Some actions | what the agent asks about | Locust |

## Codex CLI

**Asks first.** In Approve each action: every command that does more than read, and every file change, before it runs.

**Without asking.** In Ask and Plan it works in Codex’s own sandbox, where its commands can read but change nothing; in Edit, where they can also change files in this folder. In Ask and Plan it starts none of the MCP servers or plugins your Codex runs as programs on this computer, nor its own browser or computer use. It asks nothing in those modes, or in Auto.

**Always.** Locust keeps an Always for the rest of the run, and your saved rules come first.

## Claude Code

**Asks first.** In Approve each action: every file change, command and connector call, before it runs. In Ask and Plan, every connector call. In Edit, a connector call the teammate was not given (or every one, when “Ask before every connector call” is on), a command it does not run on its own say, and a change to a file outside this folder.

**Without asking.** Reading files, in every mode. In Edit, changes to files in this folder and the commands Claude Code runs on its own say. In Auto, everything. In every mode, Locust refuses one kind of command without asking: one that ends a browser, Locust or another AI agent that no teammate started, by name or by its id, which would end yours too.

**Always.** Locust keeps an Always for the rest of the run, and your saved rules come first.

## OpenCode

**Asks first.** In Approve each action: every command, file change, web page, place outside this folder and tool from your own MCP servers, before it runs.

**Without asking.** Web searches, in every mode. In Ask and Plan, no file changes, no commands and none of your own MCP tools; in Edit, commands, the web and your own MCP tools, and nothing outside this folder. In Auto, everything.

**Always.** Locust keeps an Always for the rest of the run, and your saved rules come first.

## Copilot CLI

**Asks first.** In Approve each action: every command and file change, and each fetch or place outside this folder, before it runs.

**Without asking.** Reading files, in every mode. In Ask and Plan, no file changes, no commands and none of the MCP servers in your Copilot settings; in Edit, commands and changes in this folder. In Auto, everything.

**Always.** Locust keeps an Always for the rest of the run, and your saved rules come first.

## Cursor Agent

**Asks first.** Nothing: Cursor’s command line has no way to stop and ask Locust.

**Without asking.** In Ask and Plan, Cursor’s own read-only mode, with no MCP server started that a folder brings; your own MCP tools run only where your Cursor settings already allow them. In Edit, changes in this folder, and commands as Cursor’s own settings allow. In Auto, everything.

## Antigravity

**Asks first.** Only its questions, which you answer on a card. It never asks before an action.

**Without asking.** In Ask and Plan, no file changes and no commands; in Edit, file changes and no commands; in Auto, everything. What it refused is listed after the run.

## Kilo

**Asks first.** Nothing: Kilo runs without asking Locust.

**Without asking.** In Ask and Plan, Kilo’s ask mode: no file changes, and a tool it does not name is denied, including an MCP tool from a config Locust does not load. In Edit, Kilo’s code mode in this folder, and an action it asks about is refused. In Auto, that mode, and an action it asks about is allowed. Paid models and signing in have not been measured.

## Muse Code

**Asks first.** Nothing: Muse runs without asking Locust.

**Without asking.** In Ask and Plan, no file changes and no commands. In Edit, changes in this folder, commands inside Muse’s own sandbox, and the web. In Auto, everything, with commands outside that sandbox.

## Your own agent

**Asks first.** In Approve each action, its only mode: every action the agent asks about, before it runs. Which actions it asks about is the agent’s own setting.

**Without asking.** Whatever its own settings let it do without asking, in the folder it was given.

**Always.** Locust keeps an Always for the rest of the run, and your saved rules come first.
