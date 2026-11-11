//! Whether a shell command acts on programs it did not start (0.578).
//!
//! Faithful port of `apps/desktop/src/shared/command-reach.ts`.

use std::sync::LazyLock;

use regex::Regex;

const NOT_ONLY: &str = "not only the ones this run started";

/// Kind of reach beyond programs this run started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandReachKind {
    EveryProcessNamed,
    EveryProcessOfUser,
    EveryProcessOnPort,
    WholeMachine,
}

impl CommandReachKind {
    #[cfg(any(test, feature = "test-hooks"))]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EveryProcessNamed => "every-process-named",
            Self::EveryProcessOfUser => "every-process-of-user",
            Self::EveryProcessOnPort => "every-process-on-port",
            Self::WholeMachine => "whole-machine",
        }
    }
}

/// A named reach: kind, target, badge short, and one sentence for a person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandReach {
    pub kind: CommandReachKind,
    /// The name, user or port as written, lower-cased (`.exe` kept if given).
    pub target: String,
    /// A few words for a badge: "stops every python.exe".
    pub short: String,
    /// One sentence for a person.
    pub said: String,
}

/// Whether a shell command acts on programs it did not start.
pub fn command_reach(command: Option<&str>) -> Option<CommandReach> {
    // A row whose runtime never sent the command has nothing to read.
    reach_in(command?, 0)
}

fn reach_in(command: &str, depth: u32) -> Option<CommandReach> {
    if depth > 4 {
        return None;
    }
    let parts = split_chain(command);
    for i in 0..parts.len() {
        // A group, a substitution or a backtick runs the commands inside it.
        for inner in inner_commands(&parts[i].text) {
            if let Some(found) = reach_in(&inner, depth + 1) {
                return Some(found);
            }
        }
        let words = command_words(&tokens(&parts[i].text));
        if words.is_empty() {
            continue;
        }
        if let Some(inner) = wrapped(&words) {
            if let Some(found) = reach_in(&inner, depth + 1) {
                return Some(found);
            }
            continue;
        }
        // The parts piped into this one, nearest first.
        let mut piped: Vec<Vec<String>> = Vec::new();
        let mut j = i;
        while j > 0 && parts[j].after == "|" {
            piped.push(command_words(&tokens(&parts[j - 1].text)));
            j -= 1;
        }
        if let Some(found) = reach_of(&words, &piped) {
            return Some(found);
        }
    }
    None
}

// --- the commands ---------------------------------------------------------

fn reach_of(words: &[String], piped: &[Vec<String>]) -> Option<CommandReach> {
    let verb = program_name(&words[0]);
    let args = &words[1..];
    match verb.as_str() {
        "taskkill" => taskkill(args),
        "stop-process" | "spps" => stop_process(args, piped),
        "kill" => {
            // PowerShell's `kill` is Stop-Process; a POSIX `kill` takes numbers.
            if args
                .iter()
                .any(|arg| crate::invariants::regex(r"(?i)^-(name|processname|id)$").is_match(arg))
            {
                stop_process(args, piped)
            } else {
                posix_kill(args, piped)
            }
        }
        "pkill" => pkill(args),
        "killall" => killall(args),
        "wmic" => wmic(args),
        "xargs" => xargs_kill(args, piped),
        "fuser" => {
            let has_k = args
                .iter()
                .any(|arg| crate::invariants::regex(r"(?i)^-[a-z]*k").is_match(arg));
            if !has_k {
                return None;
            }
            let port_re = crate::invariants::regex(r"(?i)^\d+(/(tcp|udp))?$");
            let port = args.iter().find(|arg| port_re.is_match(arg)).map(|arg| {
                crate::invariants::regex(r"/.*")
                    .replace(arg, "")
                    .into_owned()
            });
            on_port(port.as_deref())
        }
        "kill-port" => {
            let digits = crate::invariants::regex(r"^\d+$");
            on_port(
                args.iter()
                    .find(|arg| digits.is_match(arg))
                    .map(|s| s.as_str()),
            )
        }
        "npx" => {
            if args.first().is_some_and(|a| program_name(a) == "kill-port") {
                reach_of(args, piped)
            } else {
                None
            }
        }
        "shutdown" => {
            let abort = crate::invariants::regex(r"(?i)^[/-]a$");
            if args.iter().any(|arg| abort.is_match(arg) || arg == "-c") {
                return None;
            }
            // Windows' `/l` signs the person out; `/h` hibernates. POSIX `-h` is halt.
            if args
                .iter()
                .any(|arg| crate::invariants::regex(r"(?i)^/l$").is_match(arg))
            {
                return Some(CommandReach {
                    kind: CommandReachKind::EveryProcessOfUser,
                    target: "you".into(),
                    short: "signs you out".into(),
                    said: "Signs you out of Windows, closing every program you are running.".into(),
                });
            }
            if args
                .iter()
                .any(|arg| crate::invariants::regex(r"(?i)^/h$").is_match(arg))
            {
                return None;
            }
            let restart = args
                .iter()
                .any(|arg| crate::invariants::regex(r"(?i)^([/-]r|--reboot|-g)$").is_match(arg));
            Some(machine(restart))
        }
        "tskill" => {
            // Windows' older kill: a process id, or every process of that name.
            let target = args
                .iter()
                .find(|arg| !arg.starts_with('/') && !arg.starts_with('-'));
            match target {
                None => None,
                Some(t) if crate::invariants::regex(r"^\d+$").is_match(t) => None,
                Some(t) => Some(named(t)),
            }
        }
        "invoke-cimmethod" => {
            // `Get-CimInstance Win32_Process … | Invoke-CimMethod -MethodName Terminate`
            let method = crate::invariants::regex(r"(?i)^-methodname$");
            let terminate = crate::invariants::regex(r"(?i)^terminate$");
            if args.iter().enumerate().any(|(i, arg)| {
                method.is_match(arg) && args.get(i + 1).is_some_and(|next| terminate.is_match(next))
            }) {
                from_pipeline(piped)
            } else {
                None
            }
        }
        "foreach-object" | "%" => {
            // `Get-WmiObject Win32_Process … | % { $_.Terminate() }`
            if crate::invariants::regex(r"(?i)\.terminate\(").is_match(&args.join(" ")) {
                from_pipeline(piped)
            } else {
                None
            }
        }
        "restart-computer" | "reboot" => Some(machine(true)),
        "stop-computer" | "poweroff" | "halt" => Some(machine(false)),
        "systemctl" => {
            if args.iter().any(|a| a == "reboot") {
                Some(machine(true))
            } else if args.iter().any(|a| a == "poweroff" || a == "halt") {
                Some(machine(false))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn taskkill(args: &[String]) -> Option<CommandReach> {
    // TS: /^(\/\/?|-)im$/i after toLowerCase — same as /{1,2} or -
    let im = crate::invariants::regex(r"^(/{1,2}|-)im$");
    let fi = crate::invariants::regex(r"^(/{1,2}|-)fi$");
    let imagename = crate::invariants::regex(r"(?i)imagename\s+eq\s+(\S+)");
    let username = crate::invariants::regex(r"(?i)username\s+eq\s+(\S+)");
    for i in 0..args.len() {
        let flag = args[i].to_lowercase();
        if im.is_match(&flag)
            && let Some(next) = args.get(i + 1)
        {
            return Some(named(next));
        }
        if fi.is_match(&flag)
            && let Some(next) = args.get(i + 1)
        {
            if let Some(caps) = imagename.captures(next) {
                return Some(named(caps.get(1)?.as_str()));
            }
            if let Some(caps) = username.captures(next) {
                return Some(of_user(caps.get(1)?.as_str()));
            }
        }
    }
    None
}

fn stop_process(args: &[String], piped: &[Vec<String>]) -> Option<CommandReach> {
    for i in 0..args.len() {
        let flag = args[i].to_lowercase();
        if (flag == "-name" || flag == "-processname")
            && let Some(next) = args.get(i + 1)
        {
            let first = next.split(',').next().unwrap_or(next);
            return Some(named(first));
        }
        if (flag == "-id" || flag == "-pid") && args.get(i + 1).is_some() {
            // A number is one program; an expression that looks programs up is not.
            return looked_up(&args[i + 1..].join(" "));
        }
    }
    if args
        .iter()
        .any(|arg| crate::invariants::regex(r"^\d+$").is_match(arg))
    {
        return None;
    }
    // `... | Stop-Process`: whatever the pipeline found.
    from_pipeline(piped)
}

fn posix_kill(args: &[String], piped: &[Vec<String>]) -> Option<CommandReach> {
    // `kill -1`, `kill -9 -1`, `kill -- -1`: pid -1 is every program the user may signal.
    if let Some(minus_one) = args.iter().position(|a| a == "-1")
        && minus_one == args.len() - 1
    {
        return Some(of_user("you"));
    }
    let joined = args.join(" ");
    if crate::invariants::regex(r"\$\(|`").is_match(&joined) {
        return looked_up(&joined);
    }
    if args.is_empty() {
        return from_pipeline(piped);
    }
    None
}

fn pkill(args: &[String]) -> Option<CommandReach> {
    let user = option_value(args, &["-u", "-U", "--euid", "--uid"]);
    let full_re = crate::invariants::regex(r"(?i)^-[a-z]*f[a-z]*$");
    let digit_re = crate::invariants::regex(r"^-\d");
    let full = args.iter().any(|arg| {
        arg == "-f" || arg == "--full" || (full_re.is_match(arg) && !digit_re.is_match(arg))
    });
    let pattern = last_plain(
        args,
        &[
            "-u", "-U", "--euid", "--uid", "-s", "--signal", "-g", "-P", "-t",
        ],
    );
    if let Some(pattern) = pattern {
        return Some(if full {
            matching(pattern)
        } else {
            named(pattern)
        });
    }
    user.map(of_user)
}

fn killall(args: &[String]) -> Option<CommandReach> {
    let user = option_value(args, &["-u", "--user"]);
    let name = last_plain(args, &["-u", "--user", "-s", "--signal"]);
    if let Some(name) = name {
        return Some(named(name));
    }
    user.map(of_user)
}

fn wmic(args: &[String]) -> Option<CommandReach> {
    let text = args.join(" ");
    if !crate::invariants::regex(r"(?i)^process\b").is_match(&text)
        || !crate::invariants::regex(r"(?i)\b(delete|terminate)\b").is_match(&text)
    {
        return None;
    }
    let name_re = crate::invariants::regex(r#"(?i)name\s*=\s*['"]?([^'"\s)]+)"#);
    if let Some(caps) = name_re.captures(&text) {
        return Some(named(caps.get(1)?.as_str()));
    }
    if crate::invariants::regex(r"(?i)processid\s*=").is_match(&text) {
        return None;
    }
    Some(named("*"))
}

fn xargs_kill(args: &[String], piped: &[Vec<String>]) -> Option<CommandReach> {
    let at = args.iter().position(|arg| {
        let name = program_name(arg);
        name == "kill" || name == "taskkill"
    });
    at?;
    from_pipeline(piped)
}

// --- what a pipeline or an expression looked up ----------------------------

/// lsof's `-i:3000`, `-ti tcp:5173`, `-t -i :8080`.
static PORT_FLAG: LazyLock<Regex> =
    LazyLock::new(|| crate::invariants::regex(r"(?i)-[a-z]*i\s*(?:tcp|udp)?\s*:(\d+)"));

fn from_pipeline(piped: &[Vec<String>]) -> Option<CommandReach> {
    let name_flag = crate::invariants::regex(r"(?i)^-(name|processname)$");
    let ps_list = crate::invariants::regex(r"(?i)^(aux|-ef|-e|-a)$");
    let win32 = crate::invariants::regex(r"(?i)^win32_process$");
    let processid = crate::invariants::regex(r"(?i)processid\s*=");
    let cim_name = crate::invariants::regex(r#"(?i)\bname\s*=\s*['"]?([^'"\s]+)"#);
    let where_name = crate::invariants::regex(
        r#"(?i)\.(?:process)?name\s+-(?:eq|like|match)\s+['"]?([^'"\s}]+)"#,
    );
    let invert_v = crate::invariants::regex(r"^-[a-su-z]*v[a-z]*$");
    let invert_slash_v = crate::invariants::regex(r"(?i)^/v$");
    let invert_not = crate::invariants::regex(r"(?i)^-notmatch$");

    for words in piped {
        if words.is_empty() {
            continue;
        }
        let verb = program_name(&words[0]);
        let args = &words[1..];
        let is_ps = verb == "ps"
            && args
                .iter()
                .any(|arg| name_flag.is_match(arg) || !arg.starts_with('-'))
            && !args.iter().any(|arg| ps_list.is_match(arg));
        if verb == "get-process" || verb == "gps" || is_ps {
            let name = option_value(args, &["-Name", "-name", "-ProcessName", "-processname"])
                .or_else(|| {
                    args.iter()
                        .find(|arg| !arg.starts_with('-'))
                        .map(|s| s.as_str())
                });
            if let Some(name) = name {
                let first = name.split(',').next().unwrap_or(name);
                return Some(named(first));
            }
            continue;
        }
        if verb == "pgrep" || verb == "pidof" {
            let full = verb == "pgrep" && args.iter().any(|arg| arg == "-f" || arg == "--full");
            let pattern = last_plain(args, &["-u", "-U", "-g", "-P", "-t"]);
            if let Some(pattern) = pattern {
                return Some(if full {
                    matching(pattern)
                } else {
                    named(pattern)
                });
            }
        }
        if verb == "grep" || verb == "findstr" || verb == "select-string" {
            // An inverted match names what is left OUT, not what is stopped (0.589).
            let inverted = args.iter().any(|arg| {
                arg == "--invert-match"
                    || invert_v.is_match(arg)
                    || invert_slash_v.is_match(arg)
                    || invert_not.is_match(arg)
            });
            if inverted {
                continue;
            }
            if let Some(pattern) = args
                .iter()
                .find(|arg| !arg.starts_with('-') && !arg.starts_with('/'))
                && pattern.as_str() != "grep"
            {
                return Some(matching(pattern));
            }
        }
        if verb == "where-object" || verb == "where" || verb == "?" {
            let joined = args.join(" ");
            if let Some(caps) = where_name.captures(&joined) {
                return Some(named(caps.get(1)?.as_str()));
            }
        }
        // CIM and WMI: a process id names one program.
        if (verb == "get-ciminstance"
            || verb == "gcim"
            || verb == "get-wmiobject"
            || verb == "gwmi")
            && args.iter().any(|arg| win32.is_match(arg))
        {
            let filter = args.join(" ");
            if processid.is_match(&filter) {
                continue;
            }
            if let Some(caps) = cim_name.captures(&filter) {
                return Some(named(caps.get(1)?.as_str()));
            }
            continue;
        }
        if verb == "lsof" {
            let joined = args.join(" ");
            if let Some(caps) = PORT_FLAG.captures(&joined) {
                return on_port(Some(caps.get(1)?.as_str()));
            }
        }
        if verb == "get-nettcpconnection" {
            return on_port(option_value(args, &["-LocalPort", "-localport"]));
        }
    }
    None
}

fn looked_up(expression: &str) -> Option<CommandReach> {
    let nettcp = crate::invariants::regex(r"(?i)get-nettcpconnection[^)]*-localport\s+(\d+)");
    let port = PORT_FLAG
        .captures(expression)
        .or_else(|| nettcp.captures(expression));
    if let Some(caps) = port {
        return on_port(Some(caps.get(1)?.as_str()));
    }
    let process = crate::invariants::regex(
        r#"(?i)get-process\s+(?:-(?:process)?name\s+)?['"]?([^'"\s).|]+)"#,
    );
    if let Some(caps) = process.captures(expression) {
        let name = caps.get(1)?.as_str();
        if !name.starts_with('-') {
            return Some(named(name));
        }
    }
    let pgrep =
        crate::invariants::regex(r#"(?i)\b(pgrep|pidof)\s+((?:-\S+\s+)*)['"]?([^'"\s)`]+)"#);
    if let Some(caps) = pgrep.captures(expression) {
        let flags = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        let pattern = caps.get(3)?.as_str();
        return Some(if crate::invariants::regex(r"(^|\s)-f\b").is_match(flags) {
            matching(pattern)
        } else {
            named(pattern)
        });
    }
    None
}

// --- the sentences --------------------------------------------------------

fn named(raw: &str) -> CommandReach {
    let target = unquote(raw).to_lowercase();
    if target == "*" || target.contains('*') {
        CommandReach {
            kind: CommandReachKind::EveryProcessNamed,
            short: format!("stops every {target}"),
            said: format!("Stops every program named like {target} on this computer, {NOT_ONLY}."),
            target,
        }
    } else {
        CommandReach {
            kind: CommandReachKind::EveryProcessNamed,
            short: format!("stops every {target}"),
            said: format!("Stops every {target} on this computer, {NOT_ONLY}."),
            target,
        }
    }
}

fn matching(raw: &str) -> CommandReach {
    let target = unquote(raw).to_lowercase();
    CommandReach {
        kind: CommandReachKind::EveryProcessNamed,
        short: format!("stops every match of {target}"),
        said: format!(
            "Stops every program on this computer whose command line matches \"{target}\", {NOT_ONLY}."
        ),
        target,
    }
}

fn of_user(raw: &str) -> CommandReach {
    let target = unquote(raw).to_lowercase();
    if target == "you" {
        CommandReach {
            kind: CommandReachKind::EveryProcessOfUser,
            target,
            short: "stops all your programs".into(),
            said: format!("Stops every program you are running on this computer, {NOT_ONLY}."),
        }
    } else {
        CommandReach {
            kind: CommandReachKind::EveryProcessOfUser,
            short: format!("stops all of {target}'s programs"),
            said: format!("Stops every program {target} is running on this computer, {NOT_ONLY}."),
            target,
        }
    }
}

fn on_port(port: Option<&str>) -> Option<CommandReach> {
    let port = port?;
    Some(CommandReach {
        kind: CommandReachKind::EveryProcessOnPort,
        target: port.to_string(),
        short: format!("stops whatever uses port {port}"),
        said: format!(
            "Stops whatever program is using port {port}, which may be one this run did not start."
        ),
    })
}

fn machine(restart: bool) -> CommandReach {
    if restart {
        CommandReach {
            kind: CommandReachKind::WholeMachine,
            target: "restart".into(),
            short: "restarts the computer".into(),
            said: "Restarts this computer, closing every program on it.".into(),
        }
    } else {
        CommandReach {
            kind: CommandReachKind::WholeMachine,
            target: "shut down".into(),
            short: "shuts the computer down".into(),
            said: "Shuts this computer down, closing every program on it.".into(),
        }
    }
}

// --- reading a command line -----------------------------------------------

pub struct Part {
    pub text: String,
    /// The operator before this part (`|` for a pipe), or "" for the first.
    pub after: String,
}

/// Splits on `&&`, `||`, `;`, `&`, `|` and newlines, outside quotes.
pub fn split_chain(command: &str) -> Vec<Part> {
    let chars: Vec<char> = command.chars().collect();
    let mut parts: Vec<Part> = Vec::new();
    let mut current = String::new();
    let mut after = String::new();
    let mut quote: Option<char> = None;
    let mut depth: i32 = 0;
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if quote.is_some() {
            current.push(ch);
            if Some(ch) == quote {
                quote = None;
            }
            i += 1;
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            current.push(ch);
            i += 1;
            continue;
        }
        // `$( ... )` and `( ... )` stay whole: Stop-Process -Id (Get-Process x).Id
        if ch == '(' {
            depth += 1;
        }
        if ch == ')' && depth > 0 {
            depth -= 1;
        }
        if depth > 0 {
            current.push(ch);
            i += 1;
            continue;
        }
        let two = if i + 1 < chars.len() {
            Some([chars[i], chars[i + 1]])
        } else {
            None
        };
        let op: Option<&str> = if two == Some(['&', '&']) {
            Some("&&")
        } else if two == Some(['|', '|']) {
            Some("||")
        } else if ch == ';' || ch == '\n' || ch == '\r' {
            Some(";")
        } else if ch == '|' {
            Some("|")
        } else if ch == '&' {
            // `2>&1`, `&>` and `>&` are redirections, not separators.
            let prev = if i > 0 { chars[i - 1] } else { '\0' };
            let next = if i + 1 < chars.len() {
                chars[i + 1]
            } else {
                '\0'
            };
            if prev != '>' && prev != '<' && next != '>' {
                Some("&")
            } else {
                None
            }
        } else {
            None
        };
        let Some(op) = op else {
            current.push(ch);
            i += 1;
            continue;
        };
        if !current.trim().is_empty() {
            parts.push(Part {
                text: std::mem::take(&mut current),
                after,
            });
        } else {
            current.clear();
        }
        after = op.to_string();
        i += op.chars().count();
    }
    if !current.trim().is_empty() {
        parts.push(Part {
            text: current,
            after,
        });
    }
    parts
}

/// Words, with quotes removed. A backslash is a path separator here, not an escape.
pub fn tokens(text: &str) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut quote: Option<char> = None;
    for ch in text.chars() {
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            } else {
                current.push(ch);
            }
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            started = true;
            continue;
        }
        if ch.is_whitespace() {
            if started {
                words.push(std::mem::take(&mut current));
            }
            started = false;
            continue;
        }
        current.push(ch);
        started = true;
    }
    if started {
        words.push(current);
    }
    words
}

/// The text inside `( ... )` wrapping a whole part, each `$( ... )`, and each backtick span.
pub fn inner_commands(text: &str) -> Vec<String> {
    let trimmed = text.trim();
    let chars: Vec<char> = trimmed.chars().collect();
    let close = |from: usize| -> isize {
        let mut depth = 0i32;
        for (at, ch) in chars.iter().enumerate().skip(from) {
            if *ch == '(' {
                depth += 1;
            } else if *ch == ')' {
                depth -= 1;
                if depth == 0 {
                    return at as isize;
                }
            }
        }
        -1
    };
    let mut found: Vec<String> = Vec::new();
    if trimmed.starts_with('(') && close(0) == (chars.len() as isize - 1) {
        found.push(chars[1..chars.len() - 1].iter().collect());
    }
    let mut search_from = 0usize;
    while let Some(rel) = trimmed[search_from..].find("$(") {
        let byte_at = search_from + rel;
        let char_at = trimmed[..byte_at].chars().count();
        let end = close(char_at + 1);
        if end > char_at as isize {
            let start_char = char_at + 2;
            let end_char = end as usize;
            found.push(chars[start_char..end_char].iter().collect());
        }
        search_from = byte_at + 2;
    }
    let tick = crate::invariants::regex(r"`([^`]+)`");
    for caps in tick.captures_iter(trimmed) {
        if let Some(m) = caps.get(1) {
            found.push(m.as_str().to_string());
        }
    }
    found
}

/// Drops what runs the command rather than being it: `sudo`, `&`, `env X=1`, `timeout 10`, `nice -n 5`, `do`.
pub fn command_words(words: &[String]) -> Vec<String> {
    let mut at = 0;
    let duration = crate::invariants::regex(r"^\d+(?:\.\d+)?[smhd]?$");
    let assign = crate::invariants::regex(r"^[A-Za-z_][A-Za-z0-9_]*=");
    while at < words.len() {
        let word = &words[at];
        let name = program_name(word);
        // A loop's or a branch's body is a command like any other.
        if matches!(word.as_str(), "do" | "then" | "else" | "elif" | "{" | "!") {
            at += 1;
            continue;
        }
        if word == "&"
            || word == "."
            || matches!(
                name.as_str(),
                "sudo"
                    | "doas"
                    | "nohup"
                    | "time"
                    | "exec"
                    | "command"
                    | "env"
                    | "nice"
                    | "ionice"
                    | "stdbuf"
            )
        {
            at += 1;
            // `sudo -u x`, `env -i`, `nice -n 5`
            while at < words.len() && words[at].starts_with('-') {
                at += if words[at] == "-u" || words[at] == "-n" {
                    2
                } else {
                    1
                };
            }
            continue;
        }
        // `timeout [-s SIGNAL] 10 <command>`: its flags and its duration.
        if name == "timeout" {
            at += 1;
            while at < words.len() && words[at].starts_with('-') {
                at += if words[at] == "-s" || words[at] == "-k" {
                    2
                } else {
                    1
                };
            }
            if at < words.len() && duration.is_match(&words[at]) {
                at += 1;
            }
            continue;
        }
        if assign.is_match(word) {
            at += 1;
            continue;
        }
        break;
    }
    words[at..].to_vec()
}

/// The command inside `powershell -Command`, `cmd /c` or `bash -c`, if this is one.
pub fn wrapped(words: &[String]) -> Option<String> {
    if words.is_empty() {
        return None;
    }
    let name = program_name(&words[0]);
    let args = &words[1..];
    if name == "powershell" || name == "pwsh" {
        let enc = crate::invariants::regex(r"(?i)^-(e|ec|enc|encodedcommand)$");
        let cmd = crate::invariants::regex(r"(?i)^-(c|command)$");
        for i in 0..args.len() {
            let flag = args[i].to_lowercase();
            if enc.is_match(&flag) {
                return args.get(i + 1).and_then(|t| decode_utf16_base64(t));
            }
            if cmd.is_match(&flag) {
                return Some(args[i + 1..].join(" "));
            }
            if flag == "-file" || flag == "-f" {
                return None;
            }
        }
        // `powershell Stop-Process -Name x`: the rest is the command.
        let first = args.iter().position(|arg| !arg.starts_with('-'))?;
        return Some(args[first..].join(" "));
    }
    if name == "cmd" {
        let ck = crate::invariants::regex(r"(?i)^/[ck]$");
        let at = args.iter().position(|arg| ck.is_match(arg))?;
        return Some(args[at + 1..].join(" "));
    }
    if matches!(
        name.as_str(),
        "bash" | "sh" | "zsh" | "dash" | "ksh" | "fish"
    ) {
        let cflag = crate::invariants::regex(r"(?i)^-[a-z]*c[a-z]*$");
        let at = args.iter().position(|arg| cflag.is_match(arg))?;
        return args.get(at + 1).cloned();
    }
    None
}

fn decode_utf16_base64(text: &str) -> Option<String> {
    let bytes = decode_base64(text)?;
    let mut out = String::new();
    let mut i = 0;
    while i + 1 < bytes.len() {
        let unit = u16::from(bytes[i]) | (u16::from(bytes[i + 1]) << 8);
        if let Some(ch) = char::from_u32(u32::from(unit)) {
            out.push(ch);
        } else {
            out.push(char::REPLACEMENT_CHARACTER);
        }
        i += 2;
    }
    Some(out)
}

fn decode_base64(text: &str) -> Option<Vec<u8>> {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0u32;
    for ch in text.bytes() {
        if ch == b'=' || ch.is_ascii_whitespace() {
            continue;
        }
        let val = T.iter().position(|&c| c == ch)? as u32;
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// `C:\Windows\System32\taskkill.exe` and `/usr/bin/pkill` as `taskkill` and `pkill`.
pub fn program_name(word: &str) -> String {
    let base = word.rsplit(['\\', '/']).next().unwrap_or(word);
    crate::invariants::regex(r"(?i)\.(exe|cmd|bat|ps1)$")
        .replace(&base.to_lowercase(), "")
        .into_owned()
}

fn unquote(word: &str) -> String {
    crate::invariants::regex(r#"^['"]|['"]$"#)
        .replace_all(word, "")
        .into_owned()
}

fn option_value<'a>(args: &'a [String], names: &[&str]) -> Option<&'a str> {
    for i in 0..args.len().saturating_sub(1) {
        if names.iter().any(|n| *n == args[i]) {
            return Some(args[i + 1].as_str());
        }
    }
    None
}

/// The last argument that is neither a flag nor a flag's value.
fn last_plain<'a>(args: &'a [String], takes_value: &[&str]) -> Option<&'a str> {
    let mut found: Option<&str> = None;
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if takes_value.contains(&arg.as_str()) {
            i += 2;
            continue;
        }
        if arg.starts_with('-') {
            i += 1;
            continue;
        }
        found = Some(arg.as_str());
        i += 1;
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARENA: &str = "taskkill //F //IM python.exe";

    fn encode_base64(bytes: &[u8]) -> String {
        const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        let mut i = 0;
        while i < bytes.len() {
            let b0 = u32::from(bytes[i]);
            let b1 = u32::from(bytes.get(i + 1).copied().unwrap_or(0));
            let b2 = u32::from(bytes.get(i + 2).copied().unwrap_or(0));
            let n = (b0 << 16) | (b1 << 8) | b2;
            out.push(T[((n >> 18) & 63) as usize] as char);
            out.push(T[((n >> 12) & 63) as usize] as char);
            out.push(if i + 1 < bytes.len() {
                T[((n >> 6) & 63) as usize] as char
            } else {
                '='
            });
            out.push(if i + 2 < bytes.len() {
                T[(n & 63) as usize] as char
            } else {
                '='
            });
            i += 3;
        }
        out
    }

    fn encoded(text: &str) -> String {
        let mut bytes = Vec::new();
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        encode_base64(&bytes)
    }

    fn reaches() -> Vec<(String, &'static str, &'static str)> {
        let enc = encoded("Stop-Process -Name node");
        vec![
            (ARENA.into(), "every-process-named", "python.exe"),
            (
                "taskkill /F /IM node.exe".into(),
                "every-process-named",
                "node.exe",
            ),
            (
                "taskkill /IM Code.exe /T".into(),
                "every-process-named",
                "code.exe",
            ),
            (
                "taskkill /f /fi \"IMAGENAME eq chrome.exe\"".into(),
                "every-process-named",
                "chrome.exe",
            ),
            (
                "C:\\Windows\\System32\\taskkill.exe /IM python.exe".into(),
                "every-process-named",
                "python.exe",
            ),
            (
                "Stop-Process -Name node -Force".into(),
                "every-process-named",
                "node",
            ),
            ("spps -Name python".into(), "every-process-named", "python"),
            (
                "kill -Name electron".into(),
                "every-process-named",
                "electron",
            ),
            (
                "Get-Process python | Stop-Process -Force".into(),
                "every-process-named",
                "python",
            ),
            (
                "Get-Process -Name node -ErrorAction SilentlyContinue | Stop-Process".into(),
                "every-process-named",
                "node",
            ),
            (
                "Get-Process | Where-Object { $_.Name -eq 'vite' } | Stop-Process".into(),
                "every-process-named",
                "vite",
            ),
            (
                "Stop-Process -Id (Get-Process python).Id".into(),
                "every-process-named",
                "python",
            ),
            ("pkill node".into(), "every-process-named", "node"),
            ("pkill -9 python3".into(), "every-process-named", "python3"),
            (
                "pkill -f \"vite --port\"".into(),
                "every-process-named",
                "vite --port",
            ),
            (
                "killall Electron".into(),
                "every-process-named",
                "electron",
            ),
            (
                "sudo killall -9 node".into(),
                "every-process-named",
                "node",
            ),
            (
                "wmic process where name='python.exe' delete".into(),
                "every-process-named",
                "python.exe",
            ),
            (
                "wmic process where \"name='node.exe'\" call terminate".into(),
                "every-process-named",
                "node.exe",
            ),
            (
                "ps aux | grep vite | awk '{print $2}' | xargs kill -9".into(),
                "every-process-named",
                "vite",
            ),
            (
                "pgrep -f uvicorn | xargs kill".into(),
                "every-process-named",
                "uvicorn",
            ),
            (
                "kill $(pgrep node)".into(),
                "every-process-named",
                "node",
            ),
            ("pkill -u colin".into(), "every-process-of-user", "colin"),
            ("kill -9 -1".into(), "every-process-of-user", "you"),
            (
                "taskkill /F /FI \"USERNAME eq alice\"".into(),
                "every-process-of-user",
                "alice",
            ),
            (
                "kill -9 $(lsof -t -i:3000)".into(),
                "every-process-on-port",
                "3000",
            ),
            (
                "lsof -ti tcp:5173 | xargs kill".into(),
                "every-process-on-port",
                "5173",
            ),
            (
                "ps aux | grep python | grep -v grep | awk '{print $2}' | xargs kill".into(),
                "every-process-named",
                "python",
            ),
            (
                "fuser -k 8080/tcp".into(),
                "every-process-on-port",
                "8080",
            ),
            (
                "npx kill-port 3000".into(),
                "every-process-on-port",
                "3000",
            ),
            (
                "Stop-Process -Id (Get-NetTCPConnection -LocalPort 3000).OwningProcess -Force"
                    .into(),
                "every-process-on-port",
                "3000",
            ),
            ("tskill python".into(), "every-process-named", "python"),
            (
                "tskill /A notepad".into(),
                "every-process-named",
                "notepad",
            ),
            (
                "Get-CimInstance Win32_Process -Filter \"name='python.exe'\" | Invoke-CimMethod -MethodName Terminate".into(),
                "every-process-named",
                "python.exe",
            ),
            (
                "Get-WmiObject Win32_Process -Filter \"name='node.exe'\" | % { $_.Terminate() }"
                    .into(),
                "every-process-named",
                "node.exe",
            ),
            (
                "gwmi win32_process -filter \"Name = 'code.exe'\" | ForEach-Object { $_.Terminate() }"
                    .into(),
                "every-process-named",
                "code.exe",
            ),
            ("shutdown /l".into(), "every-process-of-user", "you"),
            ("shutdown -h now".into(), "whole-machine", "shut down"),
            ("shutdown /r /t 0".into(), "whole-machine", "restart"),
            ("shutdown /s /t 60".into(), "whole-machine", "shut down"),
            (
                "Restart-Computer -Force".into(),
                "whole-machine",
                "restart",
            ),
            ("Stop-Computer".into(), "whole-machine", "shut down"),
            ("sudo reboot".into(), "whole-machine", "restart"),
            ("systemctl poweroff".into(), "whole-machine", "shut down"),
            (
                "(cd build && pkill node)".into(),
                "every-process-named",
                "node",
            ),
            (
                "echo $(pkill vite)".into(),
                "every-process-named",
                "vite",
            ),
            (
                "echo `killall Electron`".into(),
                "every-process-named",
                "electron",
            ),
            (
                "for i in 1 2; do pkill python; done".into(),
                "every-process-named",
                "python",
            ),
            (
                "if true; then killall node; fi".into(),
                "every-process-named",
                "node",
            ),
            (
                "timeout 5 pkill node".into(),
                "every-process-named",
                "node",
            ),
            (
                "timeout -s KILL 10s killall vite".into(),
                "every-process-named",
                "vite",
            ),
            (
                "nice -n 10 pkill python".into(),
                "every-process-named",
                "python",
            ),
            (
                "\"C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe\" -Command 'Stop-Process -Name node -Force'".into(),
                "every-process-named",
                "node",
            ),
            (
                "powershell -NoProfile -Command \"Get-Process python | Stop-Process\"".into(),
                "every-process-named",
                "python",
            ),
            (
                format!("powershell -NoProfile -EncodedCommand {enc}"),
                "every-process-named",
                "node",
            ),
            (
                "cmd /c \"taskkill /F /IM python.exe\"".into(),
                "every-process-named",
                "python.exe",
            ),
            (
                "cmd.exe /s /c \"npm run build && taskkill /IM node.exe /F\"".into(),
                "every-process-named",
                "node.exe",
            ),
            (
                "bash -lc 'pkill -f vite || true'".into(),
                "every-process-named",
                "vite",
            ),
            (
                "\"C:\\Program Files\\Git\\bin\\bash.exe\" -lc \"taskkill //F //IM python.exe 2>&1; echo done\"".into(),
                "every-process-named",
                "python.exe",
            ),
            (
                "npm test; pkill node".into(),
                "every-process-named",
                "node",
            ),
            (
                "cd app && killall node & npm start".into(),
                "every-process-named",
                "node",
            ),
        ]
    }

    const DOES_NOT: &[&str] = &[
        "taskkill /PID 1234",
        "taskkill /F /PID 1234 /T",
        "tskill 1234",
        "Get-WmiObject Win32_Process -Filter \"processid=1234\" | % { $_.Terminate() }",
        "Get-CimInstance Win32_Process -Filter \"name='python.exe'\" | Select-Object Name",
        "shutdown /h",
        "kill 1234",
        "kill -9 1234",
        "kill %1",
        "kill -TERM 4567 4568",
        "Stop-Process -Id 1234",
        "Stop-Process -Id $proc.Id",
        "Stop-Process 1234",
        "$p = Start-Process python -PassThru; Stop-Process -Id $p.Id",
        "echo \"taskkill /IM python.exe\"",
        "Write-Output 'Stop-Process -Name node'",
        "echo \"pkill node\" > notes.txt",
        "echo \"build done && taskkill /IM python.exe\"",
        "git commit -m \"tests pass; pkill node no longer needed\"",
        "git commit -m \"kill the old shutdown path\"",
        "grep -rn \"taskkill /IM\" src",
        "Get-Process python",
        "ps aux | grep vite",
        "pgrep node",
        "cat pids.txt | grep -v \"^#\" | xargs kill",
        "type pids.txt | findstr /v \"#\" | xargs kill",
        "shutdown /a",
        "npm run build && npm test",
        "python -m pytest -q",
        "cmd /c dir",
        "powershell -Command \"Write-Output FIXTURE_APPROVAL\"",
        "bash -lc 'ls -la'",
        "cat killall.log",
        "node scripts/kill-switch.js",
        "npm run build 2>&1 | tail -20",
        "wmic process where processid=1234 delete",
        "systemctl status nginx",
        "fuser 8080/tcp",
    ];

    #[test]
    fn reaches_table() {
        for (command, kind, target) in reaches() {
            let reach = command_reach(Some(&command));
            assert!(reach.is_some(), "expected reach for {command:?}");
            let reach = reach.unwrap();
            assert_eq!(reach.kind.as_str(), kind, "kind for {command:?}");
            assert_eq!(reach.target, target, "target for {command:?}");
        }
    }

    #[test]
    fn does_not_reach() {
        for command in DOES_NOT {
            assert!(
                command_reach(Some(command)).is_none(),
                "expected no reach for {command:?}"
            );
        }
    }

    #[test]
    fn says_it_in_words_a_person_reads() {
        assert_eq!(
            command_reach(Some(ARENA)),
            Some(CommandReach {
                kind: CommandReachKind::EveryProcessNamed,
                target: "python.exe".into(),
                short: "stops every python.exe".into(),
                said:
                    "Stops every python.exe on this computer, not only the ones this run started."
                        .into(),
            })
        );
        assert_eq!(
            command_reach(Some("kill -9 $(lsof -t -i:3000)"))
                .as_ref()
                .map(|r| r.said.as_str()),
            Some(
                "Stops whatever program is using port 3000, which may be one this run did not start."
            )
        );
        assert_eq!(
            command_reach(Some("shutdown /r /t 0"))
                .as_ref()
                .map(|r| r.said.as_str()),
            Some("Restarts this computer, closing every program on it.")
        );
    }

    #[test]
    fn none_when_command_absent() {
        assert_eq!(command_reach(None), None);
    }
}
