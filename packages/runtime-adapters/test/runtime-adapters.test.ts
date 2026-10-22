import { EventEmitter } from "node:events";
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import {
  assertSafeRuntimeCommand,
  COPILOT_REQUIRED_FEATURES,
  createClaudePrintCommand,
  createCodexExecCommand,
  createCopilotAcpCommand,
  createCopilotPromptCommand,
  createCursorPrintCommand,
  createGeminiPrintCommand,
  createOpenCodeRunCommand,
  CURSOR_REQUIRED_FEATURES,
  MUSE_REQUIRED_FEATURES,
  OPENCODE_READ_ONLY_CONFIG,
  OPENCODE_REQUIRED_FEATURES,
  parseClaudeModelHints,
  parseCursorModelList,
  parseOpenCodeModelList,
  createNodeProbeRunner,
  createPathExecutableLocator,
  discoverInstalledRuntimes,
  parseRuntimeVersion,
  withOpenCodeGitReads,
  withoutOpenCodeQuestionTool,
} from "../src/index.js";
import type {
  CommandRunner,
  ExecutableLaunch,
  ExecutableLocator,
  ProbeCommand,
  RuntimeReadiness,
  SpawnedProbeProcess,
} from "../src/index.js";

const nativeExecutable: ExecutableLaunch = {
  commandName: "codex",
  discoveredPath: "C:\\tools\\codex.exe",
  executablePath: "C:\\tools\\codex.exe",
  prefixArgs: [],
  kind: "native",
};

describe("runtime command specifications", () => {
  it("builds prompt-on-stdin, read-only Codex and restricted Claude commands", () => {
    const codex = createCodexExecCommand(nativeExecutable, {
      workspacePath: "C:\\workspace",
      model: "gpt-example",
    });
    expect(codex.args).toEqual([
      "exec",
      "--json",
      // Without this, a workspace that is not a git repository fails the run
      // before the model is reached. Containment is the sandbox argument
      // below, not Codex's guess about version control.
      "--skip-git-repo-check",
      "--sandbox",
      "read-only",
      "-C",
      "C:\\workspace",
      "--model",
      "gpt-example",
      "-",
    ]);
    expect(codex.stdin).toBe("prompt");

    const claude = createClaudePrintCommand(
      { ...nativeExecutable, commandName: "claude" },
      { workspacePath: "C:\\workspace" },
    );
    expect(claude.args).toContain("--restricted");
    expect(claude.args).toContain("stream-json");
    expect(claude.args).toContain("default");
    expect(claude.args).not.toContain("--dangerously-skip-permissions");
    // A2.10: the prompt goes as a stream-json user turn, input open while the
    // turn runs, so a teammate's message can reach it (measured 2026-09-25).
    expect(claude.args.slice(claude.args.indexOf("--input-format"), claude.args.indexOf("--input-format") + 2))
      .toEqual(["--input-format", "stream-json"]);
    expect(claude.stdin).toBe("stream-json");
  });

  it("holds a read-only Claude mission in default mode with a reading tool list plus Task, never plan mode", () => {
    const claude = createClaudePrintCommand(
      { ...nativeExecutable, commandName: "claude" },
      { workspacePath: "C:\\workspace", sandbox:"read-only" },
    );
    expect(claude.args).toContain("--restricted");
    expect(claude.args.join(" ")).toContain("--permission-mode default");
    // The connectors ride along -- `--restricted` never kept them out, which
    // was measured on 2026-09-09 -- but nothing that writes to this machine
    // does.
    expect(claude.args.join(" ")).toContain("--tools Read,Glob,Grep,Task,Skill,WebSearch,WebFetch,mcp__*");
    expect(claude.args.join(" ")).not.toContain("Edit");
    expect(claude.args.join(" ")).not.toContain("Bash");
  });

  it("lets a Claude mission that may edit actually edit, and run what it changed", () => {
    // MEASURED 2026-09-03 off `claude --help`: --permission-mode takes
    // acceptEdits, and --restricted removes the command-running tools only
    // when --tools does not name them. Before this, every Claude run was sent
    // `--permission-mode plan` whatever the composer asked, so "Accept edits"
    // in the composer sat beside "read-only" in the mission header.
    const claude = createClaudePrintCommand(
      { ...nativeExecutable, commandName: "claude" },
      { workspacePath: "C:\\workspace", sandbox:"workspace-write" },
    );
    expect(claude.args.join(" ")).toContain("--permission-mode acceptEdits");
    expect(claude.args.join(" ")).toContain("Edit,Write,NotebookEdit,Bash");
    // Still restricted: the person's own Claude settings stay out of a
    // mission, and the tool list is explicit either way.
    expect(claude.args).toContain("--restricted");
    // And it keeps the person's connectors, which `--restricted` was never
    // keeping out: only `--strict-mcp-config` would, and nothing sends that.
    expect(claude.args.join(" ")).not.toContain("--disallowedTools");
    expect(claude.args.join(" ")).toContain("Bash,Task,Skill,WebSearch,WebFetch,mcp__*");
  });

  it("rejects permission bypass arguments", () => {
    expect(() => assertSafeRuntimeCommand({
      runtime: "codex",
      executablePath: "C:\\tools\\codex.exe",
      args: ["exec", "--dangerously-bypass-approvals-and-sandbox"],
      cwd: "C:\\workspace",
      stdin: "prompt",
      stdout: "jsonl",
    })).toThrow("Forbidden runtime argument");
  });
});

describe("the Auto mode a person switches on", () => {
  // Colin, 2026-09-06: "there needs to be an auto option ... to allow them to
  // work out of the workspace folder if desired by the user". Every flag below
  // was read off that runtime's own --help the same day; the point of these
  // cases is that the widening is EXACTLY these flags and nothing near them.
  const workspacePath = "C:\\work\\repo";
  const spec = (runtime: string, args: readonly string[]) => ({
    runtime: runtime as never,
    executablePath: "C:\\x.exe",
    args,
    cwd: workspacePath,
    stdin: "prompt" as const,
    stdout: "jsonl" as const,
  });

  it("gives Claude Code bypassPermissions and the editing tools", () => {
    const claude = createClaudePrintCommand(
      { ...nativeExecutable, commandName: "claude" },
      { workspacePath, sandbox: "full-access" },
    );
    expect(claude.args.join(" ")).toContain("--permission-mode bypassPermissions");
    expect(claude.args.join(" ")).toContain("Edit,Write,NotebookEdit,Bash");
    // MEASURED 2026-09-06: the CLI refuses the two together -- "bypassPermissions
    // not supported in restricted mode" -- and exits before it says anything
    // else, which the app saw as a run that ended without a result. Auto is
    // the one mode that drops it; every other mode keeps it.
    expect(claude.args).not.toContain("--restricted");
    expect(claude.args.join(" ")).toContain("Bash,Task,Skill,WebSearch,WebFetch,mcp__*");
    expect(claude.args).not.toContain("--dangerously-skip-permissions");
    /*
     * EVERY MODE GETS THE PERSON'S CONNECTORS.
     *
     * This block used to assert the opposite -- `--disallowedTools mcp__*`
     * in every mode but Auto -- and carried a comment saying the denial
     * "costs nothing" because `--restricted` meant those servers were never
     * read. That was wrong, and the CLI's own help says so:
     *
     *   --restricted ... ignores user, project and local settings files
     *   (managed settings and --settings still apply; ADD --strict-mcp-config
     *   TO SKIP MCP SERVERS TOO)
     *
     * MEASURED 2026-09-09 by running the argv by hand in an empty temp
     * folder: `claude --restricted --print --tools "Read,Glob,Grep,Task,
     * mcp__*"` listed every connector on the account. `--restricted` never
     * blocked them; our denial did, and the app then told the person the
     * MODE was the reason.
     *
     * Colin asked whether Auto-only was standard. It is not -- Claude Code
     * makes connectors available in every permission mode -- and it made the
     * only route to a READ-ONLY connector call a run that may edit anything
     * on the machine.
     *
     * What the mode still decides is this machine. What it cannot decide is
     * a connector, because a connector acts somewhere else; that is said on
     * the mode menu instead of being enforced here.
     */
    for (const sandbox of ["read-only", "workspace-write"] as const) {
      const other = createClaudePrintCommand({ ...nativeExecutable, commandName: "claude" }, { workspacePath, sandbox });
      expect(other.args).toContain("--restricted");
      expect(other.args.join(" ")).not.toContain("--disallowedTools");
      expect(other.args.join(" ")).toContain("Task,Skill,WebSearch,WebFetch,mcp__*");
      // Never `--strict-mcp-config`: that IS the flag that would keep the
      // person's servers out, and nothing here wants that.
      expect(other.args).not.toContain("--strict-mcp-config");
    }
    // Read-only keeps its read-only tool list. Reaching a connector is not a
    // licence to run commands.
    const asking = createClaudePrintCommand({ ...nativeExecutable, commandName: "claude" }, { workspacePath, sandbox: "read-only" });
    expect(asking.args.join(" ")).not.toContain("Bash");
    expect(asking.args.join(" ")).not.toContain("Write");
  });

  it("gives Codex danger-full-access, which is its own name for it", () => {
    const codex = createCodexExecCommand(nativeExecutable, { workspacePath, sandbox: "full-access" });
    expect(codex.args.join(" ")).toContain("--sandbox danger-full-access");
    expect(codex.args).not.toContain("--dangerously-bypass-approvals-and-sandbox");
  });

  it("gives Cursor --force and never its --yolo alias", () => {
    const cursor = createCursorPrintCommand(nativeExecutable, { workspacePath, sandbox: "full-access" });
    expect(cursor.args).toContain("--force");
    expect(cursor.args).not.toContain("--yolo");
    // The read-only pairing is gone with it: plan mode and the sandbox are
    // what hold a run back, and this run is not being held back.
    expect(cursor.args).not.toContain("--sandbox");
  });

  it("gives Copilot --allow-all-paths and never --allow-all", () => {
    const copilot = createCopilotPromptCommand(nativeExecutable, {
      workspacePath,
      sandbox: "full-access",
      prompt: "go",
    });
    expect(copilot.args).toContain("--allow-all-paths");
    expect(copilot.args).not.toContain("--allow-all");
    expect(copilot.args).not.toContain("--allow-all-urls");
    expect(copilot.args.join(" ")).not.toContain("--deny-tool");
  });

  it("gives OpenCode --auto, drops the read-only denials, and says it may leave the folder", () => {
    const opencode = createOpenCodeRunCommand(nativeExecutable, {
      workspacePath,
      sandbox: "full-access",
      prompt: "go",
      model: "provider/model",
    });
    expect(opencode.args).toContain("--auto");
    // The read-only denials are gone, which is what Auto means.
    const config = JSON.parse(opencode.env?.OPENCODE_CONFIG_CONTENT ?? "{}");
    expect(config.permission?.edit).toBeUndefined();
    expect(config.permission?.write).toBeUndefined();
    // And it may leave the folder, stated. This asserted "deny" until
    // 2026-09-25, on the reading that `--auto` would approve past it; it
    // approves only what is NOT explicitly denied, so Auto stayed in the
    // folder. Measured the same day on the free model: "deny" refused an
    // outside read, "allow" returned the file.
    expect(config.permission?.external_directory).toBe("allow");
    // In a worktree too: Auto is the person's answer to where it may go.
    const inWorktree = createOpenCodeRunCommand(nativeExecutable, {
      workspacePath,
      sandbox: "full-access",
      prompt: "go",
      repositoryRoot: "C:\\work\\shop",
    });
    expect(JSON.parse(inWorktree.env?.OPENCODE_CONFIG_CONTENT ?? "{}")).toEqual({
      tools: { question: false },
      permission: { external_directory: "allow" },
    });
  });

  it("unlocks those two arguments only for a full-access run", () => {
    for (const argument of ["--force", "--allow-all-paths"]) {
      expect(() => assertSafeRuntimeCommand(spec("cursor", [argument]))).toThrow(/Forbidden/);
      expect(() => assertSafeRuntimeCommand(spec("cursor", [argument]), "workspace-write")).toThrow(/Forbidden/);
      expect(() => assertSafeRuntimeCommand(spec("cursor", [argument]), "full-access")).not.toThrow();
    }
    // The two that travel as a flag VALUE, in both spellings.
    for (const args of [["--sandbox", "danger-full-access"], ["--sandbox=danger-full-access"]]) {
      expect(() => assertSafeRuntimeCommand(spec("codex", args))).toThrow(/danger-full-access/);
      expect(() => assertSafeRuntimeCommand(spec("codex", args), "full-access")).not.toThrow();
    }
    for (const args of [["--permission-mode", "bypassPermissions"], ["--permission-mode=bypassPermissions"]]) {
      expect(() => assertSafeRuntimeCommand(spec("claude", args))).toThrow(/bypassPermissions/);
      expect(() => assertSafeRuntimeCommand(spec("claude", args), "full-access")).not.toThrow();
    }
  });

  it("still refuses everything else, Auto or not", () => {
    // Each of these is either wider than what was asked for, or about sending
    // the work somewhere else -- which no permission mode answers.
    for (const argument of [
      "--dangerously-bypass-approvals-and-sandbox",
      "--dangerously-skip-permissions",
      "--allow-dangerously-skip-permissions",
      "--dangerously-bypass-hook-trust",
      "--full-auto",
      "--yolo",
      "-y",
      "-f",
      "--ignore-rules",
      "--allow-all",
      "--allow-all-urls",
      "--share",
      "--remote",
      "--remote-export",
    ]) {
      expect(() => assertSafeRuntimeCommand(spec("codex", [argument]), "full-access")).toThrow(/Forbidden/);
    }
    expect(() => assertSafeRuntimeCommand(spec("codex", ["--ask-for-approval", "never"]), "full-access")).toThrow(/never/);
    expect(() => assertSafeRuntimeCommand(spec("gemini", ["--approval-mode", "yolo"]), "full-access")).toThrow(/yolo/);
  });

  it("reads an unstated sandbox as the strictest one", () => {
    // A caller that forgets to say cannot get Auto by omission.
    expect(() => assertSafeRuntimeCommand(spec("cursor", ["--force"]), undefined)).toThrow(/Forbidden/);
  });

  it("carries the sandbox on the spec, so the runner can judge what it spawns", () => {
    // The process runner validates the argv again at spawn time with nothing
    // but the spec. Without this it judged an Auto mission by the strictest
    // reading and refused it: "Runtime command failed safety validation",
    // measured driving the app on 2026-09-06.
    const cursor = createCursorPrintCommand(nativeExecutable, { workspacePath, sandbox: "full-access" });
    expect(cursor.sandbox).toBe("full-access");
    expect(() => assertSafeRuntimeCommand(cursor)).not.toThrow();

    const confined = createCursorPrintCommand(nativeExecutable, { workspacePath, sandbox: "workspace-write" });
    expect(confined.sandbox).toBe("workspace-write");
    expect(() => assertSafeRuntimeCommand({ ...confined, args: [...confined.args, "--force"] })).toThrow(/Forbidden/);
  });
});

describe("runtime version and executable discovery", () => {
  it("parses prerelease versions returned by installed CLIs", () => {
    expect(parseRuntimeVersion("codex-cli 0.151.0-alpha.7.2")?.version).toBe(
      "0.151.0-alpha.7.2",
    );
    expect(parseRuntimeVersion("2.1.251 (Claude Code)")?.version).toBe("2.1.251");
    // Copilot CLI ends its version line with a full stop. Read as a sentence
    // terminator, not as part of the number -- otherwise the CLI reports a
    // version and this build says it has none.
    expect(parseRuntimeVersion("GitHub Copilot CLI 1.0.82.")?.version).toBe("1.0.82");
    expect(parseRuntimeVersion("1.18.27")?.version).toBe("1.18.27");
  });

  it("prefers native Windows executables and safely wraps PowerShell shims", async () => {
    const existing = new Set([
      "c:\\tools\\codex.exe",
      "c:\\shims\\claude.ps1",
      "c:\\windows\\system32\\windowspowershell\\v1.0\\powershell.exe",
    ]);
    const locator = createPathExecutableLocator({
      platform: "win32",
      environment: {
        PATH: "C:\\tools;C:\\shims;C:\\Windows\\System32\\WindowsPowerShell\\v1.0",
      },
      isExecutableFile: async (candidate) => existing.has(candidate.toLowerCase()),
    });

    await expect(locator.find("codex")).resolves.toMatchObject({
      executablePath: "C:\\tools\\codex.exe",
      kind: "native",
    });
    await expect(locator.find("claude")).resolves.toMatchObject({
      executablePath: "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
      prefixArgs: [
        "-NoProfile",
        "-NonInteractive",
        "-File",
        "C:\\shims\\claude.ps1",
      ],
      kind: "powershell-shim",
    });
  });
});

describe("installed runtime discovery", () => {
  // H7: a CLI under the app's own Node is probed WITH its launch's environment.
  it("probes a CLI launched through the app binary with the launch's environment", async () => {
    const underOwnNode: ExecutableLaunch = {
      ...nativeExecutable,
      executablePath: "C:\Program Files\Locust\Locust.exe",
      prefixArgs: ["C:\npm\node_modules\@openai\codex\bin\codex.js"],
      kind: "node-shim",
      env: { ELECTRON_RUN_AS_NODE: "1" },
    };
    const seen: (Readonly<Record<string, string>> | undefined)[] = [];
    await discoverInstalledRuntimes({
      locator: { find: async (name) => (name === "codex" ? underOwnNode : undefined) },
      runner: { run: async (command) => { seen.push(command.env); return fakeProbe(command); } },
    });
    expect(seen.length).toBeGreaterThan(0);
    expect(seen.every((env) => env?.ELECTRON_RUN_AS_NODE === "1")).toBe(true);
  });

  it("separates installation, safe capability support, and authentication", async () => {
    const launches = new Map<string, ExecutableLaunch>([
      ["codex", nativeExecutable],
      ["claude", {
        ...nativeExecutable,
        commandName: "claude",
        discoveredPath: "C:\\tools\\claude.exe",
        executablePath: "C:\\tools\\claude.exe",
      }],
    ]);
    const locator: ExecutableLocator = {
      find: async (name) => launches.get(name),
    };
    const runner: CommandRunner = {
      run: async (command) => fakeProbe(command),
    };

    const result = await discoverInstalledRuntimes({ runner, locator });
    expect(result.map(({ id, availability, readiness }) => ({ id, availability, readiness }))).toEqual([
      { id: "codex", availability: "available", readiness: "authentication-required" },
      { id: "claude", availability: "available", readiness: "ready" },
      { id: "cursor", availability: "unavailable", readiness: "unknown" },
      { id: "gemini", availability: "unavailable", readiness: "unknown" },
      { id: "opencode", availability: "unavailable", readiness: "unknown" },
      { id: "copilot", availability: "unavailable", readiness: "unknown" },
      { id: "muse", availability: "unavailable", readiness: "unknown" },
      { id: "omniroute", availability: "unavailable", readiness: "unknown" },
    ]);
  });

  // Both CLIs measured on this machine the day they were installed.
  const CURSOR_HELP =
    "-p, --print  --output-format <format> text | json | stream-json  --stream-partial-output  --mode <mode> plan: read-only  --workspace <path>  --resume [chatId]  --model <model>";
  const GEMINI_HELP =
    "-p, --prompt  Run in non-interactive (headless) mode. Appended to input on stdin  -o, --output-format  text json stream-json  --approval-mode default auto_edit yolo plan  -r, --resume";

  function newcomerLocator(name: string): ExecutableLocator {
    return {
      find: async (command) =>
        command === name
          ? { ...nativeExecutable, commandName: name, discoveredPath: `C:\\tools\\${name}.exe`, executablePath: `C:\\tools\\${name}.exe` }
          : undefined,
    };
  }

  it("reads a logged-out Cursor Agent from its text, because its status command exits 0 either way", async () => {
    const runner: CommandRunner = {
      run: async (command) => {
        if (command.purpose === "version") return { exitCode: 0, stdout: "2026.08.31-4057e58", stderr: "" };
        if (command.purpose === "capabilities") return { exitCode: 0, stdout: CURSOR_HELP, stderr: "" };
        return { exitCode: 0, stdout: "Not logged in\n", stderr: "" };
      },
    };
    const [cursor] = (await discoverInstalledRuntimes({ runner, locator: newcomerLocator("cursor-agent") }))
      .filter((entry) => entry.id === "cursor");
    expect(cursor?.availability).toBe("available");
    expect(cursor?.readiness).toBe("authentication-required");
    expect(cursor?.supportedFeatures).toEqual(expect.arrayContaining([...CURSOR_REQUIRED_FEATURES]));
  });

  // The first lines of the real list, as printed on 2026-09-02.
  const CURSOR_MODELS = "Available models\n\nauto - Auto (default)\ncursor-grok-4.6-high - Cursor Grok 4.6\ncomposer-2.5 - Composer 2.5\n";

  /*
   * THE EXACT BYTES `muse --version` PRINTS, and the reason this test exists.
   *
   * The readiness rule shipped as a regex for a literal backslash followed
   * by an `s` -- a heredoc had eaten the escape -- so it could never match
   * this line. Discovery therefore never called Muse ready, and Settings
   * read "Muse Code 1.3.0 - CHECKING - did not answer its version probe in
   * time" on a machine where that probe answers in 400ms. Every unit test we
   * had passed, because none of them had the runtime's own stdout in front
   * of it. This one does.
   */
  const MUSE_VERSION = "Muse Code 1.3.0 (1.3.0-R3401.1)\n";
  // `muse exec --help`, the page that names the flags a mission passes. The
  // top-level `muse --help` names none of them, which is how the capability
  // probe came to be pointed at the wrong page.
  const MUSE_HELP = readFileSync(new URL("./fixtures/muse/exec-help.txt", import.meta.url), "utf8");

  const museRunner: CommandRunner = {
    run: async (command) =>
      command.purpose === "capabilities"
        ? { exitCode: 0, stdout: MUSE_HELP, stderr: "" }
        : { exitCode: 0, stdout: MUSE_VERSION, stderr: "" },
  };

  /** Discovery with the credential lookup answered by this test, not the machine. */
  async function museWith(lookup: { homeDirectory: string; variables: Record<string, string | undefined> }, signedIn: boolean) {
    const found = await discoverInstalledRuntimes({
      runner: museRunner,
      locator: newcomerLocator("muse"),
      credentialLookup: lookup,
      // The only file this test admits to existing is the credential file,
      // and only when it is meant to be signed in.
      statFile: async (path: string) => {
        if (signedIn && /auth\.json$/.test(path)) return { size: 64, mtimeMs: 1 };
        throw new Error("ENOENT");
      },
    });
    return found.filter((entry) => entry.id === "muse")[0];
  }

  it("calls Muse Code ready from the line its own version command prints", async () => {
    const muse = await museWith({ homeDirectory: "C:\Users\dev", variables: {} }, true);
    expect(muse?.availability).toBe("available");
    expect(muse?.readiness).toBe("ready");
    expect(muse?.version?.version).toBe("1.3.0");
    expect(muse?.supportedFeatures).toEqual(expect.arrayContaining([...MUSE_REQUIRED_FEATURES]));
    // Credentials exist, but whether they are valid and what plan they carry
    // still costs a run to find out, so the caveat rides along.
    expect(JSON.stringify(muse)).toContain("readiness-unverifiable");
  });

  it("says nobody has signed in rather than calling Muse Code ready", async () => {
    /*
     * Colin, 2026-09-21, before paying for a plan: *"if it cant detect the
     * account that the user has this is kind of brutal to add and force this
     * process on the user"*.
     *
     * Muse has no free sign-in probe -- its only one is a real run, which
     * costs money -- so readiness fell back to the version and the app called
     * a never-signed-in machine READY. The person then met
     * `missing meta credentials` mid-mission. One stat settles it.
     */
    const muse = await museWith({ homeDirectory: "C:\Users\dev", variables: {} }, false);
    expect(muse?.availability).toBe("available");
    expect(muse?.readiness).toBe("authentication-required");
    expect(JSON.stringify(muse)).toContain("nobody has signed in on this machine");
    expect(JSON.stringify(muse)).toContain("muse login");
    // And it must NOT claim the weaker "we cannot tell" caveat: we can tell.
    expect(JSON.stringify(muse)).not.toContain("readiness-unverifiable");
  });

  it("does not take an API key as a credential, because no run is given one", async () => {
    /*
     * M6 (the code review): this counted META_API_KEY as signed in -- muse
     * login --help: "META_API_KEY always takes priority over the account
     * login" -- but a run's environment is an allowlist that deliberately
     * carries no provider key (process-runner.ts), so Muse showed ready and
     * every run failed for missing credentials. What a run can use is the
     * login, so the login is what is checked.
     */
    const muse = await museWith(
      { homeDirectory: "C:\Users\dev", variables: { META_API_KEY: "sk-not-a-real-key" } },
      false,
    );
    expect(muse?.readiness).toBe("authentication-required");
  });

  it("looks where Muse looks when XDG_CONFIG_HOME is set", async () => {
    /*
     * Muse's bundled help (1.3.0): the config root is `$XDG_CONFIG_HOME/muse`,
     * ELSE `$HOME/.config/muse`. On a machine with the variable set, a login
     * writes under it, and a check of the home path alone would draw SIGN IN
     * over somebody who had just signed in -- the exact complaint the check
     * was written to answer.
     */
    const at = (existing: string) =>
      discoverInstalledRuntimes({
        runner: museRunner,
        locator: newcomerLocator("muse"),
        credentialLookup: { homeDirectory: "/home/dev", variables: { XDG_CONFIG_HOME: "/xdg" } },
        statFile: async (path: string) => {
          if (path.replace(/\\/g, "/") === existing) return { size: 64, mtimeMs: 1 };
          throw new Error("ENOENT");
        },
      }).then((found) => found.filter((entry) => entry.id === "muse")[0]);

    expect((await at("/xdg/muse/auth.json"))?.readiness).toBe("ready");
    // The control: with the variable set, Muse does not read the home path,
    // so a file left there is not a login Muse will use.
    expect((await at("/home/dev/.config/muse/auth.json"))?.readiness).toBe("authentication-required");
  });

  it("does not claim nobody signed in when it cannot look", async () => {
    // No home directory to look under is not evidence of anything. Guessing
    // SIGN IN there would be the same defect pointed the other way.
    const muse = await museWith({ homeDirectory: "", variables: {} }, false);
    expect(muse?.readiness).toBe("ready");
  });

  it("reports a signed-in Cursor Agent ready and reads its models off --list-models", async () => {
    const runner: CommandRunner = {
      run: async (command) => {
        if (command.purpose === "version") return { exitCode: 0, stdout: "2026.08.31-4057e58", stderr: "" };
        if (command.purpose === "capabilities") return { exitCode: 0, stdout: CURSOR_HELP, stderr: "" };
        if (command.purpose === "models") {
          expect(command.args).toEqual(["--list-models"]);
          return { exitCode: 0, stdout: CURSOR_MODELS, stderr: "" };
        }
        return { exitCode: 0, stdout: "Logged in as someone@example.com\n", stderr: "" };
      },
    };
    const [cursor] = (await discoverInstalledRuntimes({ runner, locator: newcomerLocator("cursor-agent") }))
      .filter((entry) => entry.id === "cursor");
    expect(cursor?.readiness).toBe("ready");
    expect(cursor?.modelHints?.models).toEqual([
      { id: "auto", displayName: "Auto (default)" },
      { id: "cursor-grok-4.6-high", displayName: "Cursor Grok 4.6" },
      { id: "composer-2.5", displayName: "Composer 2.5" },
    ]);
    expect(cursor?.modelHints?.efforts).toEqual([]);
  });

  it("reads a model list the CLI printed on stderr, and says so when it cannot read one", async () => {
    const runner: CommandRunner = {
      run: async (command) => {
        if (command.purpose === "version") return { exitCode: 0, stdout: "2026.08.31-4057e58", stderr: "" };
        if (command.purpose === "capabilities") return { exitCode: 0, stdout: CURSOR_HELP, stderr: "" };
        // The one model listing this repo has captured arrived on stderr.
        if (command.purpose === "models") return { exitCode: 0, stdout: "", stderr: CURSOR_MODELS };
        return { exitCode: 0, stdout: "Logged in as someone@example.com", stderr: "" };
      },
    };
    const [cursor] = (await discoverInstalledRuntimes({ runner, locator: newcomerLocator("cursor-agent") }))
      .filter((entry) => entry.id === "cursor");
    expect(cursor?.modelHints?.models?.map((model) => model.id)).toEqual([
      "auto",
      "cursor-grok-4.6-high",
      "composer-2.5",
    ]);
  });

  it("says a model list it cannot read is unreadable, rather than leaving the picker silently empty", async () => {
    const runner: CommandRunner = {
      run: async (command) => {
        if (command.purpose === "version") return { exitCode: 0, stdout: "2026.08.31-4057e58", stderr: "" };
        if (command.purpose === "capabilities") return { exitCode: 0, stdout: CURSOR_HELP, stderr: "" };
        if (command.purpose === "models") return { exitCode: 0, stdout: "a completely different layout", stderr: "" };
        return { exitCode: 0, stdout: "Logged in as someone@example.com", stderr: "" };
      },
    };
    const [cursor] = (await discoverInstalledRuntimes({ runner, locator: newcomerLocator("cursor-agent") }))
      .filter((entry) => entry.id === "cursor");
    expect(cursor?.modelHints).toBeUndefined();
    expect(cursor?.diagnostics.some((issue) => /does not recognise/.test(issue.message))).toBe(true);
  });

  it("reads no models from a Cursor that is signed out, and none from a list it cannot read", async () => {
    expect(parseCursorModelList("Error: Authentication required.")).toBeUndefined();
    expect(parseCursorModelList("")).toBeUndefined();
    // A duplicated id is listed once; a line without the dash is not a model.
    expect(parseCursorModelList("x - X\nx - X again\njust words")?.models).toEqual([{ id: "x", displayName: "X" }]);
  });

  it("reads a Gemini sign-in that Google then refused, which exits 0 with the refusal in its text", async () => {
    const refusal = readFileSync(new URL("./fixtures/gemini-ineligible-tier.txt", import.meta.url), "utf8");
    const runner: CommandRunner = {
      run: async (command) => {
        if (command.purpose === "version") return { exitCode: 0, stdout: "0.58.0", stderr: "" };
        if (command.purpose === "capabilities") return { exitCode: 0, stdout: GEMINI_HELP, stderr: "" };
        return { exitCode: 0, stdout: "No previous sessions found for this project.", stderr: refusal };
      },
    };
    const [gemini] = (await discoverInstalledRuntimes({ runner, locator: newcomerLocator("gemini") }))
      .filter((entry) => entry.id === "gemini");
    expect(gemini?.readiness).toBe("authentication-required");
  });

  it("does not ask Gemini to sign in when the caller does not run it, and says nothing is wrong", async () => {
    /*
     * `--list-sessions` starts the whole CLI: 3.2 s warm and 7.4 s cold on
     * Colin's machine, the slowest probe of every launch (2026-09-22), for a
     * runtime Locust lists as planned. A caller that will never use the
     * answer passes `readinessFromVersion` and the probe is not spawned.
     */
    const asked: string[] = [];
    const runner: CommandRunner = {
      run: async (command) => {
        asked.push(command.purpose);
        if (command.purpose === "version") return { exitCode: 0, stdout: "0.58.0", stderr: "" };
        if (command.purpose === "capabilities") return { exitCode: 0, stdout: GEMINI_HELP, stderr: "" };
        return { exitCode: 41, stdout: "", stderr: "Please set an Auth method" };
      },
    };
    const [gemini] = (await discoverInstalledRuntimes({
      runner,
      locator: newcomerLocator("gemini"),
      readinessFromVersion: new Set(["gemini"]),
    })).filter((entry) => entry.id === "gemini");
    expect(asked).not.toContain("readiness");
    expect(gemini?.availability).toBe("available");
    expect(gemini?.version?.version).toBe("0.58.0");
    // The control is the test above: without the option, the same runner's
    // readiness answer is read, and a signed-out Gemini says so.
  });

  it("asks only the runtimes it is told to, and spawns nothing for the rest", async () => {
    // A run starting on one runtime needs one answer; the sweep that asked
    // all of them made the start wait for the slowest (2026-09-22).
    const asked: string[] = [];
    const locator: ExecutableLocator = {
      find: async (command) => {
        asked.push(`find ${command}`);
        return { ...nativeExecutable, commandName: command, discoveredPath: `C:\\tools\\${command}.exe`, executablePath: `C:\\tools\\${command}.exe` };
      },
    };
    const runner: CommandRunner = {
      run: async (command) => {
        asked.push(`${command.purpose} ${command.executablePath}`);
        return { exitCode: 0, stdout: "2.1.0 (Claude Code)", stderr: "" };
      },
    };
    const found = await discoverInstalledRuntimes({ runner, locator, only: new Set(["claude"]) });
    expect(found.map((entry) => entry.id)).toEqual(["claude"]);
    expect(asked.every((line) => /claude/.test(line))).toBe(true);
  });

  it("asks a CLI whose readiness IS its version question once, not twice", async () => {
    // Copilot's readiness is `--version`: it was spawned beside the version
    // probe and again every launch after the version was remembered.
    const COPILOT_HELP_FOR_READINESS =
      "-p, --prompt <prompt>  --output-format <format> text | json  --allow-all-tools  --deny-tool <tools>  --session-id  --resume";
    const asked: { readonly purpose: string; readonly args: readonly string[] }[] = [];
    const runner: CommandRunner = {
      run: async (command) => {
        asked.push({ purpose: command.purpose, args: command.args });
        return {
          exitCode: 0,
          stdout: command.purpose === "capabilities" ? COPILOT_HELP_FOR_READINESS : "GitHub Copilot CLI 1.0.82.",
          stderr: "",
        };
      },
    };
    const [copilot] = (await discoverInstalledRuntimes({ runner, locator: newcomerLocator("copilot") }))
      .filter((entry) => entry.id === "copilot");
    expect(copilot?.readiness).toBe("ready");
    expect(asked.filter((entry) => entry.args.join(" ") === "--version")).toHaveLength(1);
    expect(asked.map((entry) => entry.purpose)).not.toContain("readiness");
  });

  it("reads Gemini CLI's sign-in state from its session listing's exit code", async () => {
    const answers = new Map<number, RuntimeReadiness>([[41, "authentication-required"], [0, "ready"]]);
    for (const [exitCode, expected] of answers) {
      const runner: CommandRunner = {
        run: async (command) => {
          if (command.purpose === "version") return { exitCode: 0, stdout: "0.58.0", stderr: "" };
          if (command.purpose === "capabilities") return { exitCode: 0, stdout: GEMINI_HELP, stderr: "" };
          expect(command.args).toEqual(["--list-sessions"]);
          return { exitCode, stdout: "", stderr: exitCode === 0 ? "" : "Please set an Auth method" };
        },
      };
      const [gemini] = (await discoverInstalledRuntimes({ runner, locator: newcomerLocator("gemini") }))
        .filter((entry) => entry.id === "gemini");
      expect(gemini?.readiness).toBe(expected);
    }
  });
});

describe("Cursor Agent and Gemini CLI commands", () => {
  const workspacePath = "C:\\work\\repo";

  it("runs a read-only Cursor mission in ask mode, with the sandbox where it runs, and never forces commands", () => {
    const spec = createCursorPrintCommand(nativeExecutable, { workspacePath });
    expect(spec.runtime).toBe("cursor");
    expect(spec.stdin).toBe("prompt");
    // Ask mode, not plan mode: plan mode wrote files when pushed (2026-09-02),
    // ask mode refused three pushed writes (2026-09-30). The sandbox too
    // wherever Cursor can run one: macOS and Linux, never Windows.
    const withSandbox = process.platform === "darwin" || process.platform === "linux";
    expect(spec.args).toEqual([
      "--print", "--output-format", "stream-json", "--stream-partial-output", "--trust", "--workspace", workspacePath, "--mode", "ask",
      ...(withSandbox ? ["--sandbox", "enabled"] : []),
    ]);
    expect(spec.args).not.toContain("plan");
    expect(spec.args).not.toContain("--force");
    // A large edit's completion holds the file twice and is past the record
    // cap. Cursor asks for the same stand-in Claude does, so that call closes.
    expect(spec.oversizedStandIns).toBe(true);
  });

  it("approves the connectors the person already configured, because nobody is here to be asked -- outside Ask and Plan", () => {
    /*
     * A headless run cannot answer Cursor's MCP approval prompt, so without
     * this the prompt resolves to "no" and every connector call fails.
     * MEASURED against a real configured server, 2026-09-11: "Failed: user
     * rejected MCP `robinhood-trading-get_accounts`" -- a rejection, not an
     * auth failure, which is why the same connector worked in Cursor's own
     * app and not here.
     *
     * Not in Ask or Plan (0.823). MEASURED 2026-10-22: with it, an Ask run
     * told to use no tool still started the command a folder's own
     * `.cursor/mcp.json` names; without it that server never started, and
     * the servers the person set up in Cursor still loaded.
     */
    expect(createCursorPrintCommand(nativeExecutable, { workspacePath, sandbox: "read-only" }).args).not.toContain("--approve-mcps");
    expect(createCursorPrintCommand(nativeExecutable, { workspacePath }).args).not.toContain("--approve-mcps");
    for (const sandbox of ["workspace-write", "full-access"] as const) {
      expect(createCursorPrintCommand(nativeExecutable, { workspacePath, sandbox }).args).toContain("--approve-mcps");
    }
  });

  it("lets a workspace-write Cursor mission edit, still without --force", () => {
    const spec = createCursorPrintCommand(nativeExecutable, { workspacePath, sandbox: "workspace-write", model: "composer-1", resumeThreadId: "chat_1" });
    expect(spec.args).not.toContain("--mode");
    expect(spec.args).not.toContain("--force");
    expect(spec.args.slice(-4)).toEqual(["--model", "composer-1", "--resume", "chat_1"]);
  });

  it("refuses an effort for a runtime that has no effort flag", () => {
    expect(() => createCursorPrintCommand(nativeExecutable, { workspacePath, effort: "high" })).toThrow(/effort/);
    expect(() => createGeminiPrintCommand(nativeExecutable, { workspacePath, effort: "high" })).toThrow(/effort/);
  });

  it("runs Gemini headless with the prompt on stdin, trusting only this workspace", () => {
    const spec = createGeminiPrintCommand(nativeExecutable, { workspacePath });
    expect(spec.runtime).toBe("gemini");
    expect(spec.stdin).toBe("prompt");
    expect(spec.args).toEqual(["--output-format", "stream-json", "--skip-trust", "--approval-mode", "plan"]);
    const writing = createGeminiPrintCommand(nativeExecutable, { workspacePath, sandbox: "workspace-write", model: "gemini-3-flash", resumeThreadId: "s1" });
    expect(writing.args).toContain("auto_edit");
    expect(writing.args.slice(-4)).toEqual(["--model", "gemini-3-flash", "--resume", "s1"]);
  });

  it("refuses OpenCode's session sharing and Copilot's ways out of the workspace", () => {
    // `--share` publishes the transcript to opencode.ai and prints a public
    // link; the Copilot flags each widen a run past the directory the host
    // chose, or move it onto GitHub's servers.
    for (const argument of [
      "--share",
      "--allow-all-paths",
      "--allow-all-urls",
      "--allow-all",
      "--remote",
      "--remote-export",
      "--enable-memory",
    ]) {
      expect(() =>
        assertSafeRuntimeCommand({ runtime: "opencode", executablePath: "C:\\x.exe", args: [argument], cwd: workspacePath, stdin: "none", stdout: "jsonl" }),
      ).toThrow(/Forbidden/);
    }
    // `--allow-all-tools` is NOT one of them: it is what makes a headless run
    // possible at all, and the denylist is what holds it back.
    expect(() =>
      assertSafeRuntimeCommand({ runtime: "copilot", executablePath: "C:\\x.exe", args: ["--allow-all-tools"], cwd: workspacePath, stdin: "none", stdout: "jsonl" }),
    ).not.toThrow();
  });

  it("refuses Gemini's yolo mode and Cursor's force flag in any spelling", () => {
    for (const args of [["--approval-mode", "yolo"], ["--approval-mode=yolo"], ["-y"], ["--force"], ["-f"], ["--yolo"]]) {
      expect(() =>
        assertSafeRuntimeCommand({ runtime: "gemini", executablePath: "C:\\x.exe", args, cwd: workspacePath, stdin: "prompt", stdout: "jsonl" }),
      ).toThrow(/Forbidden/);
    }
  });

  it("finds OpenCode's and Copilot's launchers in the npm bin directory without PATH", async () => {
    // Both install from npm, which puts its global launchers here and does
    // not always put that directory on a packaged app's PATH.
    const ROAMING = "C:\\Users\\x\\AppData\\Roaming";
    const powershell = "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe";
    const locator = createPathExecutableLocator({
      platform: "win32",
      environment: { PATH: "", APPDATA: ROAMING, LOCALAPPDATA: "C:\\Users\\x\\AppData\\Local", SystemRoot: "C:\\Windows" },
      isExecutableFile: async (candidate) =>
        candidate === `${ROAMING}\\npm\\opencode.ps1`
        || candidate === `${ROAMING}\\npm\\copilot.ps1`
        || candidate === powershell,
      readDirectory: async () => [],
    });
    await expect(locator.find("opencode")).resolves.toMatchObject({
      discoveredPath: `${ROAMING}\\npm\\opencode.ps1`,
      kind: "powershell-shim",
    });
    await expect(locator.find("copilot")).resolves.toMatchObject({
      discoveredPath: `${ROAMING}\\npm\\copilot.ps1`,
      kind: "powershell-shim",
    });
  });

  it("prefers a shim on PATH over an executable in a guessed install directory", async () => {
    // MEASURED 2026-09-03 against the locator itself. Codex CLI installed
    // from npm lands as `codex.cmd` and `codex.ps1`, never an `.exe`, while
    // an older copy sat under %LOCALAPPDATA%\OpenAI with a real `.exe`. The
    // locator swept every directory for an `.exe` before it swept any for a
    // `.ps1`, so the older copy won and installing a newer runtime changed
    // nothing about what actually ran.
    const ROAMING = "C:\\Users\\x\\AppData\\Roaming";
    const LOCAL = "C:\\Users\\x\\AppData\\Local";
    const powershell = "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe";
    const onPath = `${ROAMING}\\npm\\codex.ps1`;
    const guessed = `${LOCAL}\\OpenAI\\Codex\\bin\\abc123\\codex.exe`;
    const found = await createPathExecutableLocator({
      platform: "win32",
      environment: {
        PATH: `${ROAMING}\\npm`,
        APPDATA: ROAMING,
        LOCALAPPDATA: LOCAL,
        SystemRoot: "C:\\Windows",
      },
      isExecutableFile: async (candidate) =>
        candidate === onPath || candidate === guessed || candidate === powershell,
      readDirectory: async (directory) =>
        directory === `${LOCAL}\\OpenAI\\Codex\\bin` ? ["abc123"] : [],
    }).find("codex");

    expect(found?.discoveredPath).toBe(onPath);
    expect(found?.kind).toBe("powershell-shim");
  });

  it("runs a .cmd shim through cmd.exe, and prefers it to the .ps1 npm writes beside it", async () => {
    // MEASURED 2026-09-03. npm writes `codex.cmd` and `codex.ps1` together.
    // The `.ps1` cannot take Codex's arguments: a bare `-` for
    // prompt-on-stdin makes PowerShell's parameter binder reject the call
    // outright, so preferring it broke every Codex mission for anyone whose
    // Codex came from npm. `.cmd` under cmd.exe is what a terminal runs.
    const ROAMING = "C:\\Users\\x\\AppData\\Roaming";
    const cmdShim = `${ROAMING}\\npm\\codex.cmd`;
    const psShim = `${ROAMING}\\npm\\codex.ps1`;
    const shell = "C:\\Windows\\System32\\cmd.exe";
    const found = await createPathExecutableLocator({
      platform: "win32",
      environment: {
        PATH: `${ROAMING}\\npm`,
        APPDATA: ROAMING,
        LOCALAPPDATA: "C:\\Users\\x\\AppData\\Local",
        SystemRoot: "C:\\Windows",
      },
      isExecutableFile: async (candidate) =>
        candidate === cmdShim || candidate === psShim || candidate === shell,
      readDirectory: async () => [],
    }).find("codex");

    expect(found).toMatchObject({
      discoveredPath: cmdShim,
      executablePath: shell,
      prefixArgs: ["/d", "/s", "/c", cmdShim],
      kind: "cmd-shim",
    });
  });

  it("resolves an npm .cmd shim past cmd.exe to the node script it wraps", async () => {
    // MEASURED 2026-09-05. cmd.exe ends a command line at the first newline,
    // so a runtime that takes its prompt as an argument lost every flag after
    // a multi-line prompt -- including `--deny-tool=write,shell`. A read-only
    // Copilot run silently had write access. Under node directly there is no
    // shell to truncate anything.
    const ROAMING = "C:\\Users\\x\\AppData\\Roaming";
    const cmdShim = `${ROAMING}\\npm\\copilot.cmd`;
    const script = `${ROAMING}\\npm\\node_modules\\@github\\copilot\\npm-loader.js`;
    const node = "C:\\Program Files\\nodejs\\node.exe";
    const shell = "C:\\Windows\\System32\\cmd.exe";
    const found = await createPathExecutableLocator({
      platform: "win32",
      environment: {
        PATH: `${ROAMING}\\npm;C:\\Program Files\\nodejs`,
        APPDATA: ROAMING,
        LOCALAPPDATA: "C:\\Users\\x\\AppData\\Local",
        SystemRoot: "C:\\Windows",
      },
      isExecutableFile: async (candidate) => candidate === cmdShim || candidate === node || candidate === shell,
      readDirectory: async () => [],
      // The exact line npm writes, lifted from the real shim on 2026-09-05.
      readFile: async (path) =>
        path === cmdShim
          ? 'endLocal & goto #_undefined_# 2>NUL || title %COMSPEC% & "%_prog%"  "%dp0%\\node_modules\\@github\\copilot\\npm-loader.js" %*\r\n'
          : undefined,
    }).find("copilot");

    expect(found).toMatchObject({
      discoveredPath: cmdShim,
      executablePath: node,
      prefixArgs: [script],
      kind: "node-shim",
    });
  });

  it("resolves the shim of an install that is not global, which points one folder up", async () => {
    // MEASURED 2026-10-07: the runtime canary installs a new Copilot with
    // `npm install --prefix <folder>`, and npm writes its shim inside
    // node_modules\.bin as `%dp0%\..\@github\...`. Only the global shape was
    // known, so the run fell to cmd.exe and a multi-line prompt was refused.
    const prefix = "C:\\work\\.scratch\\runtime-canary\\copilot-1.0.93";
    const bin = `${prefix}\\node_modules\\.bin`;
    const cmdShim = `${bin}\\copilot.cmd`;
    const script = `${prefix}\\node_modules\\@github\\copilot\\npm-loader.js`;
    const node = "C:\\Program Files\\nodejs\\node.exe";
    const shell = "C:\\Windows\\System32\\cmd.exe";
    const found = await createPathExecutableLocator({
      platform: "win32",
      environment: { PATH: `${bin};C:\\Program Files\\nodejs`, SystemRoot: "C:\\Windows" },
      isExecutableFile: async (candidate) => candidate === cmdShim || candidate === node || candidate === shell,
      readDirectory: async () => [],
      // The exact line npm wrote into that shim.
      readFile: async (path) =>
        path === cmdShim
          ? 'endLocal & goto #_undefined_# 2>NUL || title %COMSPEC% & "%_prog%"  "%dp0%\\..\\@github\\copilot\\npm-loader.js" %*\r\n'
          : undefined,
    }).find("copilot");

    expect(found).toMatchObject({
      discoveredPath: cmdShim,
      executablePath: node,
      prefixArgs: [script],
      kind: "node-shim",
    });
  });

  it("prefers the node.exe npm keeps beside its shim, the way the shim itself does", async () => {
    const ROAMING = "C:\\Users\\x\\AppData\\Roaming";
    const cmdShim = `${ROAMING}\\npm\\opencode.cmd`;
    const beside = `${ROAMING}\\npm\\node.exe`;
    const onPath = "C:\\Program Files\\nodejs\\node.exe";
    const found = await createPathExecutableLocator({
      platform: "win32",
      environment: { PATH: `${ROAMING}\\npm;C:\\Program Files\\nodejs`, APPDATA: ROAMING, SystemRoot: "C:\\Windows" },
      isExecutableFile: async (candidate) => candidate === cmdShim || candidate === beside || candidate === onPath,
      readDirectory: async () => [],
      readFile: async () => '"%_prog%"  "%dp0%\\node_modules\\opencode-ai\\bin\\opencode.js" %*',
    }).find("opencode");

    expect(found?.executablePath).toBe(beside);
    expect(found?.kind).toBe("node-shim");
  });

  it("resolves an npm .cmd shim around a native binary to the binary itself", async () => {
    // OpenCode and Claude Code ship a real .exe and npm wraps it in a .cmd.
    // There is nothing for node to run; cmd.exe was only ever in the way, and
    // OpenCode takes its prompt as an argument, so it had the same truncation.
    const ROAMING = "C:\\Users\\x\\AppData\\Roaming";
    const cmdShim = `${ROAMING}\\npm\\opencode.cmd`;
    const binary = `${ROAMING}\\npm\\node_modules\\opencode-ai\\bin\\opencode.exe`;
    const shell = "C:\\Windows\\System32\\cmd.exe";
    const found = await createPathExecutableLocator({
      platform: "win32",
      environment: { PATH: `${ROAMING}\\npm`, APPDATA: ROAMING, SystemRoot: "C:\\Windows" },
      isExecutableFile: async (candidate) => candidate === cmdShim || candidate === binary || candidate === shell,
      readDirectory: async () => [],
      // Lifted from the real shim on 2026-09-05.
      readFile: async () => '"%dp0%\\node_modules\\opencode-ai\\bin\\opencode.exe"   %*\r\n',
    }).find("opencode");

    expect(found).toMatchObject({ discoveredPath: cmdShim, executablePath: binary, prefixArgs: [], kind: "native" });
  });

  it("keeps cmd.exe when the binary an npm shim names is not there", async () => {
    // A shim whose target is gone is a broken install; the shell will say so
    // the way it always did, rather than this locator inventing a path.
    const ROAMING = "C:\\Users\\x\\AppData\\Roaming";
    const cmdShim = `${ROAMING}\\npm\\opencode.cmd`;
    const shell = "C:\\Windows\\System32\\cmd.exe";
    const found = await createPathExecutableLocator({
      platform: "win32",
      environment: { PATH: `${ROAMING}\\npm`, APPDATA: ROAMING, SystemRoot: "C:\\Windows" },
      isExecutableFile: async (candidate) => candidate === cmdShim || candidate === shell,
      readDirectory: async () => [],
      readFile: async () => '"%dp0%\\node_modules\\opencode-ai\\bin\\opencode.exe"   %*',
    }).find("opencode");

    expect(found?.kind).toBe("cmd-shim");
  });

  it("keeps cmd.exe for a .cmd that is not an npm shim", async () => {
    // Somebody's own batch file. Guessing at what it does is how a locator
    // starts running things it did not mean to, so it keeps its shell.
    const ROAMING = "C:\\Users\\x\\AppData\\Roaming";
    const cmdShim = `${ROAMING}\\npm\\codex.cmd`;
    const shell = "C:\\Windows\\System32\\cmd.exe";
    const found = await createPathExecutableLocator({
      platform: "win32",
      environment: { PATH: `${ROAMING}\\npm`, APPDATA: ROAMING, SystemRoot: "C:\\Windows" },
      isExecutableFile: async (candidate) => candidate === cmdShim || candidate === shell,
      readDirectory: async () => [],
      readFile: async () => "@echo off\r\ncall some-other-thing.exe %*\r\n",
    }).find("codex");

    expect(found).toMatchObject({ executablePath: shell, prefixArgs: ["/d", "/s", "/c", cmdShim], kind: "cmd-shim" });
  });

  it("keeps cmd.exe for an npm shim when no node.exe can be found", async () => {
    // A machine this cannot help is no worse off than before.
    const ROAMING = "C:\\Users\\x\\AppData\\Roaming";
    const cmdShim = `${ROAMING}\\npm\\copilot.cmd`;
    const shell = "C:\\Windows\\System32\\cmd.exe";
    const found = await createPathExecutableLocator({
      platform: "win32",
      environment: { PATH: `${ROAMING}\\npm`, APPDATA: ROAMING, SystemRoot: "C:\\Windows" },
      isExecutableFile: async (candidate) => candidate === cmdShim || candidate === shell,
      readDirectory: async () => [],
      readFile: async () => '"%_prog%"  "%dp0%\\node_modules\\@github\\copilot\\npm-loader.js" %*',
    }).find("copilot");

    expect(found?.kind).toBe("cmd-shim");
  });

  it("finds Copilot CLI where it unpacks itself, when npm's bin directory is not there either", async () => {
    const LOCAL = "C:\\Users\\x\\AppData\\Local";
    const launcher = `${LOCAL}\\copilot\\copilot.exe`;
    const found = await createPathExecutableLocator({
      platform: "win32",
      environment: { PATH: "", LOCALAPPDATA: LOCAL, APPDATA: "C:\\Users\\x\\AppData\\Roaming", SystemRoot: "C:\\Windows" },
      isExecutableFile: async (candidate) => candidate === launcher,
      readDirectory: async () => [],
    }).find("copilot");
    expect(found?.kind).toBe("native");
    expect(found?.executablePath).toBe(launcher);
  });

  it("finds Cursor's launcher in its own install directory without PATH", async () => {
    // The installer writes `cursor-agent.cmd` and `cursor-agent.ps1` side by
    // side; the `.ps1` is the one the locator knows how to host safely.
    const LOCAL = "C:\\Users\\x\\AppData\\Local";
    const launcher = `${LOCAL}\\cursor-agent\\cursor-agent.ps1`;
    const powershell = "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe";
    const found = await createPathExecutableLocator({
      platform: "win32",
      environment: { PATH: "", LOCALAPPDATA: LOCAL, APPDATA: "C:\\Users\\x\\AppData\\Roaming", SystemRoot: "C:\\Windows" },
      isExecutableFile: async (candidate) => candidate === launcher || candidate === powershell,
      readDirectory: async () => [],
    }).find("cursor-agent");
    expect(found?.kind).toBe("powershell-shim");
    expect(found?.discoveredPath).toBe(launcher);
  });
});

describe("OpenCode and Copilot CLI commands", () => {
  const workspacePath = "C:\\work\\repo";
  const PROMPT = "Summarize the README in one sentence.";
  const openCode: ExecutableLaunch = { ...nativeExecutable, commandName: "opencode", discoveredPath: "C:\\tools\\opencode.exe", executablePath: "C:\\tools\\opencode.exe" };
  const copilot: ExecutableLaunch = { ...nativeExecutable, commandName: "copilot", discoveredPath: "C:\\tools\\copilot.exe", executablePath: "C:\\tools\\copilot.exe" };

  it("holds a read-only OpenCode mission with the permission config that actually enforces it", () => {
    const spec = createOpenCodeRunCommand(openCode, { workspacePath, prompt: PROMPT });
    expect(spec.runtime).toBe("opencode");
    // The prompt goes on stdin, like Codex and Claude, since 2026-09-17: as
    // a positional it hit the Windows command-line ceiling on the last hop
    // of the first Chief of Staff exchange and the person never got the
    // report. Measured: with no positional, `opencode run` reads stdin.
    expect(spec.stdin).toBe("prompt");
    // A new session is titled, so OpenCode does not spend a second model
    // call naming it (A6.3).
    expect(spec.args).toEqual(["run", "--format", "json", "--print-logs", "--log-level", "ERROR", "--title", "Locust"]);
    expect(spec.args).not.toContain(PROMPT);
    // There is no read-only FLAG. Measured, this environment value is the
    // only thing that stops a run editing files -- with it, the write tool is
    // not offered at all. And a read-only run loads no plugins, so a repo's
    // own .opencode/plugin code cannot run outside that config (A6.2).
    expect(spec.env).toEqual({ OPENCODE_CONFIG_CONTENT: withoutOpenCodeQuestionTool(OPENCODE_READ_ONLY_CONFIG), OPENCODE_PURE: "1" });
    expect(JSON.parse(OPENCODE_READ_ONLY_CONFIG)).toEqual({
      permission: {
        // Every tool the rules below do not name asks, and is refused (0.821): measured 2026-10-22, a `save_note`
        // tool from an MCP server in the person's own global config ran without asking and wrote note.txt.
        "*": "ask",
        // The reading tools are allowed after it, by name. A `.env` file is not read.
        read: { "*": "allow", "*.env": "deny", "*.env.*": "deny", "*.env.example": "allow" },
        glob: "allow",
        grep: "allow",
        list: "allow",
        lsp: "allow",
        task: "allow",
        todowrite: "allow",
        todoread: "allow",
        skill: "allow",
        webfetch: "allow",
        websearch: "allow",
        codesearch: "allow",
        edit: "deny",
        write: "deny",
        // "ask", not "deny". Bisected on the free model 2026-09-18: a run
        // that offers no bash tool is refused by the provider with "free
        // tier can only be used from within OpenCode" (Grok, passes 9 and
        // 10: four red cards from Ask mode). With "ask" the tool is listed,
        // and `opencode run`, being non-interactive, auto-rejects every
        // use of it -- measured: "permission requested: bash; auto-rejecting".
        // Still read-only; refused at the call rather than by absence.
        // EVERY shell command, git included (0.818): OpenCode matches a
        // command with its redirects taken off, so 0.744's `git status*`
        // allow let `git status > status.txt` write the file, and `git log -1
        // > ../outside.txt` write one outside the folder (measured
        // 2026-10-22). Git reaches the run through Locust's git tool instead.
        bash: "ask",
        patch: "deny",
        // Stated, not left to the default: an unstated refusal ends the run.
        external_directory: "deny",
      },
      // A refused call is answered to the model, not the end of the run
      // (A6.1): measured, `git status` refused, the file read instead, exit 0.
      experimental: { continue_loop_on_deny: true },
    });
    // The catch-all comes FIRST: OpenCode's last matching rule wins, so after the allows it would refuse reading too.
    expect(Object.keys(JSON.parse(OPENCODE_READ_ONLY_CONFIG).permission)[0]).toBe("*");
  });

  it("gives a run with no shell Locust's git tool, and only such a run", () => {
    const gitReads = { command: "C:\\Locust\\Locust.exe", args: ["C:\\Locust\\resources\\locust-git-reads.mjs"], env: { ELECTRON_RUN_AS_NODE: "1" }, instructions: "C:\\Users\\a\\AppData\\Roaming\\Locust\\opencode-git-tool.md" };
    const config = (sandbox?: "read-only" | "workspace-write" | "full-access", repositoryRoot?: string) => JSON.parse(
      createOpenCodeRunCommand(openCode, { workspacePath, prompt: PROMPT, gitReads, ...(sandbox === undefined ? {} : { sandbox }), ...(repositoryRoot === undefined ? {} : { repositoryRoot }) }).env?.OPENCODE_CONFIG_CONTENT ?? "{}",
    ) as { mcp?: Record<string, unknown>; permission: Record<string, unknown>; instructions?: unknown };
    // Ask (the default) and Plan: the server, in the run's own folder, allowed, and told of in OpenCode's instructions.
    const ask = config();
    expect(ask.mcp?.locust_git).toEqual({
      type: "local",
      command: ["C:\\Locust\\Locust.exe", "C:\\Locust\\resources\\locust-git-reads.mjs"],
      environment: { ELECTRON_RUN_AS_NODE: "1", LOCUST_GIT_FOLDER: workspacePath },
      enabled: true,
    });
    expect(ask.permission["locust_git_*"]).toBe("allow");
    expect(ask.instructions).toEqual([gitReads.instructions]);
    // The shell stays shut beside it.
    expect(ask.permission.bash).toBe("ask");
    // A worktree's read-only run gets it too, in the worktree.
    expect(config("read-only", "C:\\work\\shop").mcp?.locust_git).toMatchObject({ environment: { LOCUST_GIT_FOLDER: workspacePath } });
    // A run that may edit, and Auto, have a shell that runs git: no second way in.
    for (const sandbox of ["workspace-write", "full-access"] as const) {
      const other = config(sandbox);
      expect(other.mcp?.locust_git, sandbox).toBeUndefined();
      expect(other.permission["locust_git_*"], sandbox).toBeUndefined();
      expect(other.instructions, sandbox).toBeUndefined();
    }
    // Without a spec the config is as it was.
    expect(withOpenCodeGitReads(OPENCODE_READ_ONLY_CONFIG, undefined, workspacePath)).toBe(OPENCODE_READ_ONLY_CONFIG);
    // The person's and the project's instructions are OpenCode's to add; a config's own list is kept and added to.
    expect(JSON.parse(withOpenCodeGitReads(JSON.stringify({ permission: {}, instructions: ["NOTES.md"] }), gitReads, workspacePath)).instructions).toEqual(["NOTES.md", gitReads.instructions]);
  });

  it("titles only a NEW OpenCode session, and keeps plugins for a run that may edit", () => {
    const resumed = createOpenCodeRunCommand(openCode, { workspacePath, prompt: PROMPT, resumeThreadId: "ses_abc" });
    expect(resumed.args).toEqual(["run", "--format", "json", "--print-logs", "--log-level", "ERROR", "-s", "ses_abc"]);
    const editing = createOpenCodeRunCommand(openCode, { workspacePath, prompt: PROMPT, sandbox: "workspace-write" });
    expect(editing.env?.OPENCODE_PURE).toBeUndefined();
  });

  it("lets a worktree run reach the repository it belongs to, and only that", () => {
    // A worktree's `.git` is a file pointing at the parent repository, so a
    // run inside one reaches out of its own folder by necessity. OpenCode
    // auto-rejects a directory outside its own AND the rejection ends the
    // run: three teammates on worktrees lost between one and three of their
    // runs to it (drive, 2026-09-06), reproduced from a shell with
    // `permission requested: external_directory (<parent>\*); auto-rejecting`.
    const worktree = createOpenCodeRunCommand(openCode, {
      workspacePath: "C:\\work\\shop\\.locust\\worktrees\\tm_wren",
      sandbox: "workspace-write",
      prompt: PROMPT,
      repositoryRoot: "C:\\work\\shop",
    });
    expect(JSON.parse(worktree.env?.OPENCODE_CONFIG_CONTENT ?? "{}")).toEqual({
        tools: { question: false },
        // `.git` and nothing else. Granting the whole parent folder DID let all
      // three teammates finish -- and one of them then wrote its file into the
      // shared folder as well as its own worktree, which is exactly what a
      // worktree exists to prevent. A worktree needs the parent's `.git`,
      // because its own is a file pointing there; it does not need the
      // parent's working tree. Keys and values read off OpenCode's own config
      // schema, not guessed.
      permission: {
        external_directory: {
          "C:\\work\\shop\\.git\\*": "allow",
          // Everything else refused BY NAME. Without the catch-all the rest
          // falls to OpenCode's default, which does not refuse -- it ends the
          // run.
          "*": "deny",
        },
      },
    });

    // A read-only worktree run still needs the parent AND still may not write.
    // These used to be written into the same environment variable, where the
    // second would simply have replaced the first.
    const readOnly = createOpenCodeRunCommand(openCode, {
      workspacePath: "C:\\work\\shop\\.locust\\worktrees\\tm_wren",
      sandbox: "read-only",
      prompt: PROMPT,
      repositoryRoot: "C:\\work\\shop",
    });
    expect(JSON.parse(readOnly.env?.OPENCODE_CONFIG_CONTENT ?? "{}")).toEqual({
      tools: { question: false },
      permission: {
        // The same catch-all and reading tools as the plain read-only run (0.821); the drift guard below compares them.
        ...Object.fromEntries(Object.entries(JSON.parse(OPENCODE_READ_ONLY_CONFIG).permission as Record<string, unknown>).filter(([rule]) => rule !== "external_directory")),
        edit: "deny",
        write: "deny",
        /*
         * "ask", and THIS LINE ASSERTED "deny" UNTIL 2026-09-22.
         *
         * The bisect recorded in the sibling test above -- a run offering no
         * bash tool is refused by the free provider with "OpenCode's free
         * tier can only be used from within OpenCode" -- was applied to
         * `OPENCODE_READ_ONLY_CONFIG` and to its test, and not to the
         * worktree builder or to this one. So the same defect survived in
         * the path that combines three of the app's defaults: a teammate on
         * its own branch, in Ask or Plan mode, on the free model the first
         * screen recommends.
         *
         * REPRODUCED 2026-09-22 against opencode-ai 1.18.27 by sending both
         * configs to the real provider: "deny" returns HTTP 403
         * `FreeTierError`, "ask" answers. A test that agrees with the code
         * and disagrees with the provider is worse than no test.
         *
         * 0.818: "ask" again, as the sibling's -- every shell command refused.
         */
        bash: JSON.parse(OPENCODE_READ_ONLY_CONFIG).permission.bash,
        patch: "deny",
        external_directory: { "C:\\work\\shop\\.git\\*": "allow", "*": "deny" },
      },
      // Read-only, so a refused call does not end it either (A6.1).
      experimental: { continue_loop_on_deny: true },
    });

    /*
     * The drift guard, which is the actual repair.
     *
     * Two builders write the same read-only denials, and a correction landed
     * on one of them. Whatever those denials become, both must carry them --
     * so this compares the two rather than restating either, and a future
     * change to one alone fails here instead of at a person's first mission.
     */
    const plain = JSON.parse(
      createOpenCodeRunCommand(openCode, { workspacePath, sandbox: "read-only", prompt: PROMPT })
        .env?.OPENCODE_CONFIG_CONTENT ?? "{}",
    ) as { permission: Record<string, unknown> };
    const inWorktree = JSON.parse(readOnly.env?.OPENCODE_CONFIG_CONTENT ?? "{}") as {
      permission: Record<string, unknown>;
    };
    for (const rule of Object.keys(plain.permission).filter((rule) => rule !== "external_directory")) {
      expect(inWorktree.permission[rule], `${rule} differs between a plain read-only run and a worktree one`)
        .toEqual(plain.permission[rule]);
    }
    expect(["edit", "write", "bash", "patch", "*"].every((rule) => rule in plain.permission)).toBe(true);
    expect(Object.keys(inWorktree.permission)[0], "the worktree run's catch-all comes first too").toBe("*");

    // And a run in the folder itself asks for nothing extra -- the negative
    // control, without which every assertion above passes on a builder that
    // simply always allows.
    const inPlace = createOpenCodeRunCommand(openCode, {
      workspacePath: "C:\\work\\shop",
      sandbox: "workspace-write",
      prompt: PROMPT,
    });
    // It carries the plain confinement every run now carries, and -- the part
    // that matters -- NO allow entry of any kind. Asserting "no config at all"
    // used to serve this purpose and no longer can, but the control's job is
    // unchanged: prove the parent grant is not handed out unconditionally.
    expect(JSON.parse(inPlace.env?.OPENCODE_CONFIG_CONTENT ?? "{}")).toEqual({
      tools: { question: false },
      permission: { external_directory: "deny" },
    });
    expect(inPlace.env?.OPENCODE_CONFIG_CONTENT).not.toContain("allow");
  });

  it("lets a workspace-write OpenCode mission edit, and states its confinement rather than assuming it", () => {
    const spec = createOpenCodeRunCommand(openCode, {
      workspacePath,
      sandbox: "workspace-write",
      model: "opencode/big-pickle",
      resumeThreadId: "ses_1",
      prompt: PROMPT,
    });
    // This used to carry no config at all, and that is what a person's run
    // died of: with nothing stated, a look outside the folder is met by
    // OpenCode's default, which prints "permission requested:
    // external_directory (...); auto-rejecting" and ENDS the process without
    // a step saying it stopped. Colin hit it on 2026-09-06 asking a teammate
    // to message its teammates; the model went looking around the filesystem
    // and the run died with a red card and no answer. Measured over three
    // runs of one prompt: unstated -> run never reached its own end; "deny"
    // -> refused cleanly, the model said so, the run finished.
    expect(JSON.parse(spec.env?.OPENCODE_CONFIG_CONTENT ?? "{}")).toEqual({
      tools: { question: false },
      permission: { external_directory: "deny" },
    });
    // Confinement is stated; nothing about editing inside the folder changed.
    expect(spec.args).toEqual(["run", "--format", "json", "--print-logs", "--log-level", "ERROR", "-m", "opencode/big-pickle", "-s", "ses_1"]);
    // `--auto` buys nothing: measured, `run` edits files without it.
    expect(spec.args).not.toContain("--auto");
    // Plan mode is narration, not enforcement, so it is never the read-only
    // mechanism here.
    expect(spec.args).not.toContain("--agent");
  });

  it("holds a read-only Copilot mission with the tool denylist that was measured refusing writes", () => {
    const spec = createCopilotPromptCommand(copilot, { workspacePath, prompt: PROMPT, sessionId: "uuid-1" });
    expect(spec.runtime).toBe("copilot");
    expect(spec.stdin).toBe("none");
    expect(spec.args).toEqual([
      "-p", PROMPT, "--output-format", "json", "--allow-all-tools", "--no-color",
      "--excluded-tools=session_store_sql", "--deny-tool=write,shell", "--session-id", "uuid-1",
    ]);
  });

  it("never offers a Copilot teammate the machine's other Copilot conversations, on either route (0.378)", () => {
    // `session_store_sql` reads every Copilot session on the machine, and runs
    // without asking: measured answering a cold follow-up from an unrelated one.
    for (const spec of [
      createCopilotPromptCommand(copilot, { workspacePath, prompt: PROMPT, sessionId: "uuid-1" }),
      createCopilotPromptCommand(copilot, { workspacePath, prompt: PROMPT, sandbox: "full-access", resumeThreadId: "uuid-1" }),
      createCopilotAcpCommand(copilot, { workspacePath }),
    ]) {
      expect(spec.args).toContain("--excluded-tools=session_store_sql");
    }
  });

  it("lets a workspace-write Copilot mission edit, and resumes by the id the host minted", () => {
    const spec = createCopilotPromptCommand(copilot, {
      workspacePath,
      sandbox: "workspace-write",
      model: "gpt-5.6-luna",
      resumeThreadId: "uuid-1",
      prompt: PROMPT,
    });
    expect(spec.args).not.toContain("--deny-tool=write,shell");
    // Measured: the resume flag takes its value with `=`.
    expect(spec.args.slice(-3)).toEqual(["--model", "gpt-5.6-luna", "--resume=uuid-1"]);
    expect(spec.args).not.toContain("--session-id");
  });

  it("passes an OpenCode effort as its variant, and only an effort word", () => {
    // This refused every effort until A6.5 measured it (2026-09-25): each
    // model lists its variants in `models --verbose`, and `--variant high`
    // reasoned more than `--variant low` in four runs of four on the free Ling.
    const spec = createOpenCodeRunCommand(openCode, { workspacePath, prompt: PROMPT, model: "opencode/ling-3.0-flash-fin-free", effort: "high" });
    expect(spec.args).toEqual(["run", "--format", "json", "--print-logs", "--log-level", "ERROR", "-m", "opencode/ling-3.0-flash-fin-free", "--variant", "high", "--title", "Locust"]);
    // OpenCode accepts ANY name and ignores one it does not know, so a word
    // that is not an effort is refused here rather than shown as applied.
    expect(() => createOpenCodeRunCommand(openCode, { workspacePath, prompt: PROMPT, effort: "bogus" })).toThrow(/effort level called "bogus"/);
  });

  it("reads each OpenCode model's variants as its efforts, in the scale's order", () => {
    // The verbose listing's own shape: an id line, then the model as JSON.
    const verbose = [
      "opencode/big-pickle",
      "{",
      '  "id": "big-pickle",',
      '  "variants": {}',
      "}",
      "opencode/space-bunny-free",
      "{",
      '  "id": "space-bunny-free",',
      '  "variants": {',
      '    "max": { "reasoningEffort": "max" },',
      '    "low": { "reasoningEffort": "low" },',
      '    "turbo": { "speed": "fast" }',
      "  }",
      "}",
    ].join("\n");
    expect(parseOpenCodeModelList(verbose)?.models).toEqual([
      { id: "opencode/big-pickle", displayName: "big-pickle" },
      // A variant that is not an effort word is a provider's option, not offered.
      { id: "opencode/space-bunny-free", displayName: "space-bunny-free", description: "free", efforts: ["low", "max"] },
    ]);
    // The plain listing still reads, with no efforts, as before.
    expect(parseOpenCodeModelList("opencode/big-pickle\nopencode/space-bunny-free\n")?.models).toEqual([
      { id: "opencode/big-pickle", displayName: "big-pickle" },
      { id: "opencode/space-bunny-free", displayName: "space-bunny-free", description: "free" },
    ]);
  });

  it("PASSES an effort to Copilot, which does take one", () => {
    // This used to be paired with OpenCode above as a runtime with "no effort
    // flag". Measured 2026-09-08 against copilot 1.0.83, that was untrue:
    // `--effort, --reasoning-effort <level>` with seven choices. Locust
    // refused a level the CLI accepts, and read none of its choices either.
    const spec = createCopilotPromptCommand(copilot, { workspacePath, prompt: PROMPT, effort: "high" });
    expect(spec.args).toContain("--effort");
    expect(spec.args[spec.args.indexOf("--effort") + 1]).toBe("high");
  });

  it("refuses to build a command with no prompt, because the prompt IS the argv here", () => {
    // For these two there is no stdin fallback: an empty positional would
    // launch a CLI that sits there with nothing to do.
    expect(() => createOpenCodeRunCommand(openCode, { workspacePath })).toThrow(/Prompt/);
    expect(() => createCopilotPromptCommand(copilot, { workspacePath })).toThrow(/Prompt/);
  });

  it("reads OpenCode's model list as it prints it, and marks the free models free", () => {
    // The real list, as printed 2026-09-03: one `provider/model` per line.
    const hints = parseOpenCodeModelList(
      "opencode/muse-spark-1.3-contributor-free\nopencode/nemotron-3.5-lightning-free\nopencode/big-pickle\n",
    );
    expect(hints?.models).toEqual([
      { id: "opencode/muse-spark-1.3-contributor-free", displayName: "muse-spark-1.3-contributor-free", description: "free" },
      { id: "opencode/nemotron-3.5-lightning-free", displayName: "nemotron-3.5-lightning-free", description: "free" },
      { id: "opencode/big-pickle", displayName: "big-pickle" },
    ]);
    expect(hints?.efforts).toEqual([]);
  });

  it("reads no models from output that is not a model list", () => {
    expect(parseOpenCodeModelList("")).toBeUndefined();
    expect(parseOpenCodeModelList("Error: something went wrong")).toBeUndefined();
    // A duplicated id is listed once; a line with no slash is not a model.
    expect(parseOpenCodeModelList("a/b\na/b\njust words")?.models).toEqual([{ id: "a/b", displayName: "b" }]);
  });
});

describe("OpenCode and Copilot CLI discovery", () => {
  const OPENCODE_HELP = "opencode run [message..]  --format <format>  text | json  -m, --model  -s, --session";
  const COPILOT_HELP = "-p, --prompt <prompt>  --output-format <format> text | json  --allow-all-tools  --deny-tool <tools>  --session-id  --resume";
  const OPENCODE_MODELS = "opencode/muse-spark-1.3-contributor-free\nopencode/big-pickle\n";

  function newcomerLocator(name: string): ExecutableLocator {
    return {
      find: async (command) =>
        command === name
          ? { ...nativeExecutable, commandName: name, discoveredPath: `C:\\tools\\${name}.exe`, executablePath: `C:\\tools\\${name}.exe` }
          : undefined,
    };
  }

  it("reports OpenCode ready with no sign-in at all, and reads the models it ships", async () => {
    // MEASURED: OpenCode has no auth command and needs none -- it ships free
    // models. Listing them is the cheapest true statement available, so it is
    // both the readiness check and the model probe.
    const runner: CommandRunner = {
      run: async (command) => {
        if (command.purpose === "version") return { exitCode: 0, stdout: "1.18.27", stderr: "" };
        if (command.purpose === "capabilities") {
          expect(command.args).toEqual(["run", "--help"]);
          return { exitCode: 0, stdout: OPENCODE_HELP, stderr: "" };
        }
        expect([["models"], ["models", "--verbose"]]).toContainEqual(command.args);
        return { exitCode: 0, stdout: OPENCODE_MODELS, stderr: "" };
      },
    };
    const [opencode] = (await discoverInstalledRuntimes({ runner, locator: newcomerLocator("opencode") }))
      .filter((entry) => entry.id === "opencode");
    expect(opencode?.availability).toBe("available");
    expect(opencode?.readiness).toBe("ready");
    expect(opencode?.version?.version).toBe("1.18.27");
    expect(opencode?.supportedFeatures).toEqual(expect.arrayContaining([...OPENCODE_REQUIRED_FEATURES]));
    expect(opencode?.modelHints?.models?.map((model) => model.id))
      .toEqual(["opencode/muse-spark-1.3-contributor-free", "opencode/big-pickle"]);
  });

  it("reads each model's efforts from the verbose list, and keeps the plain list when an older CLI refuses it", async () => {
    const verbose = 'opencode/ling-3.0-flash-fin-free\n{\n  "variants": { "low": {}, "high": {} }\n}\n';
    const discover = async (verboseAnswer: { exitCode: number; stdout: string; stderr: string }) => {
      const runner: CommandRunner = {
        run: async (command) => {
          if (command.purpose === "version") return { exitCode: 0, stdout: "1.18.27", stderr: "" };
          if (command.purpose === "capabilities") return { exitCode: 0, stdout: OPENCODE_HELP, stderr: "" };
          if (command.args.includes("--verbose")) return verboseAnswer;
          return { exitCode: 0, stdout: "opencode/ling-3.0-flash-fin-free\n", stderr: "" };
        },
      };
      const [opencode] = (await discoverInstalledRuntimes({ runner, locator: newcomerLocator("opencode") }))
        .filter((entry) => entry.id === "opencode");
      return opencode;
    };
    expect((await discover({ exitCode: 0, stdout: verbose, stderr: "" }))?.modelHints?.models)
      .toEqual([{ id: "opencode/ling-3.0-flash-fin-free", displayName: "ling-3.0-flash-fin-free", description: "free", efforts: ["low", "high"] }]);
    const older = await discover({ exitCode: 1, stdout: "", stderr: "Unknown argument: verbose" });
    expect(older?.modelHints?.models).toEqual([{ id: "opencode/ling-3.0-flash-fin-free", displayName: "ling-3.0-flash-fin-free", description: "free" }]);
    expect(older?.diagnostics.some((issue) => issue.code === "capability-probe-failed")).toBe(false);
  });

  it("still says signed out when a CLI says so in its own words", async () => {
    const runner: CommandRunner = {
      run: async (command) => {
        if (command.purpose === "capabilities") return { exitCode: 0, stdout: COPILOT_HELP, stderr: "" };
        return { exitCode: 1, stdout: "", stderr: "Error: not logged in. Run copilot login." };
      },
    };
    const [copilot] = (await discoverInstalledRuntimes({ runner, locator: newcomerLocator("copilot") }))
      .filter((entry) => entry.id === "copilot");
    expect(copilot?.readiness).toBe("authentication-required");
  });

  it("does not report an OpenCode that cannot list a single model as ready to run one", async () => {
    const runner: CommandRunner = {
      run: async (command) => {
        if (command.purpose === "version") return { exitCode: 0, stdout: "1.18.27", stderr: "" };
        if (command.purpose === "capabilities") return { exitCode: 0, stdout: OPENCODE_HELP, stderr: "" };
        return { exitCode: 0, stdout: "Error: could not reach the model registry", stderr: "" };
      },
    };
    const [opencode] = (await discoverInstalledRuntimes({ runner, locator: newcomerLocator("opencode") }))
      .filter((entry) => entry.id === "opencode");
    // Not ready -- and not "signed out" either: OpenCode has no sign-in, and
    // this asserted "authentication-required" until a fresh-profile beta
    // report of 0.345 saw that word stick while the window never re-checked.
    // Not answering is checked again.
    expect(opencode?.readiness).toBe("unhealthy");
  });

  it("reports Copilot CLI ready on its version alone, and says on the record why that is a guess", async () => {
    // Copilot has NO free readiness check: whether the account's plan
    // includes the CLI is only learned by starting a run, and a run costs a
    // premium request. Discovery will not spend the user's quota to fill in a
    // status field, so the caveat rides along with the ready verdict.
    const runner: CommandRunner = {
      run: async (command) => {
        if (command.purpose === "capabilities") return { exitCode: 0, stdout: COPILOT_HELP, stderr: "" };
        expect(command.args).toEqual(["--version"]);
        // Note the trailing period, which is how the CLI prints it.
        return { exitCode: 0, stdout: "GitHub Copilot CLI 1.0.82.", stderr: "" };
      },
    };
    const [copilot] = (await discoverInstalledRuntimes({ runner, locator: newcomerLocator("copilot") }))
      .filter((entry) => entry.id === "copilot");
    expect(copilot?.readiness).toBe("ready");
    expect(copilot?.version?.version).toBe("1.0.82");
    expect(copilot?.supportedFeatures).toEqual(expect.arrayContaining([...COPILOT_REQUIRED_FEATURES]));
    const caveat = copilot?.diagnostics.find((issue) => issue.code === "readiness-unverifiable");
    expect(caveat?.severity).toBe("info");
    expect(caveat?.resolution).toContain("https://github.com/settings/copilot");
  });

  it("offers Copilot only the route where the CLI picks, because it names no models before a run", async () => {
    // The only place model names appear is inside a paid run. Naming one here
    // would be inventing it.
    const runner: CommandRunner = {
      run: async (command) => ({
        exitCode: 0,
        stdout: command.purpose === "capabilities" ? COPILOT_HELP : "GitHub Copilot CLI 1.0.82.",
        stderr: "",
      }),
    };
    const [copilot] = (await discoverInstalledRuntimes({ runner, locator: newcomerLocator("copilot") }))
      .filter((entry) => entry.id === "copilot");
    expect(copilot?.modelHints?.models).toEqual([
      { id: "auto", displayName: "Auto", description: "Copilot picks the model" },
    ]);
  });

  it("reports a Copilot whose version command says something else as not ready", async () => {
    const runner: CommandRunner = {
      run: async (command) => ({
        exitCode: 0,
        stdout: command.purpose === "capabilities" ? COPILOT_HELP : "command not recognized",
        stderr: "",
      }),
    };
    const [copilot] = (await discoverInstalledRuntimes({ runner, locator: newcomerLocator("copilot") }))
      .filter((entry) => entry.id === "copilot");
    // A version command saying something else is not a sign-in statement (the
    // same 0.345 correction): not ready, and checked again.
    expect(copilot?.readiness).toBe("unhealthy");
    expect(copilot?.modelHints).toBeUndefined();
  });
});

function fakeProbe(command: ProbeCommand) {
  const args = command.args.join(" ");
  const isClaude = command.executablePath.includes("claude");
  if (command.purpose === "version") {
    return Promise.resolve({
      exitCode: 0,
      stdout: isClaude ? "2.1.251 (Claude Code)" : "codex-cli 0.151.0-alpha.7.2",
      stderr: "",
    });
  }
  if (command.purpose === "capabilities") {
    return Promise.resolve({
      exitCode: 0,
      stdout: isClaude
        ? "--print stdin --output-format stream-json --restricted --verbose --include-partial-messages --permission-mode default --tools --disallowedTools"
        : "Run non-interactively stdin --json --cd --sandbox read-only",
      stderr: "",
    });
  }
  return Promise.resolve({ exitCode: args.includes("login status") ? 1 : 0, stdout: "", stderr: "" });
}

describe("bounded Node probe runner", () => {
  it("caps captured output and never asks Node for a shell", async () => {
    const stdout = new EventEmitter();
    const stderr = new EventEmitter();
    let spawnOptions: unknown;
    const runner = createNodeProbeRunner({
      maxOutputBytes: 4,
      spawnProcess: (_executable, _args, options) => {
        spawnOptions = options;
        const process = new EventEmitter() as EventEmitter & SpawnedProbeProcess;
        Object.assign(process, {
          stdout,
          stderr,
          kill: () => true,
        });
        queueMicrotask(() => {
          stdout.emit("data", "abcdefgh");
          stderr.emit("data", "12345678");
          process.emit("close", 0);
        });
        return process;
      },
    });

    const result = await runner.run({
      purpose: "version",
      executablePath: "C:\\tools\\codex.exe",
      args: ["--version"],
      timeoutMs: 100,
    });
    expect(result.stdout).toBe("abcd");
    expect(result.stderr).toBe("1234");
    expect(spawnOptions).toMatchObject({ shell: false, windowsHide: true });
  });

  it("terminates a timed-out probe", async () => {
    const stdout = new EventEmitter();
    const stderr = new EventEmitter();
    const signals: Array<NodeJS.Signals | undefined> = [];
    const runner = createNodeProbeRunner({
      maximumTimeoutMs: 5,
      killGraceMs: 5,
      spawnProcess: () => {
        const process = new EventEmitter() as EventEmitter & SpawnedProbeProcess;
        Object.assign(process, {
          stdout,
          stderr,
          kill: (signal?: NodeJS.Signals) => {
            signals.push(signal);
            return true;
          },
        });
        return process;
      },
    });

    const result = await runner.run({
      purpose: "readiness",
      executablePath: "C:\\tools\\codex.exe",
      args: ["login", "status"],
      timeoutMs: 5,
    });
    expect(result.timedOut).toBe(true);
    expect(signals).toEqual(["SIGTERM", "SIGKILL"]);
  });

  it("kills every probe still in flight when the app is leaving, tree first", async () => {
    /*
     * Fable, pass 1: close Locust while a discovery sweep is out and ten
     * processes stay behind on Linux -- five shims and their five sleeps,
     * each in its own session. The timer was the only caller of the kill,
     * and there is no timer once the app has gone. `dispose` is what
     * `before-quit` calls.
     */
    const treeKills: number[] = [];
    const signals: Array<NodeJS.Signals | undefined> = [];
    let nextPid = 100;
    const runner = createNodeProbeRunner({
      maximumTimeoutMs: 10_000,
      killTree: (pid) => treeKills.push(pid),
      spawnProcess: () => {
        const process = new EventEmitter() as EventEmitter & SpawnedProbeProcess;
        Object.assign(process, {
          stdout: new EventEmitter(),
          stderr: new EventEmitter(),
          pid: nextPid++,
          kill: (signal?: NodeJS.Signals) => {
            signals.push(signal);
            return true;
          },
        });
        return process;
      },
    });
    const command = { purpose: "readiness", executablePath: "C:\\tools\\hung.cmd", args: ["--version"], timeoutMs: 10_000 } as const;
    // Two out, never answering; a third that closed on its own first.
    void runner.run(command);
    void runner.run(command);
    const closed = runner.run(command);
    const third = [...signals];
    expect(third).toEqual([]);
    runner.dispose();
    expect(treeKills).toEqual([100, 101, 102]);
    expect(signals).toEqual(["SIGKILL", "SIGKILL", "SIGKILL"]);
    // Disposing again finds nothing: the set was cleared.
    runner.dispose();
    expect(treeKills).toHaveLength(3);
    void closed;
  });

  it("spawns nothing once it has been disposed", async () => {
    /*
     * Fable, pass 2, by reading: dispose() killed what was live and cleared
     * the set, and run() would spawn again afterwards -- so a probe started
     * AFTER the kill was reachable by nothing, because dispose is called once
     * from before-quit.
     *
     * The window is the bounded shutdown: the ledger flushes for up to six
     * seconds with the renderer alive and its re-check chains still firing.
     * With five hung CLIs a sweep landing in there is ten processes outliving
     * an ORDINARY quit, by the same route as the SIGKILL case that cannot be
     * fixed.
     */
    let spawns = 0;
    const runner = createNodeProbeRunner({
      spawnProcess: () => {
        spawns += 1;
        const process = new EventEmitter() as EventEmitter & SpawnedProbeProcess;
        Object.assign(process, { stdout: new EventEmitter(), stderr: new EventEmitter(), pid: 1, kill: () => true });
        return process;
      },
    });
    const command = { purpose: "readiness", executablePath: "C:\tools\hung.cmd", args: ["--version"], timeoutMs: 1_000 } as const;
    runner.dispose();
    // Refused rather than ignored: a caller gets an error it can see, not a
    // promise that never settles.
    await expect(runner.run(command)).rejects.toThrow(/disposed/i);
    expect(spawns).toBe(0);
  });

  it("does not reach a probe that already closed", async () => {
    const treeKills: number[] = [];
    let spawned: (EventEmitter & SpawnedProbeProcess) | undefined;
    const runner = createNodeProbeRunner({
      killTree: (pid) => treeKills.push(pid),
      spawnProcess: () => {
        const process = new EventEmitter() as EventEmitter & SpawnedProbeProcess;
        Object.assign(process, { stdout: new EventEmitter(), stderr: new EventEmitter(), pid: 7, kill: () => true });
        spawned = process;
        return process;
      },
    });
    const done = runner.run({ purpose: "readiness", executablePath: "C:\\tools\\quick.cmd", args: ["--version"], timeoutMs: 1_000 });
    spawned?.emit("close", 0);
    await done;
    runner.dispose();
    expect(treeKills).toEqual([]);
  });

  it("takes a timed-out probe's whole tree down, not only the shim it spawned", async () => {
    /*
     * MEASURED 2026-09-18 with five `.cmd` shims that ping for an hour: the
     * signals above end cmd.exe and the ping it started runs on. Five leaked
     * per sweep; fifty were running after two drives. The tree goes first,
     * by the pid the child reports, and a child that reports none gets the
     * signals alone.
     */
    const treeKills: number[] = [];
    const signals: Array<NodeJS.Signals | undefined> = [];
    const runner = createNodeProbeRunner({
      maximumTimeoutMs: 5,
      killGraceMs: 5,
      killTree: (pid) => treeKills.push(pid),
      spawnProcess: () => {
        const process = new EventEmitter() as EventEmitter & SpawnedProbeProcess;
        Object.assign(process, {
          stdout: new EventEmitter(),
          stderr: new EventEmitter(),
          pid: 4242,
          kill: (signal?: NodeJS.Signals) => {
            signals.push(signal);
            return true;
          },
        });
        return process;
      },
    });

    const result = await runner.run({
      purpose: "readiness",
      executablePath: "C:\\tools\\opencode.cmd",
      args: ["--version"],
      timeoutMs: 5,
    });
    expect(result.timedOut).toBe(true);
    expect(treeKills).toEqual([4242]);
    // The tree first, then the signals: the order in which they end things.
    expect(signals).toEqual(["SIGTERM", "SIGKILL"]);
  });

  it("off Windows, spawns the probe as its own process group and signals the group on timeout", async () => {
    /*
     * Grok's pass 12, Linux: 41 sleeping shims after Locust exited. The tree
     * kill shipped in 0.182.0 was taskkill, a Windows program, and returned
     * early everywhere else. A probe spawned detached is its own group, and
     * kill(-pid) reaches the shim and whatever it started.
     */
    let spawnOptions: { detached?: boolean } | undefined;
    const killed: Array<[number, NodeJS.Signals | undefined]> = [];
    const realKill = process.kill;
    process.kill = ((pid: number, signal?: NodeJS.Signals) => {
      killed.push([pid, signal]);
      return true;
    }) as typeof process.kill;
    try {
      const runner = createNodeProbeRunner({
        maximumTimeoutMs: 5,
        killGraceMs: 5,
        platform: "linux",
        spawnProcess: (_executable, _args, options) => {
          spawnOptions = options;
          const child = new EventEmitter() as EventEmitter & SpawnedProbeProcess;
          Object.assign(child, { stdout: new EventEmitter(), stderr: new EventEmitter(), pid: 777, kill: () => true });
          return child;
        },
      });
      const result = await runner.run({ purpose: "readiness", executablePath: "/usr/local/bin/opencode", args: ["--version"], timeoutMs: 5 });
      expect(result.timedOut).toBe(true);
    } finally {
      process.kill = realKill;
    }
    expect(spawnOptions?.detached).toBe(true);
    expect(killed).toEqual([[-777, "SIGKILL"]]);
  });

  it("on Windows, does not detach: a detached child there gets its own console", async () => {
    let spawnOptions: { detached?: boolean } | undefined;
    const runner = createNodeProbeRunner({
      maximumTimeoutMs: 5,
      killGraceMs: 5,
      platform: "win32",
      killTree: () => undefined,
      spawnProcess: (_executable, _args, options) => {
        spawnOptions = options;
        const child = new EventEmitter() as EventEmitter & SpawnedProbeProcess;
        Object.assign(child, { stdout: new EventEmitter(), stderr: new EventEmitter(), pid: 1, kill: () => true });
        return child;
      },
    });
    await runner.run({ purpose: "readiness", executablePath: "C:\\tools\\opencode.cmd", args: ["--version"], timeoutMs: 5 });
    expect(spawnOptions?.detached).toBeUndefined();
  });
});

describe("model and effort on the command line", () => {
  const executable = {
    commandName: "x",
    discoveredPath: process.platform === "win32" ? "C:\\tools\\x.exe" : "/tools/x",
    executablePath: process.platform === "win32" ? "C:\\tools\\x.exe" : "/tools/x",
    prefixArgs: [],
    kind: "native" as const,
  };
  const workspacePath = process.platform === "win32" ? "C:\\work" : "/work";

  it("passes the chosen model and effort to Claude Code", () => {
    const spec = createClaudePrintCommand(executable, { workspacePath, model: "fable", effort: "high" });
    expect(spec.args).toContain("--model");
    expect(spec.args[spec.args.indexOf("--model") + 1]).toBe("fable");
    expect(spec.args[spec.args.indexOf("--effort") + 1]).toBe("high");
  });

  it("passes effort to codex exec through the config override its docs describe", () => {
    const spec = createCodexExecCommand(executable, { workspacePath, model: "gpt-5.6-sol", effort: "xhigh" });
    expect(spec.args[spec.args.indexOf("-c") + 1]).toBe("model_reasoning_effort=xhigh");
  });

  it("refuses an effort that is not one plain word", () => {
    expect(() => createClaudePrintCommand(executable, { workspacePath, effort: "high; rm -rf" })).toThrow("Effort is invalid");
    expect(() => createCodexExecCommand(executable, { workspacePath, effort: "" })).toThrow("Effort is invalid");
  });

  it("reads the aliases and effort levels the Claude CLI advertises in its own help", () => {
    const help = [
      "  --model <model>                       Model for the current session. Provide",
      "                                        an alias for the latest model (e.g.",
      "                                        'fable', 'opus', or 'sonnet') or a",
      "                                        model's full name (e.g.",
      "                                        'claude-fable-5').",
      "  --effort <level>                      Effort level for the current session",
      "                                        (low, medium, high, xhigh, max)",
    ].join("\n");
    expect(parseClaudeModelHints(help)).toEqual({
      aliases: ["fable", "opus", "sonnet"],
      efforts: ["low", "medium", "high", "xhigh", "max"],
    });
  });

  it("names nothing when the help does not", () => {
    expect(parseClaudeModelHints("Usage: claude [options]")).toBeUndefined();
  });
});

describe("continuing a conversation", () => {
  const executable = {
    commandName: "x",
    discoveredPath: process.platform === "win32" ? "C:\\tools\\x.exe" : "/tools/x",
    executablePath: process.platform === "win32" ? "C:\\tools\\x.exe" : "/tools/x",
    prefixArgs: [],
    kind: "native" as const,
  };
  const workspacePath = process.platform === "win32" ? "C:\\work" : "/work";

  it("resumes a Claude session by id", () => {
    const spec = createClaudePrintCommand(executable, { workspacePath, resumeThreadId: "sess-42" });
    expect(spec.args[spec.args.indexOf("--resume") + 1]).toBe("sess-42");
  });

  it("resumes a Codex session with only the arguments that subcommand accepts", () => {
    // MEASURED against codex-cli 0.151.0: `codex exec resume` takes neither
    // `--sandbox` nor `-C`, and exits 2 with "unexpected argument" if given
    // either. This test used to assert the argv I had written rather than the
    // argv the CLI accepts, so every Codex reply failed and the suite stayed
    // green. The sandbox travels as the config override the subcommand does
    // support, and the working directory needs no flag: the process is
    // spawned in it.
    const spec = createCodexExecCommand(executable, {
      workspacePath,
      resumeThreadId: "thread-42",
      sandbox: "read-only",
    });
    expect(spec.args.slice(0, 3)).toEqual(["exec", "resume", "thread-42"]);
    expect(spec.args).not.toContain("--sandbox");
    expect(spec.args).not.toContain("-C");
    expect(spec.args).not.toContain(workspacePath);
    expect(spec.args).toEqual(expect.arrayContaining(["-c", "sandbox_mode=read-only"]));
    // The prompt still arrives on stdin, so the trailing `-` must survive.
    expect(spec.args.at(-1)).toBe("-");
  });

  it("carries a write-capable resume as the same override, not as a flag", () => {
    const spec = createCodexExecCommand(executable, {
      workspacePath,
      resumeThreadId: "thread-42",
      sandbox: "workspace-write",
    });
    expect(spec.args).toEqual(expect.arrayContaining(["-c", "sandbox_mode=workspace-write"]));
    expect(spec.args).not.toContain("--sandbox");
  });

  it("starts a fresh conversation when no session is given", () => {
    const spec = createCodexExecCommand(executable, { workspacePath });
    expect(spec.args.includes("resume")).toBe(false);
    expect(createClaudePrintCommand(executable, { workspacePath }).args.includes("--resume")).toBe(false);
  });

  it("refuses a session id that is not plain text", () => {
    expect(() => createCodexExecCommand(executable, { workspacePath, resumeThreadId: "" })).toThrow();
    expect(() =>
      createClaudePrintCommand(executable, { workspacePath, resumeThreadId: `bad${String.fromCharCode(0)}id` }),
    ).toThrow();
  });
});

describe("finding a CLI that never put itself on PATH", () => {
  const LOCAL = "C:\\Users\\x\\AppData\\Local";
  const ROAMING = "C:\\Users\\x\\AppData\\Roaming";

  function locator(files: readonly string[], versions: readonly string[] = [], path = "") {
    return createPathExecutableLocator({
      platform: "win32",
      environment: { PATH: path, LOCALAPPDATA: LOCAL, APPDATA: ROAMING },
      isExecutableFile: async (candidate) => files.includes(candidate),
      readDirectory: async (directory) =>
        directory === `${LOCAL}\\OpenAI\\Codex\\bin` ? versions : [],
    });
  }

  it("finds Codex in its versioned install root when PATH knows nothing", async () => {
    // The exact shape that reported "not found" in a packaged build on a
    // machine where Codex was installed and working in a terminal.
    const exe = `${LOCAL}\\OpenAI\\Codex\\bin\\abc123\\codex.exe`;
    const found = await locator([exe], ["abc123"]).find("codex");
    expect(found?.executablePath).toBe(exe);
    expect(found?.kind).toBe("native");
  });

  it("prefers what PATH says over an install root", async () => {
    const onPath = "C:\\tools\\codex.exe";
    const installed = `${LOCAL}\\OpenAI\\Codex\\bin\\abc123\\codex.exe`;
    const found = await locator([onPath, installed], ["abc123"], "C:\\tools").find("codex");
    expect(found?.executablePath).toBe(onPath);
  });

  it("finds Claude Code's npm shim without PATH", async () => {
    const shim = `${ROAMING}\\npm\\claude.ps1`;
    const powershell = `${ROAMING}\\npm\\powershell.exe`;
    const found = await locator([shim, powershell]).find("claude");
    expect(found?.kind).toBe("powershell-shim");
    expect(found?.discoveredPath).toBe(shim);
  });

  it("reports nothing when the runtime is genuinely absent", async () => {
    expect(await locator([], ["abc123"]).find("codex")).toBeUndefined();
    expect(await locator([]).find("claude")).toBeUndefined();
  });

  it("searches no install root for a command it does not know", async () => {
    const stray = `${LOCAL}\\OpenAI\\Codex\\bin\\abc123\\somethingelse.exe`;
    expect(await locator([stray], ["abc123"]).find("somethingelse")).toBeUndefined();
  });
});
