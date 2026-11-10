//! What a command does, in plain words (0.840).
//!
//! Faithful port of `apps/desktop/src/shared/command-plain.ts`.

use crate::command_reach::{
    command_words, inner_commands, program_name, split_chain, tokens, wrapped,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// A sentence a step, for a person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlainCommand {
    pub said: String,
    pub careful: bool,
    pub only_looks: bool,
}

#[derive(Debug, Clone)]
struct Step {
    said: String,
    looks: bool,
    careful: bool,
}

impl Step {
    fn new(said: impl Into<String>) -> Self {
        Self {
            said: said.into(),
            looks: false,
            careful: false,
        }
    }
    fn looking(said: impl Into<String>) -> Self {
        Self {
            said: said.into(),
            looks: true,
            careful: false,
        }
    }
    fn amber(said: impl Into<String>) -> Self {
        Self {
            said: said.into(),
            looks: false,
            careful: true,
        }
    }
}

const MAX_STEPS: usize = 4;
const MAX_LOOKING_STEPS: usize = 12;
const SAID_OF_A_LONG_ONE: usize = 3;
const ONLY_LOOKS: &str = " It only looks.";
const NO_BIN: &str = " Nothing goes to the Recycle Bin or Trash.";
const SETUP_SCRIPTS: &str = " A package can run its own setup code as it installs.";

/// Worked out from the words of the command, never asked of a model.
pub fn plain_command(command: &str) -> Option<PlainCommand> {
    if command.trim().is_empty() || command.len() > 2000 {
        return None;
    }
    if crate::invariants::regex(r#"\\"|`["']"#).is_match(command) || !closed(command) {
        let url = messy_download_url(command)?;
        return Some(PlainCommand {
            said: download_and_run(&url).said,
            careful: true,
            only_looks: false,
        });
    }
    let all = steps_of(command, 0)?;
    if all.len() > MAX_LOOKING_STEPS {
        return None;
    }
    let steps = once(all.into_iter().filter(|s| !s.said.is_empty()).collect());
    if steps.is_empty() {
        return None;
    }
    let only_looks = steps.iter().all(|s| s.looks);
    if steps.len() > MAX_STEPS && !only_looks {
        return None;
    }
    let shown: Vec<String> = if steps.len() > MAX_STEPS {
        let mut v: Vec<_> = steps[..SAID_OF_A_LONG_ONE]
            .iter()
            .map(|s| s.said.clone())
            .collect();
        let more = steps.len() - SAID_OF_A_LONG_ONE;
        v.push(format!(
            "And {more} more step{}.",
            if more == 1 { "" } else { "s" }
        ));
        v
    } else {
        steps.iter().map(|s| s.said.clone()).collect()
    };
    Some(PlainCommand {
        said: format!(
            "{}{}",
            shown.join(" "),
            if only_looks { ONLY_LOOKS } else { "" }
        ),
        careful: steps.iter().any(|s| s.careful),
        only_looks,
    })
}

fn messy_download_url(command: &str) -> Option<String> {
    let re = crate::invariants::regex(
        r#"(?i)\b(?:curl|wget|irm|iwr|invoke-restmethod|invoke-webrequest)\b[^|;&]*?(https?://[^\s'"\\)]+)[^|;&]*\|\s*(?:sh|bash|zsh|iex|invoke-expression)\b"#,
    );
    if let Some(c) = re.captures(command) {
        return Some(c[1].to_string());
    }
    if crate::invariants::regex(r"(?i)\b(?:iex|invoke-expression)\b").is_match(command) {
        return runs_downloaded("iex", command);
    }
    None
}

fn closed(command: &str) -> bool {
    let mut quote = None;
    for ch in command.chars() {
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => {}
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None => {}
        }
    }
    quote.is_none()
}

fn once(steps: Vec<Step>) -> Vec<Step> {
    struct Kept {
        step: Step,
        times: usize,
    }
    let mut kept: Vec<Kept> = Vec::new();
    for step in steps {
        let merge = kept.last().is_some_and(|last| {
            last.step.said == step.said && !crate::invariants::regex(r"\.\s").is_match(&step.said)
        });
        if merge {
            let last = kept.last_mut().unwrap();
            last.times += 1;
            if step.careful {
                last.step.careful = true;
            }
            if !step.looks {
                last.step.looks = false;
            }
        } else {
            kept.push(Kept { step, times: 1 });
        }
    }
    kept.into_iter()
        .map(|Kept { step, times }| {
            if times == 1 {
                step
            } else {
                let base = step.said.trim_end_matches('.');
                let count = if times == 2 {
                    "twice".into()
                } else {
                    format!("{times} times")
                };
                Step {
                    said: format!("{base}, {count}."),
                    looks: step.looks,
                    careful: step.careful,
                }
            }
        })
        .collect()
}

fn steps_of(command: &str, depth: u32) -> Option<Vec<Step>> {
    if depth > 3 {
        return None;
    }
    let parts = split_chain(command);
    let mut steps = Vec::new();
    let mut i = 0;
    while i < parts.len() {
        if let Some(set) = setting(&parts[i].text) {
            let next_is_pipe = parts.get(i + 1).is_some_and(|p| p.after == "|");
            if !next_is_pipe {
                steps.push(set);
                i += 1;
                continue;
            }
        }
        let mut end = i;
        while end + 1 < parts.len() && parts[end + 1].after == "|" {
            end += 1;
        }
        let texts: Vec<&str> = parts[i..=end].iter().map(|p| p.text.as_str()).collect();
        let found = pipeline(&texts, depth)?;
        if parts[i].after == "||" {
            for (at, step) in found.into_iter().enumerate() {
                if at == 0 {
                    steps.push(Step {
                        said: format!("If that fails, {}", lower_first(&step.said)),
                        looks: step.looks,
                        careful: step.careful,
                    });
                } else {
                    steps.push(step);
                }
            }
        } else {
            steps.extend(found);
        }
        i = end + 1;
    }
    Some(steps)
}

fn setting(text: &str) -> Option<Step> {
    if crate::invariants::regex(r"(?i)^\s*\$\w+\s*=\s*\$(LASTEXITCODE|\?)\s*$").is_match(text) {
        return Some(Step::looking(""));
    }
    if crate::invariants::regex(r#"(?i)^\s*\$ErrorActionPreference\s*=\s*['"]?\w+['"]?\s*$"#)
        .is_match(text)
    {
        return Some(Step::looking(""));
    }
    let env = crate::invariants::regex(
        r#"(?i)^\s*\$env:([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(?:'[^']*'|"[^"$`]*"|[^\s;|&()$`'"]+)\s*$"#,
    );
    env.captures(text)
        .map(|c| Step::new(format!("Sets {} for the commands after it.", &c[1])))
}

fn pipeline(texts: &[&str], depth: u32) -> Option<Vec<Step>> {
    let sets = if texts.len() == 1 {
        leading_settings(&tokens(texts[0]))
    } else {
        Vec::new()
    };
    let words: Vec<Vec<String>> = texts
        .iter()
        .map(|text| without_redirects(&command_words(&tokens(text))))
        .collect();
    if words.iter().any(|each| each.is_empty()) {
        return None;
    }
    if texts
        .iter()
        .any(|text| crate::invariants::regex(r"(?i)^\s*(command|builtin)\s").is_match(text))
    {
        return None;
    }
    if texts.len() == 1 {
        let verb = program_name(&words[0][0]);
        if let Some(ran) = runs_downloaded(&verb, texts[0]) {
            return Some(vec![download_and_run(&ran)]);
        }
    }
    if texts.iter().any(|t| computes(t)) {
        return None;
    }
    // `redirects(text)?.length !== 0` is true when redirects is None or non-empty.
    for text in &texts[..texts.len().saturating_sub(1)] {
        match redirects(text) {
            Some(s) if s.is_empty() => {}
            _ => return None,
        }
    }
    let saved = redirects(texts[texts.len() - 1])?;
    if saved.len() > 1 {
        return None;
    }
    let found = commands(texts, &words, depth)?;
    let mut out: Vec<Step> = sets
        .into_iter()
        .map(|name| Step::new(format!("Sets {name} for this command.")))
        .collect();
    out.extend(found);
    out.extend(saved.into_iter().map(saved_in));
    Some(out)
}

fn commands(texts: &[&str], words: &[Vec<String>], depth: u32) -> Option<Vec<Step>> {
    let last = &words[words.len() - 1];
    if texts.len() > 1 && RUNNERS.contains(program_name(&last[0]).as_str()) {
        let url = downloaded(&words[0])?;
        return Some(vec![download_and_run(&url)]);
    }
    if texts.len() == 1 {
        let verb = program_name(&words[0][0]);
        if !inner_commands(texts[0]).is_empty() {
            return None;
        }
        if let Some(inner) = wrapped(&words[0]) {
            return steps_of(&inner, depth + 1);
        }
        let file = if matches!(verb.as_str(), "bash" | "sh" | "zsh") {
            words[0].get(1).cloned()
        } else if matches!(verb.as_str(), "powershell" | "pwsh") {
            value_of(&words[0][1..], &["-file"]).map(str::to_string)
        } else {
            None
        };
        if let Some(ref file) = file
            && !file.starts_with('-') {
                return Some(vec![Step::new(format!("Runs the script {file}."))]);
            }
        if matches!(
            verb.as_str(),
            "powershell" | "pwsh" | "cmd" | "bash" | "sh" | "zsh"
        ) {
            return None;
        }
        let step = step_of(&words[0])?;
        if crate::invariants::regex(r"(?i)^\s*(sudo|doas)\s").is_match(texts[0]) {
            return Some(vec![Step {
                said: format!("{} It runs with administrator rights.", step.said),
                looks: step.looks,
                careful: step.careful,
            }]);
        }
        return Some(vec![step]);
    }
    if texts.iter().any(|text| !inner_commands(text).is_empty()) {
        return None;
    }
    let first = step_of(&words[0])?;
    let rest = &words[1..];
    if !rest.iter().all(|each| {
        FILTERS.contains(program_name(&each[0]).as_str()) && !writes_from_a_filter(each)
    }) {
        return None;
    }
    let narrowed = rest.iter().any(|each| {
        let name = program_name(&each[0]);
        matches!(name.as_str(), "head" | "tail")
            || (matches!(name.as_str(), "select-object" | "select")
                && each.iter().any(|word| {
                    crate::invariants::regex(r"(?i)^-(first|last|skip|index)$").is_match(word)
                }))
    });
    if narrowed && first.said.starts_with("Shows what is in ") {
        let rest_said = &first.said["Shows what is in ".len()..];
        return Some(vec![Step {
            said: format!("Shows part of {rest_said}"),
            looks: first.looks,
            careful: first.careful,
        }]);
    }
    Some(vec![first])
}

fn leading_settings(words: &[String]) -> Vec<String> {
    let mut names = Vec::new();
    for word in words {
        let Some(c) = crate::invariants::regex(r"^([A-Za-z_][A-Za-z0-9_]*)=").captures(word) else {
            break;
        };
        names.push(c[1].to_string());
    }
    names
}

fn computes(text: &str) -> bool {
    let bare = crate::invariants::regex(r"\w@\{[\w.:-]+\}").replace_all(text, "");
    let mut quote = None;
    for ch in bare.chars() {
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => {}
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None if ch == '(' || ch == '{' => return true,
            None => {}
        }
    }
    false
}

#[derive(Clone)]
struct Saved {
    file: String,
    adds: bool,
    errors: bool,
}

fn saved_in(saved: Saved) -> Step {
    let what = if saved.errors {
        "Its error messages"
    } else {
        "What it prints"
    };
    if saved.adds {
        let verb = if saved.errors { "are" } else { "is" };
        return Step::new(format!("{what} {verb} added to the end of {}.", saved.file));
    }
    if crate::invariants::regex(r"(?i)\.(log|tap|out)$").is_match(&saved.file) {
        let verb = if saved.errors { "are" } else { "is" };
        return Step::new(format!("{what} {verb} saved in {}.", saved.file));
    }
    let verb = if saved.errors { "replace" } else { "replaces" };
    Step::amber(format!("{what} {verb} what was in {}.", saved.file))
}

fn writes_from_a_filter(words: &[String]) -> bool {
    let name = program_name(&words[0]);
    match name.as_str() {
        "sed" => words
            .iter()
            .any(|w| crate::invariants::regex(r"^-[a-z]*i").is_match(w) || w.starts_with("--in-place")),
        "sort" => words.iter().any(|w| {
            w == "-o" || w.starts_with("--output") || crate::invariants::regex(r"^-[a-z]*o").is_match(w)
        }),
        _ => false,
    }
}

static RUNNERS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    HashSet::from([
        "sh",
        "bash",
        "zsh",
        "dash",
        "iex",
        "invoke-expression",
        "python",
        "python3",
        "node",
        "powershell",
        "pwsh",
    ])
});

static FILTERS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    HashSet::from([
        "head",
        "tail",
        "grep",
        "egrep",
        "rg",
        "findstr",
        "select-string",
        "sls",
        "sort",
        "sort-object",
        "uniq",
        "wc",
        "cut",
        "tr",
        "less",
        "more",
        "select-object",
        "select",
        "where-object",
        "?",
        "measure-object",
        "measure",
        "format-table",
        "ft",
        "format-list",
        "fl",
        "out-string",
        "out-host",
        "out-null",
        "jq",
        "column",
        "cat",
        "sed",
        "group-object",
        "convertfrom-json",
        "convertto-json",
        "nl",
        "where",
    ])
});

static DOWNLOADERS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    HashSet::from([
        "curl",
        "wget",
        "irm",
        "iwr",
        "invoke-restmethod",
        "invoke-webrequest",
    ])
});

fn downloaded(words: &[String]) -> Option<String> {
    if !DOWNLOADERS.contains(program_name(&words[0]).as_str()) {
        return None;
    }
    words[1..]
        .iter()
        .find(|w| crate::invariants::regex(r"(?i)^https?://").is_match(w))
        .cloned()
}

fn runs_downloaded(verb: &str, text: &str) -> Option<String> {
    if matches!(verb, "sh" | "bash" | "zsh" | "dash") {
        return crate::invariants::regex(
            r#"(?i)(?:\$\(|<\()\s*(?:curl|wget)\b[^)]*?(https?://[^\s'")]+)"#,
        )
        .captures(text)
        .map(|c| c[1].to_string());
    }
    if verb != "iex" && verb != "invoke-expression" {
        return None;
    }
    if let Some(c) = crate::invariants::regex(
        r#"(?i)\b(?:curl|wget|irm|iwr|invoke-restmethod|invoke-webrequest)\b[^)]*?(https?://[^\s'")]+)"#,
    )
    .captures(text)
    {
        return Some(c[1].to_string());
    }
    crate::invariants::regex(r#"(?i)downloadstring\(\s*['"](https?://[^'"]+)['"]"#)
        .captures(text)
        .map(|c| c[1].to_string())
}

fn download_and_run(url: &str) -> Step {
    Step::amber(format!(
        "Downloads a script from {} and runs it straight away, without showing it to you first.",
        host_of(url)
    ))
}

// --- one command -------------------------------------------------------------

fn step_of(words: &[String]) -> Option<Step> {
    let verb = program_name(&words[0]);
    let args = &words[1..];
    if let Some(version) = version_of(&verb, args) {
        return Some(version);
    }
    match verb.as_str() {
        "git" => git(args),
        "gh" => gh(args),
        "npm" | "pnpm" | "yarn" | "bun" => packages(&verb, args),
        "npx" | "bunx" => run_package(args),
        "pip" | "pip3" => pip(args),
        "uv" => {
            if args.first().map(|s| s.as_str()) == Some("pip") {
                pip(&args[1..])
            } else {
                None
            }
        }
        "python" | "python3" | "py" => python(args),
        "node" | "deno" => script(args),
        "pytest" => Some(Step::new(format!(
            "Runs this project{AP}s Python tests.",
            AP = '\u{2019}'
        ))),
        "vitest" | "jest" => Some(Step::new(format!(
            "Runs this project{AP}s tests.",
            AP = '\u{2019}'
        ))),
        "cargo" => cargo(args),
        "go" => go_tool(args),
        "tsc" => {
            if args.iter().any(|a| a == "--noEmit") {
                Some(Step::looking(format!(
                    "Checks this project{AP}s TypeScript for mistakes.",
                    AP = '\u{2019}'
                )))
            } else {
                None
            }
        }
        "rm" | "del" | "erase" | "rd" | "rmdir" | "ri" | "remove-item" | "unlink" => {
            remove(&verb, args)
        }
        "mv" | "move" | "mi" | "move-item" => move_cmd(&verb, args),
        "ren" | "rename" | "rni" | "rename-item" => rename(args),
        "cp" | "copy" | "cpi" | "copy-item" | "xcopy" => copy(&verb, args),
        "robocopy" => robocopy(args),
        "mkdir" | "md" => named(&plain_of(args, &verb, &[]), |folders| {
            format!(
                "Makes the folder{} {}.",
                plural(folders),
                join(folders)
            )
        }),
        "new-item" | "ni" => new_item(args),
        "touch" => named(&plain_of(args, &verb, &[]), |files| {
            format!(
                "Makes {} if {} not there yet.",
                join(files),
                if files.len() == 1 { "it is" } else { "they are" }
            )
        }),
        "type" => {
            let plain = plain_of(args, &verb, &[]);
            if plain.iter().all(|word| crate::invariants::regex(r"[./\\]").is_match(word)) {
                looks(named(&plain, |files| {
                    format!("Shows what is in {}.", join(files))
                }))
            } else {
                None
            }
        }
        "cat" | "gc" | "get-content" | "less" | "more" => {
            let part = if value_of(args, &["-tail", "-last"]).is_some() {
                "the end of"
            } else if value_of(args, &["-totalcount", "-head", "-first"]).is_some() {
                "the start of"
            } else {
                "what is in"
            };
            let mut files = values_of(args, &["-path", "-literalpath"]);
            files.extend(plain_of(args, &verb, &[]));
            let files: Vec<String> = files.into_iter().flat_map(|v| comma_list(&v)).collect();
            looks(named(&files, |files| {
                format!("Shows {part} {}.", join(files))
            }))
        }
        "test-path" => {
            let mut files = values_of(args, &["-path", "-literalpath"]);
            files.extend(plain_of(args, &verb, &[]));
            let files: Vec<String> = files.into_iter().flat_map(|v| comma_list(&v)).collect();
            looks(named(&files, |files| {
                format!(
                    "Checks whether {} {} there.",
                    join(files),
                    if files.len() == 1 { "is" } else { "are" }
                )
            }))
        }
        "get-date" | "date" => {
            if args
                .iter()
                .any(|arg| arg == "-s" || arg.starts_with("--set"))
            {
                None
            } else {
                Some(Step::looking("Shows the date and time."))
            }
        }
        "sleep" | "start-sleep" => waits(args),
        "get-process" | "gps" => {
            let mut which = values_of(args, &["-id", "-name"]);
            which.extend(plain_of(args, &verb, &["-id"]));
            let which: Vec<String> = which.into_iter().flat_map(|v| comma_list(&v)).collect();
            Some(Step::looking(if which.is_empty() {
                "Lists the programs running on this computer.".into()
            } else {
                format!(
                    "Looks up the running program{} {}.",
                    plural(&which),
                    join(&which)
                )
            }))
        }
        "ps" | "tasklist" => Some(Step::looking(
            "Lists the programs running on this computer.",
        )),
        "get-ciminstance" | "gcim" | "get-wmiobject" | "gwmi" => {
            let kind = value_of(args, &["-classname", "-class"])
                .map(str::to_string)
                .or_else(|| plain_of(args, &verb, &[]).into_iter().next())
                .unwrap_or_default()
                .to_lowercase();
            if kind == "win32_process" {
                Some(Step::looking(
                    "Lists the programs running on this computer.",
                ))
            } else if kind == "win32_logicaldisk" {
                Some(Step::looking(
                    "Shows how much room is left on each drive.",
                ))
            } else {
                None
            }
        }
        "get-psdrive" | "gdr" | "df" => Some(Step::looking(
            "Shows how much room is left on each drive.",
        )),
        "head" | "tail" => looks(named(&plain_of(args, &verb, &["-n", "-c"]), |files| {
            format!(
                "Shows the {} of {}.",
                if verb == "head" { "start" } else { "end" },
                join(files)
            )
        })),
        "ls" | "dir" | "gci" | "get-childitem" | "tree" => listing(&verb, args),
        "find" => {
            if args.iter().any(|arg| {
                crate::invariants::regex(r"^-(delete|exec|execdir|ok|okdir|fprint\w*|fls)$")
                    .is_match(arg)
            }) {
                None
            } else {
                let where_ = plain_of(args, &verb, &[])
                    .into_iter()
                    .next();
                Some(Step::looking(format!(
                    "Looks for files in {}.",
                    place(where_.as_deref())
                )))
            }
        }
        "grep" | "egrep" | "rg" | "ag" | "findstr" | "sls" | "select-string" => {
            search(&verb, args)
        }
        "pwd" | "gl" | "get-location" => Some(Step::looking("Shows which folder this is.")),
        "echo" | "printf" | "write-output" | "write-host" => {
            Some(Step::looking("Prints a line of text."))
        }
        "which" | "where" | "gcm" | "get-command" => {
            looks(named(&plain_of(args, &verb, &[]), |programs| {
                format!(
                    "Finds where {} {} installed.",
                    join(programs),
                    if programs.len() == 1 { "is" } else { "are" }
                )
            }))
        }
        "cd" | "chdir" | "pushd" | "sl" | "set-location" => {
            let to = plain_of(args, &verb, &[]).into_iter().next()?;
            Some(Step::looking(if to == ".." {
                "Goes up one folder.".into()
            } else {
                format!("Goes into {to}.")
            }))
        }
        "exit" => {
            if args.len() <= 1 {
                Some(Step::looking(""))
            } else {
                None
            }
        }
        "start" | "open" | "xdg-open" | "start-process" | "saps" => opens(args),
        "curl" | "wget" | "irm" | "iwr" | "invoke-restmethod" | "invoke-webrequest" => {
            fetches(&verb, args)
        }
        _ => None,
    }
}

static VERSIONED: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    HashSet::from([
        "node",
        "npm",
        "pnpm",
        "yarn",
        "bun",
        "deno",
        "python",
        "python3",
        "py",
        "pip",
        "pip3",
        "git",
        "gh",
        "cargo",
        "rustc",
        "go",
        "java",
        "tsc",
        "uv",
        "docker",
        "code",
        "claude",
        "codex",
        "opencode",
        "cursor-agent",
        "gemini",
    ])
});

fn version_of(verb: &str, args: &[String]) -> Option<Step> {
    if !VERSIONED.contains(verb) || args.len() != 1 {
        return None;
    }
    let a = args[0].as_str();
    if !matches!(a, "--version" | "-v" | "-V" | "version" | "-version") {
        return None;
    }
    Some(Step::looking(format!(
        "Shows which version of {verb} is installed."
    )))
}

// --- git ---------------------------------------------------------------------

fn git(all: &[String]) -> Option<Step> {
    let mut at = 0;
    while at < all.len() && all[at].starts_with('-') {
        at += if all[at] == "-C" || all[at] == "-c" {
            2
        } else {
            1
        };
    }
    let sub = all.get(at)?.as_str();
    let args = &all[at + 1..];
    let plain = plain_of(
        args,
        "git",
        &[
            "-m",
            "--message",
            "-b",
            "-B",
            "-c",
            "-C",
            "-u",
            "--set-upstream-to",
            "--author",
            "--format",
            "--pretty",
            "-n",
            "--depth",
            "-o",
            "--origin",
        ],
    );
    let has = |flags: &[&str]| -> bool {
        args.iter().any(|arg| {
            flags.iter().any(|flag| {
                arg == flag || (flag.starts_with("--") && arg.starts_with(&format!("{flag}=")))
            })
        })
    };
    match sub {
        "status" => Some(Step::looking("Shows which files have changed.")),
        "diff" => {
            if has(&["--check"]) {
                Some(Step::looking(
                    "Checks the changes for stray spaces and other whitespace mistakes.",
                ))
            } else if has(&["--staged", "--cached"]) {
                Some(Step::looking(
                    "Shows the changes marked for the next commit.",
                ))
            } else {
                Some(Step::looking("Shows what has changed in the files."))
            }
        }
        "grep" => search("git grep", args),
        "worktree" => {
            if args.first().map(|s| s.as_str()) == Some("list") {
                Some(Step::looking(
                    "Lists the folders this repository is checked out in.",
                ))
            } else {
                None
            }
        }
        "log" | "shortlog" | "reflog" => {
            if matches!(args.first().map(|s| s.as_str()), Some("expire" | "delete")) {
                None
            } else {
                Some(Step::looking("Lists recent commits."))
            }
        }
        "show" => {
            let at = if plain.len() == 1 {
                crate::invariants::regex(r"^([^:\s]{2,}):(.+)$").captures(&plain[0])
            } else {
                None
            };
            Some(Step::looking(if let Some(c) = at {
                format!("Shows {} as it was at {}.", &c[2], &c[1])
            } else {
                "Shows a commit and what it changed.".into()
            }))
        }
        "blame" => Some(Step::looking(format!(
            "Shows who last changed each line of {}.",
            plain.first().map(|s| s.as_str()).unwrap_or("a file")
        ))),
        "ls-files" | "rev-parse" | "describe" => Some(Step::looking(
            "Reads what Git knows about this folder.",
        )),
        "remote" => {
            if args.is_empty() || (has(&["-v", "--verbose"]) && args.len() == 1) {
                Some(Step::looking(
                    "Lists where this repository sends and gets commits.",
                ))
            } else {
                None
            }
        }
        "add" => {
            if plain.is_empty() && !has(&["-A", "--all", "-u", "--update"]) {
                None
            } else if has(&["-A", "--all"]) || plain.iter().any(|p| p == ".") {
                Some(Step::new(
                    "Marks every change in this folder to go in the next commit.",
                ))
            } else if has(&["-u", "--update"]) {
                Some(Step::new(
                    "Marks every change to files Git already tracks to go in the next commit.",
                ))
            } else {
                Some(Step::new(format!(
                    "Marks {} to go in the next commit.",
                    join(&plain)
                )))
            }
        }
        "commit" => {
            let message = value_of(args, &["-m", "--message", "-am"]);
            let with_message = match message {
                None => String::new(),
                Some(m) => {
                    let first = m.split('\n').next().unwrap_or(m);
                    format!(
                        " with the message {LDQ}{cut}{RDQ}",
                        LDQ = '\u{201c}',
                        RDQ = '\u{201d}',
                        cut = cut(first, 72)
                    )
                }
            };
            if has(&["--amend"]) {
                Some(Step::new(format!(
                    "Rewrites the last commit{with_message}, on this computer only."
                )))
            } else {
                let every = has(&["-a", "--all"])
                    || args
                        .iter()
                        .any(|arg| crate::invariants::regex(r"^-[a-zA-Z]*a[a-zA-Z]*$").is_match(arg));
                Some(Step::new(format!(
                    "Saves {} as a commit{with_message}, on this computer only.",
                    if every {
                        "every change to tracked files"
                    } else {
                        "the marked changes"
                    }
                )))
            }
        }
        "push" => push(args),
        "pull" => {
            let from = plain
                .first()
                .map(|s| s.as_str())
                .unwrap_or("the repository this branch is linked to");
            let how = if has(&["--rebase", "-r"]) {
                format!(
                    "puts this branch{AP}s own commits on top of them",
                    AP = '\u{2019}'
                )
            } else {
                "merges them into this branch".into()
            };
            Some(Step::new(format!("Gets new commits from {from} and {how}.")))
        }
        "fetch" => {
            let from = if has(&["--all"]) {
                "every linked repository".into()
            } else {
                plain
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "the linked repository".into())
            };
            Some(Step::new(format!(
                "Gets new commits from {from} without changing your files."
            )))
        }
        "clone" => {
            let from = plain.first()?;
            let into = match plain.get(1) {
                None => "a new folder here".into(),
                Some(f) => format!("the folder {f}"),
            };
            Some(Step::new(format!(
                "Downloads a copy of the repository {} into {into}.",
                repository_of(from)
            )))
        }
        "switch" => {
            if let Some(made) = value_of(args, &["-c", "-C", "--create"]) {
                return Some(Step::new(format!(
                    "Makes a new branch, {made}, and switches to it."
                )));
            }
            let name = plain.first()?;
            Some(Step::new(format!("Switches to the branch {name}.")))
        }
        "checkout" => checkout(args, &plain),
        "restore" => {
            if plain.is_empty() {
                return None;
            }
            if has(&["--staged", "-S"]) && !has(&["--worktree", "-W"]) {
                Some(Step::new(format!(
                    "Takes {} out of the next commit; the changes stay in the files.",
                    join(&plain)
                )))
            } else {
                Some(Step::amber(format!(
                    "Throws away the changes to {} that are not committed. They cannot be brought back.",
                    everything(&plain)
                )))
            }
        }
        "reset" => {
            if has(&["--hard"]) {
                let move_to = match plain.first() {
                    None => String::new(),
                    Some(p) => format!(", and moves this branch back to {p}"),
                };
                return Some(Step::amber(format!(
                    "Throws away every change not committed, in every file Git tracks{move_to}. It cannot be undone."
                )));
            }
            let files: Vec<String> = if let Some(i) = args.iter().position(|a| a == "--") {
                args[i + 1..].to_vec()
            } else {
                plain
                    .iter()
                    .filter(|one| one.as_str() != "HEAD" && looks_like_a_file(one))
                    .cloned()
                    .collect()
            };
            if !files.is_empty() {
                return Some(Step::new(format!(
                    "Takes {} out of the next commit; the changes stay in the files.",
                    join(&files)
                )));
            }
            if plain.is_empty() || (plain.len() == 1 && plain[0] == "HEAD") {
                return Some(Step::new(
                    "Takes everything out of the next commit; the changes stay in the files.",
                ));
            }
            if plain.len() != 1 {
                return None;
            }
            let soft = if has(&["--soft"]) {
                ", marked for the next commit"
            } else {
                ""
            };
            Some(Step::new(format!(
                "Moves this branch back to {}; the changes since stay in the files{soft}.",
                plain[0]
            )))
        }
        "clean" => {
            if has(&["-n", "--dry-run"])
                || args
                    .iter()
                    .any(|arg| crate::invariants::regex(r"^-[a-z]*n[a-z]*$").is_match(arg))
            {
                return Some(Step::looking(
                    "Lists the files Git does not track that it would delete.",
                ));
            }
            if !args.iter().any(|arg| {
                crate::invariants::regex(r"^-[a-z]*f[a-z]*$").is_match(arg) || arg == "--force"
            }) {
                return None;
            }
            let folders = if args
                .iter()
                .any(|arg| crate::invariants::regex(r"^-[a-z]*d[a-z]*$").is_match(arg))
            {
                ", and folders"
            } else {
                ""
            };
            let ignored = if args
                .iter()
                .any(|arg| crate::invariants::regex(r"(?i)^-[a-z]*x[a-z]*$").is_match(arg))
            {
                ", ignored ones too"
            } else {
                ""
            };
            Some(Step::amber(format!(
                "Deletes every file Git does not track{folders}{ignored}: new files never committed are gone for good."
            )))
        }
        "stash" => {
            let kind = plain.first().map(|s| s.as_str()).unwrap_or("push");
            match kind {
                "push" | "save" => Some(Step::new(
                    "Puts the changes not committed aside, to bring back later.",
                )),
                "pop" | "apply" => Some(Step::new("Brings back changes that were put aside.")),
                "list" | "show" => {
                    Some(Step::looking("Lists changes that were put aside."))
                }
                "drop" | "clear" => Some(Step::amber(format!(
                    "Throws away {} that was put aside.",
                    if kind == "clear" {
                        "every set of changes"
                    } else {
                        "a set of changes"
                    }
                ))),
                _ => None,
            }
        }
        "branch" => branch(args),
        "merge" => {
            if has(&["--abort"]) {
                Some(Step::new(
                    "Stops the merge under way and puts things back as they were before it.",
                ))
            } else {
                let name = plain.first()?;
                Some(Step::new(format!(
                    "Merges the branch {name} into this one."
                )))
            }
        }
        "rebase" => {
            if has(&["--abort"]) {
                Some(Step::new(
                    "Stops the rebase under way and puts the branch back as it was.",
                ))
            } else if has(&["--continue"]) {
                Some(Step::new("Carries on with the rebase under way."))
            } else {
                let onto = plain.first()?;
                Some(Step::new(format!(
                    "Replays this branch{AP}s commits on top of {onto}, rewriting them.",
                    AP = '\u{2019}'
                )))
            }
        }
        "tag" => {
            if plain.is_empty() || has(&["-l", "--list"]) {
                Some(Step::looking("Lists the tags."))
            } else if has(&["-d", "--delete"]) {
                Some(Step::new(format!(
                    "Deletes the tag {} on this computer.",
                    plain[0]
                )))
            } else {
                Some(Step::new(format!(
                    "Marks this commit with the tag {}.",
                    plain[0]
                )))
            }
        }
        "rm" => {
            if plain.is_empty() {
                return None;
            }
            if has(&["--cached"]) {
                Some(Step::new(format!(
                    "Stops Git tracking {}; the file{} stay{} on this computer.",
                    join(&plain),
                    plural(&plain),
                    if plain.len() == 1 { "s" } else { "" }
                )))
            } else {
                Some(Step::new(format!(
                    "Deletes {} and marks the deletion for the next commit.",
                    join(&plain)
                )))
            }
        }
        "mv" => {
            if plain.len() == 2 {
                Some(Step::new(format!(
                    "Moves {} to {}, and tells Git.",
                    plain[0], plain[1]
                )))
            } else {
                None
            }
        }
        "init" => Some(Step::new(
            "Starts Git in this folder, so changes to it can be tracked.",
        )),
        _ => None,
    }
}

fn push(args: &[String]) -> Option<Step> {
    let plain = plain_of(args, "git", &["-o", "--push-option", "--repo"]);
    let remote = plain.first().map(|s| s.as_str());
    let refs = if plain.len() > 1 { &plain[1..] } else { &[][..] };
    let where_ = remote.unwrap_or("the repository this branch is linked to");
    let branch_of = |r: &str| -> String {
        r.trim_start_matches('+')
            .split(':')
            .next_back()
            .unwrap_or(r)
            .trim_start_matches("refs/heads/")
            .to_string()
    };
    let branch_name = refs.first().map(|r| branch_of(r));
    let which = match branch_name.as_deref() {
        None | Some("HEAD") => "this branch".into(),
        Some(b) => b.to_string(),
    };
    if args.iter().any(|a| a == "--delete" || a == "-d")
        || refs.iter().any(|r| r.starts_with(':'))
    {
        return Some(Step::amber(format!(
            "Deletes the branch {which} on {where_}."
        )));
    }
    if args.iter().any(|arg| arg.starts_with("--force-with-lease")) {
        return Some(Step::amber(format!(
            "Replaces {which} on {where_} with this computer{AP}s version, unless someone else changed it there since you last fetched. Commits on {where_} that are not on this computer are lost.",
            AP = '\u{2019}'
        )));
    }
    if args.iter().any(|arg| {
        arg == "--force"
            || arg == "-f"
            || (crate::invariants::regex(r"^-[a-zA-Z]*f[a-zA-Z]*$").is_match(arg)
                && !arg.starts_with("--"))
    }) || refs.iter().any(|r| r.starts_with('+'))
    {
        return Some(Step::amber(format!(
            "Replaces {which} on {where_} with this computer{AP}s version: commits there that are not on this computer are lost.",
            AP = '\u{2019}'
        )));
    }
    if args.iter().any(|a| a == "--tags") {
        return Some(Step::new(format!("Sends every tag to {where_}.")));
    }
    Some(Step::new(match branch_name.as_deref() {
        None | Some("HEAD") => format!(
            "Sends this branch{AP}s new commits to {where_}.",
            AP = '\u{2019}'
        ),
        Some(b) => format!("Sends the commits on {b} to {where_}."),
    }))
}

fn checkout(args: &[String], plain: &[String]) -> Option<Step> {
    if let Some(made) = value_of(args, &["-b", "-B"]) {
        return Some(Step::new(format!(
            "Makes a new branch, {made}, and switches to it."
        )));
    }
    let files: Vec<String> = if let Some(dashes) = args.iter().position(|a| a == "--") {
        args[dashes + 1..].to_vec()
    } else if plain.len() == 1 && looks_like_a_file(&plain[0]) {
        plain.to_vec()
    } else {
        Vec::new()
    };
    if !files.is_empty() {
        return Some(Step::amber(format!(
            "Throws away the changes to {} that are not committed. They cannot be brought back.",
            everything(&files)
        )));
    }
    if plain.len() != 1 {
        return None;
    }
    if crate::invariants::regex(r"^HEAD|[~^]|^[0-9a-f]{7,40}$").is_match(&plain[0]) {
        return Some(Step::new(format!(
            "Puts the files as they were at {}, outside any branch.",
            plain[0]
        )));
    }
    Some(Step::new(format!(
        "Switches to {}. If {} is a file rather than a branch, its changes not committed are thrown away.",
        plain[0], plain[0]
    )))
}

fn branch(args: &[String]) -> Option<Step> {
    let plain: Vec<String> = args
        .iter()
        .filter(|arg| !arg.starts_with('-'))
        .cloned()
        .collect();
    if args.iter().any(|a| a == "--show-current") {
        return Some(Step::looking("Shows which branch this is."));
    }
    if args.iter().any(|a| a == "-D")
        || (args.iter().any(|a| a == "--delete") && args.iter().any(|a| a == "--force"))
    {
        if plain.is_empty() {
            return None;
        }
        return Some(Step::amber(format!(
            "Deletes the branch{} {}, even if {} commits are on no other branch: those commits can be lost.",
            plural(&plain),
            join(&plain),
            if plain.len() == 1 { "its" } else { "their" }
        )));
    }
    if args.iter().any(|a| a == "-d" || a == "--delete") {
        if plain.is_empty() {
            return None;
        }
        return Some(Step::new(format!(
            "Deletes the branch{} {}, only if {} commits are merged.",
            plural(&plain),
            join(&plain),
            if plain.len() == 1 { "its" } else { "their" }
        )));
    }
    if args.iter().any(|a| a == "-m" || a == "-M") {
        if plain.is_empty() {
            return None;
        }
        let renamed = if plain.len() == 2 {
            format!("{} to {}", plain[0], plain[1])
        } else {
            format!("to {}", plain[0])
        };
        return Some(Step::new(format!("Renames the branch {renamed}.")));
    }
    if plain.is_empty() {
        return Some(Step::looking("Lists the branches."));
    }
    if plain.len() == 1 {
        Some(Step::new(format!(
            "Makes a new branch, {}, without switching to it.",
            plain[0]
        )))
    } else {
        None
    }
}

fn gh(args: &[String]) -> Option<Step> {
    let area = args.first().map(|s| s.as_str())?;
    let act = args.get(1).map(|s| s.as_str());
    let number = args[2..]
        .iter()
        .find(|arg| crate::invariants::regex(r"^#?\d+$").is_match(arg));
    let it = match number {
        None => String::new(),
        Some(n) => format!(" #{}", n.trim_start_matches('#')),
    };
    let read = ["view", "list", "status", "checks", "diff", "watch"];
    if matches!(area, "pr" | "issue" | "run" | "release" | "repo")
        && act.is_some_and(|a| read.contains(&a))
    {
        let act = act.unwrap();
        let one = number.is_some() && matches!(area, "pr" | "issue");
        let said = if one {
            format!(
                "Reads the {}{it} on GitHub.",
                if area == "pr" { "pull request" } else { "issue" }
            )
        } else {
            format!(
                "Reads {} on GitHub.",
                match area {
                    "pr" => "pull requests",
                    "issue" => "issues",
                    "run" => "check runs",
                    "release" => "releases",
                    _ => "the repository",
                }
            )
        };
        let _ = act;
        return Some(Step::looking(said));
    }
    if area == "pr" {
        match act {
            Some("create") => {
                return Some(Step::new(format!(
                    "Opens a pull request on GitHub{}.",
                    if args.iter().any(|a| a == "--draft") {
                        ", as a draft"
                    } else {
                        ""
                    }
                )));
            }
            Some("merge") => {
                return Some(Step::new(format!(
                    "Merges the pull request{it} on GitHub."
                )));
            }
            Some("close") => {
                return Some(Step::new(format!(
                    "Closes the pull request{it} on GitHub."
                )));
            }
            Some("comment" | "review") => {
                return Some(Step::new(format!(
                    "Posts a {} on the pull request{it} on GitHub.",
                    act.unwrap()
                )));
            }
            Some("checkout") => {
                return Some(Step::new(format!(
                    "Gets the pull request{it} onto this computer and switches to it."
                )));
            }
            _ => {}
        }
    }
    if area == "issue" {
        match act {
            Some("create") => return Some(Step::new("Opens an issue on GitHub.")),
            Some("comment") => {
                return Some(Step::new(format!(
                    "Posts a comment on the issue{it} on GitHub."
                )));
            }
            Some("close") => {
                return Some(Step::new(format!("Closes the issue{it} on GitHub.")));
            }
            _ => {}
        }
    }
    if area == "release" && act == Some("create") {
        return Some(Step::new("Publishes a release on GitHub."));
    }
    if area == "repo" && act == Some("create") {
        return Some(Step::new("Makes a new repository on GitHub."));
    }
    if area == "repo" && act == Some("clone") {
        let name = args.get(2)?;
        return Some(Step::new(format!(
            "Downloads a copy of the repository {name} from GitHub into a new folder here."
        )));
    }
    if area == "run" && act == Some("rerun") {
        return Some(Step::new("Runs the checks on GitHub again."));
    }
    if area == "auth" && act == Some("status") {
        return Some(Step::looking("Shows which GitHub account is signed in."));
    }
    None
}

// --- packages and scripts ----------------------------------------------------

static BARE_SCRIPTS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    HashSet::from([
        "build", "dev", "lint", "typecheck", "format", "preview", "check", "serve", "watch",
    ])
});

fn packages(manager: &str, args: &[String]) -> Option<Step> {
    let takes_value: &[&str] = match manager {
        "npm" => &["--prefix", "-w", "--workspace"],
        "pnpm" => &["-C", "--dir", "--filter", "-F"],
        _ => &["--cwd"],
    };
    let plain = plain_of(args, manager, takes_value);
    let sub = plain.first().map(|s| s.as_str());
    let rest = if plain.len() > 1 {
        &plain[1..]
    } else {
        &[][..]
    };
    let global = args.iter().any(|a| a == "-g" || a == "--global")
        || (manager == "yarn" && sub == Some("global"));
    match sub {
        None => {
            if manager == "yarn" {
                Some(Step::new(format!(
                    "Installs the packages this project lists, downloading them from the internet.{SETUP_SCRIPTS}"
                )))
            } else {
                None
            }
        }
        Some("install" | "i" | "add" | "ci") => {
            let names: &[String] = if sub == Some("ci") { &[] } else { rest };
            if names.is_empty() {
                Some(Step::new(format!(
                    "Installs the packages this project lists, downloading them from the internet.{SETUP_SCRIPTS}"
                )))
            } else {
                Some(Step::new(format!(
                    "Downloads {} from the npm registry and installs {} {}.{SETUP_SCRIPTS}",
                    join(names),
                    if names.len() == 1 { "it" } else { "them" },
                    if global {
                        "for every project on this computer"
                    } else {
                        "in this project"
                    }
                )))
            }
        }
        Some("uninstall" | "remove" | "rm" | "un") => {
            if rest.is_empty() {
                None
            } else {
                Some(Step::new(format!(
                    "Removes {} from {}.",
                    join(rest),
                    if global { "this computer" } else { "this project" }
                )))
            }
        }
        Some("test" | "t" | "start") => {
            let name = if sub == Some("t") { "test" } else { sub.unwrap() };
            if manager == "bun" && name != "start" {
                Some(Step::new(format!(
                    "Runs this project{AP}s tests.",
                    AP = '\u{2019}'
                )))
            } else {
                Some(Step::new(project_script(name)))
            }
        }
        Some("run" | "run-script") => {
            if rest.is_empty() {
                Some(Step::looking(format!(
                    "Lists this project{AP}s scripts.",
                    AP = '\u{2019}'
                )))
            } else {
                Some(Step::new(project_script(&rest[0])))
            }
        }
        Some("exec") => {
            if manager == "npm" || rest.is_empty() {
                None
            } else {
                Some(Step::new(format!(
                    "Runs {}, a program this project installed.",
                    rest[0]
                )))
            }
        }
        Some("dlx") => {
            if rest.is_empty() {
                None
            } else {
                Some(Step::new(format!(
                    "Downloads the package {} and runs it.",
                    rest[0]
                )))
            }
        }
        Some("list" | "ls") => Some(Step::looking(
            "Lists the packages installed in this project.",
        )),
        Some("outdated") => Some(Step::looking(
            "Lists the packages that have newer versions.",
        )),
        Some(other) if manager != "npm" && BARE_SCRIPTS.contains(other) => {
            Some(Step::new(project_script(other)))
        }
        _ => None,
    }
}

fn project_script(name: &str) -> String {
    format!(
        "Runs this project{AP}s {LDQ}{name}{RDQ} script: whatever its package.json says that is.",
        AP = '\u{2019}',
        LDQ = '\u{201c}',
        RDQ = '\u{201d}'
    )
}

fn run_package(args: &[String]) -> Option<Step> {
    let name = plain_of(args, "npx", &["-p", "--package"])
        .into_iter()
        .next()?;
    Some(Step::new(format!(
        "Runs {name}, downloading it first if this project does not have it."
    )))
}

fn pip(args: &[String]) -> Option<Step> {
    let sub = args.first().map(|s| s.as_str())?;
    let plain = plain_of(
        &args[1..],
        "pip",
        &[
            "-r",
            "--requirement",
            "-e",
            "--editable",
            "-i",
            "--index-url",
            "-t",
            "--target",
        ],
    );
    match sub {
        "install" => {
            if let Some(listed) = value_of(args, &["-r", "--requirement"]) {
                return Some(Step::new(format!(
                    "Downloads the Python packages listed in {listed} and installs them.{SETUP_SCRIPTS}"
                )));
            }
            let editable = value_of(args, &["-e", "--editable"]);
            if editable.is_some() || (plain.len() == 1 && plain[0] == ".") {
                return Some(Step::new(format!(
                    "Installs the Python project in {}.{SETUP_SCRIPTS}",
                    place(editable)
                )));
            }
            if plain.is_empty() {
                None
            } else {
                Some(Step::new(format!(
                    "Downloads {} from PyPI and installs {}.{SETUP_SCRIPTS}",
                    join(&plain),
                    if plain.len() == 1 { "it" } else { "them" }
                )))
            }
        }
        "uninstall" => {
            if plain.is_empty() {
                None
            } else {
                Some(Step::new(format!(
                    "Removes the Python package{} {}.",
                    plural(&plain),
                    join(&plain)
                )))
            }
        }
        "list" | "freeze" | "show" => {
            Some(Step::looking("Lists the Python packages installed."))
        }
        _ => None,
    }
}

fn python(args: &[String]) -> Option<Step> {
    if args.iter().any(|a| a == "-c") {
        return None;
    }
    if let Some(module) = value_of(args, &["-m"]) {
        let m_at = args.iter().position(|a| a == "-m").unwrap();
        let after = &args[m_at + 2..];
        if module == "pip" {
            return pip(after);
        }
        if module == "pytest" {
            return Some(Step::new(format!(
                "Runs this project{AP}s Python tests.",
                AP = '\u{2019}'
            )));
        }
        let folder = after.iter().find(|arg| !arg.starts_with('-'));
        if module == "venv" {
            let folder = folder?;
            return Some(Step::new(format!(
                "Makes a Python environment in the folder {folder}."
            )));
        }
        return Some(Step::new(format!("Runs the Python module {module}.")));
    }
    script(args)
}

fn script(args: &[String]) -> Option<Step> {
    if args
        .iter()
        .any(|arg| arg == "-e" || arg == "--eval" || arg == "-p" || arg == "--print")
    {
        return None;
    }
    let file = args.iter().find(|arg| !arg.starts_with('-'))?;
    if args.iter().any(|a| a == "--check" || a == "-c") {
        return Some(Step::looking(format!(
            "Checks {file} for mistakes in how it is written, without running it."
        )));
    }
    if args.iter().any(|a| a == "--test") {
        let files: Vec<String> = args
            .iter()
            .filter(|arg| !arg.starts_with('-'))
            .cloned()
            .collect();
        return Some(Step::new(format!(
            "Runs the tests in {}.",
            join(&files)
        )));
    }
    Some(Step::new(format!("Runs the script {file}.")))
}

fn cargo(args: &[String]) -> Option<Step> {
    match args.first().map(|s| s.as_str()) {
        Some("test") => Some(Step::new(format!(
            "Runs this Rust project{AP}s tests.",
            AP = '\u{2019}'
        ))),
        Some("build") => Some(Step::new("Builds this Rust project.")),
        Some("run") => Some(Step::new("Builds this Rust project and runs it.")),
        Some("check" | "clippy") => Some(Step::looking(
            "Checks this Rust project for mistakes.",
        )),
        _ => None,
    }
}

fn go_tool(args: &[String]) -> Option<Step> {
    match args.first().map(|s| s.as_str()) {
        Some("test") => Some(Step::new(format!(
            "Runs this Go project{AP}s tests.",
            AP = '\u{2019}'
        ))),
        Some("build") => Some(Step::new("Builds this Go project.")),
        Some("vet") => Some(Step::looking(
            "Checks this Go project for mistakes.",
        )),
        Some("run") => {
            let file = args.get(1)?;
            Some(Step::new(format!("Builds {file} and runs it.")))
        }
        _ => None,
    }
}

// --- files -------------------------------------------------------------------

const PS_VALUES: &[&str] = &[
    "-erroraction",
    "-ea",
    "-include",
    "-exclude",
    "-filter",
    "-itemtype",
    "-type",
    "-value",
    "-encoding",
    "-depth",
    "-first",
    "-last",
    "-tail",
    "-totalcount",
    "-head",
    "-newname",
    "-destination",
    "-path",
    "-literalpath",
    "-name",
    "-pattern",
    "-pathtype",
    "-classname",
    "-class",
    "-seconds",
    "-milliseconds",
    "-readcount",
    "-delimiter",
];

fn remove(verb: &str, args: &[String]) -> Option<Step> {
    let mut targets = values_of(args, &["-path", "-literalpath"]);
    targets.extend(plain_of(args, verb, &[]));
    let targets: Vec<String> = targets.into_iter().flat_map(|v| comma_list(&v)).collect();
    if targets.is_empty() {
        return None;
    }
    if targets
        .iter()
        .all(|t| crate::invariants::regex(r"(?i)^env:[A-Za-z_][A-Za-z0-9_]*$").is_match(t))
    {
        let names: Vec<String> = targets.iter().map(|t| t[4..].to_string()).collect();
        return Some(Step::new(format!(
            "Clears {} for the commands after it.",
            join(&names)
        )));
    }
    let deep = args.iter().any(|arg| {
        crate::invariants::regex(r"(?i)^-recurse$").is_match(arg)
            || arg == "--recursive"
            || (crate::invariants::regex(r"^-[a-zA-Z]{1,3}$").is_match(arg)
                && crate::invariants::regex(r"(?i)r").is_match(arg))
            || crate::invariants::regex(r"(?i)^/s$").is_match(arg)
    });
    if matches!(verb, "rmdir" | "rd") && !deep {
        return Some(Step::new(format!(
            "Deletes the folder{} {}, only if {} empty.",
            plural(&targets),
            join(&targets),
            if targets.len() == 1 {
                "it is"
            } else {
                "they are"
            }
        )));
    }
    if matches!(verb, "del" | "erase") && deep {
        return Some(Step::amber(format!(
            "Deletes {} in this folder and every folder inside it.{NO_BIN}",
            join(&targets)
        )));
    }
    Some(Step::amber(format!(
        "Deletes {}{}.{NO_BIN}",
        join(&targets),
        if deep {
            format!(
                " and everything inside {}",
                if targets.len() == 1 { "it" } else { "them" }
            )
        } else {
            String::new()
        }
    )))
}

fn move_cmd(verb: &str, args: &[String]) -> Option<Step> {
    let given: Vec<String> = values_of(args, &["-path", "-literalpath"])
        .into_iter()
        .flat_map(|v| comma_list(&v))
        .collect();
    let to = value_of(args, &["-destination"]).map(str::to_string);
    let plain = plain_of(args, verb, &["-t", "--target-directory"]);
    let dest = to.clone().or_else(|| {
        if !given.is_empty() {
            plain.first().cloned()
        } else if plain.len() > 1 {
            plain.last().cloned()
        } else {
            None
        }
    })?;
    let from: Vec<String> = if to.is_none() && given.is_empty() {
        let mut v = given;
        v.extend(plain[..plain.len() - 1].iter().cloned());
        v
    } else if to.is_none() {
        given
    } else {
        let mut v = given;
        v.extend(plain);
        v
    };
    if from.is_empty() {
        return None;
    }
    Some(Step::new(format!("Moves {} to {dest}.", join(&from))))
}

fn rename(args: &[String]) -> Option<Step> {
    let from = value_of(args, &["-path", "-literalpath"]).map(str::to_string);
    let to = value_of(args, &["-newname"]).map(str::to_string);
    let plain = plain_of(args, "rename", &[]);
    let one = from.clone().or_else(|| plain.first().cloned())?;
    let two = to.or_else(|| {
        if from.is_none() {
            plain.get(1).cloned()
        } else {
            plain.first().cloned()
        }
    })?;
    Some(Step::new(format!("Renames {one} to {two}.")))
}

fn copy(verb: &str, args: &[String]) -> Option<Step> {
    let given: Vec<String> = values_of(args, &["-path", "-literalpath"])
        .into_iter()
        .flat_map(|v| comma_list(&v))
        .collect();
    let to = value_of(args, &["-destination"]).map(str::to_string);
    let plain = plain_of(args, verb, &["-t", "--target-directory"]);
    let dest = to.clone().or_else(|| {
        if !given.is_empty() {
            plain.first().cloned()
        } else if plain.len() > 1 {
            plain.last().cloned()
        } else {
            None
        }
    })?;
    let from: Vec<String> = if to.is_none() && given.is_empty() {
        let mut v = given;
        v.extend(plain[..plain.len() - 1].iter().cloned());
        v
    } else if to.is_none() {
        given
    } else {
        let mut v = given;
        v.extend(plain);
        v
    };
    if from.is_empty() {
        return None;
    }
    Some(Step::new(format!("Copies {} to {dest}.", join(&from))))
}

fn robocopy(args: &[String]) -> Option<Step> {
    let plain: Vec<_> = args
        .iter()
        .filter(|arg| !arg.starts_with('/'))
        .cloned()
        .collect();
    let from = plain.first()?;
    let to = plain.get(1)?;
    if args
        .iter()
        .any(|arg| crate::invariants::regex(r"(?i)^/(mir|purge)$").is_match(arg))
    {
        return Some(Step::amber(format!(
            "Makes {to} match {from} exactly: files in {to} that {from} does not have are deleted."
        )));
    }
    if args
        .iter()
        .any(|arg| crate::invariants::regex(r"(?i)^/mov(e)?$").is_match(arg))
    {
        return Some(Step::new(format!("Moves the files in {from} to {to}.")));
    }
    Some(Step::new(format!("Copies the files in {from} to {to}.")))
}

fn new_item(args: &[String]) -> Option<Step> {
    let type_ = value_of(args, &["-itemtype", "-type"]).map(|s| s.to_lowercase());
    let mut path = values_of(args, &["-path", "-name"]);
    path.extend(plain_of(args, "new-item", &[]));
    let path: Vec<String> = path.into_iter().flat_map(|v| comma_list(&v)).collect();
    if path.is_empty() {
        return None;
    }
    if type_.as_deref() == Some("directory") {
        return Some(Step::new(format!(
            "Makes the folder{} {}.",
            plural(&path),
            join(&path)
        )));
    }
    if type_.is_some() && type_.as_deref() != Some("file") {
        return None;
    }
    let force = args
        .iter()
        .any(|arg| crate::invariants::regex(r"(?i)^-force$").is_match(arg));
    if force {
        Some(Step::amber(format!(
            "Makes the file{} {}. A file already there is replaced, and what was in it is lost.",
            plural(&path),
            join(&path)
        )))
    } else {
        Some(Step::new(format!(
            "Makes the file{} {}.",
            plural(&path),
            join(&path)
        )))
    }
}

fn listing(verb: &str, args: &[String]) -> Option<Step> {
    let mut where_ = values_of(args, &["-path", "-literalpath"]);
    where_.extend(plain_of(args, verb, &[]));
    let where_ = where_.into_iter().next();
    let deep = args.iter().any(|arg| {
        crate::invariants::regex(r"(?i)^-recurse$").is_match(arg)
            || arg == "-R"
            || crate::invariants::regex(r"(?i)^/s$").is_match(arg)
    }) || verb == "tree";
    Some(Step::looking(if verb == "tree" {
        format!(
            "Shows the folders and files in {} as a tree.",
            place(where_.as_deref())
        )
    } else {
        format!(
            "Lists the files in {}{}.",
            place(where_.as_deref()),
            if deep {
                " and every folder inside it"
            } else {
                ""
            }
        )
    }))
}

fn search(verb: &str, args: &[String]) -> Option<Step> {
    let takes_value = [
        "-e",
        "-f",
        "-A",
        "-B",
        "-C",
        "-m",
        "-g",
        "-t",
        "-T",
        "--glob",
        "--type",
        "--max-count",
        "-pattern",
        "-path",
        "-include",
        "-exclude",
        "-context",
    ];
    if verb == "rg" && args.iter().any(|a| a == "--files") {
        let where_ = plain_of(args, verb, &takes_value);
        let like = values_of(args, &["-g", "--glob", "--iglob"]);
        let named = if !like.is_empty() && like.iter().all(|g| !g.starts_with('!')) {
            if like.len() == 1 {
                format!(" named like {}", like[0])
            } else {
                let head: Vec<_> = like[..like.len() - 1]
                    .iter()
                    .map(|one| cut(one, 40))
                    .collect();
                format!(
                    " named like {} or {}",
                    head.join(", "),
                    cut(&like[like.len() - 1], 40)
                )
            }
        } else {
            String::new()
        };
        return Some(Step::looking(format!(
            "Lists the files in {}{named}.",
            if where_.is_empty() {
                "this folder".into()
            } else {
                join(&where_.iter().map(|w| place(Some(w))).collect::<Vec<_>>())
            }
        )));
    }
    let pattern = value_of(args, &["-e", "--regexp", "-pattern"]).map(str::to_string);
    let plain = plain_of(args, verb, &takes_value);
    let what = pattern
        .clone()
        .or_else(|| plain.first().cloned())?;
    let mut where_ = values_of(args, &["-path", "-literalpath"]);
    if pattern.is_none() {
        where_.extend(plain.into_iter().skip(1));
    } else {
        where_.extend(plain);
    }
    Some(Step::looking(format!(
        "Searches {} for {LDQ}{}{RDQ}.",
        if where_.is_empty() {
            "this folder".into()
        } else {
            join(
                &where_
                    .iter()
                    .map(|w| place(Some(w)))
                    .collect::<Vec<_>>(),
            )
        },
        cut(&what, 60),
        LDQ = '\u{201c}',
        RDQ = '\u{201d}'
    )))
}

fn opens(args: &[String]) -> Option<Step> {
    let target = value_of(args, &["-filepath"])
        .map(str::to_string)
        .or_else(|| {
            plain_of(args, "start", &["-argumentlist", "-a"])
                .into_iter()
                .find(|arg| !arg.is_empty())
        })?;
    if crate::invariants::regex(r"(?i)^https?://").is_match(&target) {
        Some(Step::new(format!(
            "Opens {} in your web browser.",
            host_of(&target)
        )))
    } else {
        None
    }
}

fn waits(args: &[String]) -> Option<Step> {
    if let Some(ms) = value_of(args, &["-milliseconds", "-m"]) {
        return if crate::invariants::regex(r"^\d+$").is_match(ms) {
            Some(Step::looking("Waits a moment."))
        } else {
            None
        };
    }
    let given = value_of(args, &["-seconds", "-s"])
        .map(str::to_string)
        .or_else(|| {
            args.iter()
                .find(|arg| !arg.starts_with('-'))
                .cloned()
        })?;
    let time = crate::invariants::regex(r"^(\d+(?:\.\d+)?)([smhd]?)$").captures(&given)?;
    let n: f64 = time[1].parse().ok()?;
    let mult = match &time[2] {
        "" | "s" => 1.0,
        "m" => 60.0,
        "h" => 3600.0,
        "d" => 86400.0,
        _ => 1.0,
    };
    let seconds = n * mult;
    if seconds < 120.0 {
        let s = seconds.round() as i64;
        Some(Step::looking(format!(
            "Waits {s} second{}.",
            if s == 1 { "" } else { "s" }
        )))
    } else if seconds < 7200.0 {
        Some(Step::looking(format!(
            "Waits about {} minutes.",
            (seconds / 60.0).round() as i64
        )))
    } else {
        Some(Step::looking(format!(
            "Waits about {} hours.",
            (seconds / 3600.0).round() as i64
        )))
    }
}

fn fetches(verb: &str, args: &[String]) -> Option<Step> {
    let url = value_of(args, &["-uri"])
        .map(str::to_string)
        .or_else(|| {
            args.iter()
                .find(|arg| crate::invariants::regex(r"(?i)^https?://").is_match(arg))
                .cloned()
        })?;
    if !crate::invariants::regex(r"(?i)^https?://").is_match(&url) {
        return None;
    }
    let host = host_of(&url);
    let method = value_of(args, &["-X", "--request", "-method"])
        .unwrap_or("")
        .to_uppercase();
    let sends = method == "POST"
        || method == "PUT"
        || method == "PATCH"
        || args.iter().any(|arg| {
            crate::invariants::regex(r"^(-d|--data.*|-F|--form|-T|--upload-file|--json)$")
                .is_match(arg)
                || crate::invariants::regex(r"(?i)^-(body|infile)$").is_match(arg)
        });
    if method == "DELETE" {
        return Some(Step::amber(format!("Asks {host} to delete something.")));
    }
    if sends {
        return Some(Step::new(format!("Sends data to {host}.")));
    }
    let mut out_names = vec!["-o", "--output", "-outfile"];
    if verb == "wget" {
        out_names.extend(["-O", "--output-document"]);
    }
    let out = value_of(args, &out_names);
    if let Some(out) = out
        && out != "-" {
            return Some(Step::new(format!(
                "Downloads {} into {out}.",
                short_url(&url)
            )));
        }
    if (args.iter().any(|a| a == "-O") && verb == "curl")
        || args.iter().any(|a| a == "--remote-name")
    {
        return Some(Step::new(format!(
            "Downloads {} into this folder.",
            short_url(&url)
        )));
    }
    if verb == "wget" && out.is_none() {
        return Some(Step::new(format!(
            "Downloads {} into this folder.",
            short_url(&url)
        )));
    }
    Some(Step::new(format!(
        "Fetches {} and shows what comes back.",
        short_url(&url)
    )))
}

// --- reading the words --------------------------------------------------------

fn redirects(text: &str) -> Option<Vec<Saved>> {
    let mut found = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut quote = None;
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if quote.is_some() {
            if Some(ch) == quote {
                quote = None;
            }
            i += 1;
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            i += 1;
            continue;
        }
        if ch != '>' {
            i += 1;
            continue;
        }
        let errors = i > 0 && chars[i - 1] == '2';
        let adds = i + 1 < chars.len() && chars[i + 1] == '>';
        let mut at = i + 1;
        while at < chars.len() && (chars[at] == '>' || chars[at] == ' ') {
            at += 1;
        }
        if at >= chars.len() {
            break;
        }
        let opener = chars[at];
        let target: String = if opener == '"' || opener == '\'' {
            let end = chars[at + 1..]
                .iter()
                .position(|&c| c == opener)
                .map(|p| at + 1 + p)
                .unwrap_or(chars.len() - 1);
            chars[at..=end.min(chars.len() - 1)].iter().collect()
        } else {
            let rest: String = chars[at..].iter().collect();
            crate::invariants::regex(r"^(?:&\d?|[^\s;|&]*)")
                .find(&rest)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default()
        };
        i = at + target.chars().count() - 1;
        let file = crate::invariants::regex(r#"^(["'])(.*)\1$"#)
            .replace(&target, "$2")
            .into_owned();
        if crate::invariants::regex(r"(?i)^(&\d?|/dev/null|\$null|nul)$").is_match(&file) {
            i += 1;
            continue;
        }
        if file.is_empty() || crate::invariants::regex(r"[$`(){}@*?]").is_match(&file) {
            return None;
        }
        found.push(Saved {
            file,
            adds,
            errors,
        });
        i += 1;
    }
    Some(found)
}

fn without_redirects(words: &[String]) -> Vec<String> {
    let mut kept = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let word = &words[i];
        if crate::invariants::regex(r"^(\d|\*|&)?>>?$").is_match(word) {
            i += 2;
            continue;
        }
        if crate::invariants::regex(r"^(\d|\*|&)?>").is_match(word) {
            i += 1;
            continue;
        }
        kept.push(word.clone());
        i += 1;
    }
    kept
}

fn plain_of(args: &[String], verb: &str, takes_value: &[&str]) -> Vec<String> {
    let windows = matches!(
        verb,
        "del"
            | "erase"
            | "rd"
            | "rmdir"
            | "copy"
            | "move"
            | "xcopy"
            | "type"
            | "dir"
            | "findstr"
            | "md"
            | "mkdir"
            | "tree"
            | "more"
            | "ren"
            | "rename"
            | "start"
            | "where"
            | "cd"
            | "chdir"
            | "pushd"
    );
    let mut found = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--" {
            found.extend(args[i + 1..].iter().cloned());
            break;
        }
        if takes_value.iter().any(|t| t == arg)
            || PS_VALUES.iter().any(|p| p.eq_ignore_ascii_case(arg))
        {
            i += 2;
            continue;
        }
        if arg.starts_with('-') && arg.len() > 1 {
            i += 1;
            continue;
        }
        if windows && crate::invariants::regex(r"(?i)^/[a-z?][a-z:]{0,3}$").is_match(arg) {
            i += 1;
            continue;
        }
        found.push(arg.clone());
        i += 1;
    }
    found
}

fn value_of<'a>(args: &'a [String], names: &[&str]) -> Option<&'a str> {
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        let lower = arg.to_lowercase();
        let matched = names.iter().any(|one| {
            *one == arg
                || (one.eq_ignore_ascii_case(&lower)
                    && one.starts_with('-')
                    && !one.starts_with("--")
                    && one.len() > 2)
        });
        if matched {
            return args.get(i + 1).map(|s| s.as_str());
        }
        if let Some(long) = names
            .iter()
            .find(|one| one.starts_with("--") && arg.starts_with(&format!("{one}=")))
        {
            return Some(&arg[long.len() + 1..]);
        }
        i += 1;
    }
    None
}

fn values_of(args: &[String], names: &[&str]) -> Vec<String> {
    let mut found = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        let lower = arg.to_lowercase();
        let name = names.iter().find(|one| {
            *one == arg
                || (one.eq_ignore_ascii_case(&lower)
                    && one.starts_with('-')
                    && !one.starts_with("--")
                    && one.len() > 2)
        });
        if name.is_some()
            && let Some(next) = args.get(i + 1) {
                found.push(next.clone());
                i += 2;
                continue;
            }
        if let Some(long) = names
            .iter()
            .find(|one| one.starts_with("--") && arg.starts_with(&format!("{one}=")))
        {
            found.push(arg[long.len() + 1..].to_string());
        }
        i += 1;
    }
    found
}

fn comma_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|one| one.trim().to_string())
        .filter(|one| !one.is_empty())
        .collect()
}

fn named(list: &[String], say: impl Fn(&[String]) -> String) -> Option<Step> {
    if list.is_empty() {
        None
    } else {
        Some(Step::new(say(list)))
    }
}

fn looks(step: Option<Step>) -> Option<Step> {
    step.map(|s| Step {
        looks: true,
        ..s
    })
}

fn looks_like_a_file(word: &str) -> bool {
    word == "."
        || (crate::invariants::regex(r"(?i)\.[a-z0-9]{1,6}$").is_match(word)
            && !crate::invariants::regex(r"^\.\.?$").is_match(word))
}

fn place(path: Option<&str>) -> String {
    match path {
        None | Some(".") | Some("./") | Some(".\\") => "this folder".into(),
        Some(p) => p.to_string(),
    }
}

fn everything(files: &[String]) -> String {
    if files.len() == 1 && files[0] == "." {
        "every file in this folder".into()
    } else {
        join(files)
    }
}

fn join(list: &[String]) -> String {
    let shown: Vec<String> = list.iter().map(|one| cut(one, 80)).collect();
    match shown.len() {
        0 => String::new(),
        1 => shown[0].clone(),
        2 | 3 => format!(
            "{} and {}",
            shown[..shown.len() - 1].join(", "),
            shown[shown.len() - 1]
        ),
        _ => format!(
            "{}, and {} more",
            shown[..2].join(", "),
            shown.len() - 2
        ),
    }
}

fn plural(list: &[String]) -> &'static str {
    if list.len() == 1 { "" } else { "s" }
}

fn cut(text: &str, most: usize) -> String {
    if text.len() > most {
        format!(
            "{}{ELL}",
            text[..most - 1].trim_end(),
            ELL = '\u{2026}'
        )
    } else {
        text.to_string()
    }
}

fn lower_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => format!("{}{}", c.to_lowercase(), chars.as_str()),
    }
}

fn host_of(url: &str) -> String {
    parse_url_host_path(url)
        .map(|(host, _)| host)
        .unwrap_or_else(|| cut(url, 60))
}

fn short_url(url: &str) -> String {
    match parse_url_host_path(url) {
        Some((host, path)) => {
            let path = if path == "/" { "" } else { path.as_str() };
            cut(&format!("{host}{path}"), 70)
        }
        None => cut(url, 70),
    }
}

/// Minimal `https://host/path` parse (no `url` crate dependency).
fn parse_url_host_path(url: &str) -> Option<(String, String)> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .or_else(|| url.strip_prefix("HTTPS://"))
        .or_else(|| url.strip_prefix("HTTP://"))?;
    let end = rest
        .find(['/', '?', '#', ' ', '\'', '"'])
        .unwrap_or(rest.len());
    let host = rest[..end].to_string();
    if host.is_empty() {
        return None;
    }
    let path = if end < rest.len() && rest[end..].starts_with('/') {
        let path_end = rest[end..]
            .find(['?', '#', ' ', '\'', '"'])
            .map(|i| end + i)
            .unwrap_or(rest.len());
        rest[end..path_end].to_string()
    } else {
        "/".into()
    };
    Some((host, path))
}

fn repository_of(from: &str) -> String {
    if let Some(c) =
        crate::invariants::regex(r"(?i)github\.com[/:]([^/\s]+/[^/\s]+?)(?:\.git)?/?$").captures(from)
    {
        return format!("{} from GitHub", &c[1]);
    }
    if crate::invariants::regex(r"(?i)^https?://").is_match(from) {
        format!("at {}", short_url(from))
    } else {
        from.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn said(command: &str) -> Option<String> {
        plain_command(command).map(|p| p.said)
    }

    #[test]
    fn push_force_and_delete() {
        assert_eq!(
            said("git push origin main").as_deref(),
            Some("Sends the commits on main to origin.")
        );
        assert_eq!(
            said("git push").as_deref(),
            Some("Sends this branch\u{2019}s new commits to the repository this branch is linked to.")
        );
        let forced = plain_command("git push --force origin main").unwrap();
        assert_eq!(
            forced.said,
            "Replaces main on origin with this computer\u{2019}s version: commits there that are not on this computer are lost."
        );
        assert!(forced.careful);
        assert_eq!(said("git push origin +main"), Some(forced.said.clone()));
        assert_eq!(
            said("git push -f").as_deref(),
            Some("Replaces this branch on the repository this branch is linked to with this computer\u{2019}s version: commits there that are not on this computer are lost.")
        );
        assert!(
            said("git push --force-with-lease origin main")
                .unwrap()
                .contains("unless someone else changed it there since you last fetched")
        );
        assert_eq!(
            said("git push origin --delete old-branch").as_deref(),
            Some("Deletes the branch old-branch on origin.")
        );
    }

    #[test]
    fn delete_skips_recycle_bin() {
        let gone = plain_command("rm -rf node_modules dist").unwrap();
        assert_eq!(
            gone.said,
            "Deletes node_modules and dist and everything inside them. Nothing goes to the Recycle Bin or Trash."
        );
        assert!(gone.careful);
        assert_eq!(
            said("Remove-Item notes.txt -Force").as_deref(),
            Some("Deletes notes.txt. Nothing goes to the Recycle Bin or Trash.")
        );
    }

    #[test]
    fn download_run_unseen() {
        let piped = plain_command("curl -fsSL https://get.example.dev/install.sh | sh").unwrap();
        assert_eq!(
            piped.said,
            "Downloads a script from get.example.dev and runs it straight away, without showing it to you first."
        );
        assert!(piped.careful);
        assert!(!piped.only_looks);
        assert_eq!(
            said("irm https://get.example.dev/install.ps1 | iex"),
            Some(piped.said.clone())
        );
        assert_eq!(
            said("bash -c \"$(curl -fsSL https://get.example.dev/install.sh)\""),
            Some(piped.said.clone())
        );
    }

    #[test]
    fn looking_only() {
        let looks = plain_command("git status && git diff").unwrap();
        assert_eq!(
            looks,
            PlainCommand {
                said: "Shows which files have changed. Shows what has changed in the files. It only looks."
                    .into(),
                careful: false,
                only_looks: true,
            }
        );
        assert_eq!(
            said("git log --oneline -20 | head -5").as_deref(),
            Some("Lists recent commits. It only looks.")
        );
        assert_eq!(
            said("rg -n \"cartTotal\" src").as_deref(),
            Some("Searches src for \u{201c}cartTotal\u{201d}. It only looks.")
        );
    }

    #[test]
    fn silence_when_unread() {
        assert!(plain_command("npm test && ./deploy.sh").is_none());
        assert!(plain_command("git status; terraform apply").is_none());
        assert!(plain_command("rm $(git ls-files --deleted)").is_none());
        assert!(plain_command("command -v git").is_none());
        assert!(plain_command("type git").is_none());
        assert!(plain_command("git frobnicate").is_none());
        assert!(plain_command("").is_none());
        assert!(plain_command("./build.sh").is_none());
        assert!(plain_command("node -e \"require('fs').rmSync('x')\"").is_none());
        assert!(plain_command("python -c \"print(1)\"").is_none());
        assert!(plain_command("git ls-files | xargs rm").is_none());
        assert!(plain_command(
            "git add -A && git commit -m x && git tag v2 && git push && git push --tags"
        )
        .is_none());
    }

    #[test]
    fn throws_work_away() {
        let hard = plain_command("git reset --hard origin/main").unwrap();
        assert_eq!(
            hard.said,
            "Throws away every change not committed, in every file Git tracks, and moves this branch back to origin/main. It cannot be undone."
        );
        assert!(hard.careful);
        assert_eq!(
            said("git checkout -- src/cart.ts").as_deref(),
            Some("Throws away the changes to src/cart.ts that are not committed. They cannot be brought back.")
        );
        assert_eq!(
            said("git restore .").as_deref(),
            Some("Throws away the changes to every file in this folder that are not committed. They cannot be brought back.")
        );
        assert_eq!(
            said("git branch -D spike").as_deref(),
            Some("Deletes the branch spike, even if its commits are on no other branch: those commits can be lost.")
        );
        assert!(!plain_command("git checkout main").unwrap().careful);
        assert_eq!(
            said("git checkout -b fix/cart").as_deref(),
            Some("Makes a new branch, fix/cart, and switches to it.")
        );
        assert_eq!(
            said("git checkout HEAD~3").as_deref(),
            Some("Puts the files as they were at HEAD~3, outside any branch.")
        );
    }

    #[test]
    fn installs_and_gh() {
        assert!(
            said("npm install")
                .unwrap()
                .contains("A package can run its own setup code as it installs.")
        );
        assert_eq!(
            said("gh pr create --title \"Fix cart\" --body \"...\"").as_deref(),
            Some("Opens a pull request on GitHub.")
        );
        assert_eq!(
            said("gh pr merge 42 --squash").as_deref(),
            Some("Merges the pull request #42 on GitHub.")
        );
        assert_eq!(
            said("gh pr view 42").as_deref(),
            Some("Reads the pull request #42 on GitHub. It only looks.")
        );
        assert_eq!(
            said("npx prettier --check .").as_deref(),
            Some("Runs prettier, downloading it first if this project does not have it.")
        );
    }

    #[test]
    fn shells_unwrap() {
        assert_eq!(
            said(r#""C:\WINDOWS\System32\WindowsPowerShell\v1.0\powershell.exe" -Command "git status""#).as_deref(),
            Some("Shows which files have changed. It only looks.")
        );
        assert_eq!(
            said("bash -lc 'git push origin main'").as_deref(),
            Some("Sends the commits on main to origin.")
        );
    }
}
