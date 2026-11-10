//! Composer placeholder strings — copy Electron's words exactly.

pub struct PlaceholderInput<'a> {
    pub working_now: bool,
    pub working_name: &'a str,
    pub queued: bool,
    pub route_can_run: bool,
    pub selected_ready: bool,
    pub selected_display: Option<&'a str>,
    pub teammate_name: Option<&'a str>,
    pub discovery_loading: bool,
    pub discovery_error: bool,
    pub ready_elsewhere: Option<&'a str>,
    pub auth_required: bool,
    pub installed_silent: usize,
    pub checking: bool,
    pub nothing_installed: bool,
}

/// The invitation in the box (Composer.tsx placeholder chain, direct/chat path).
pub fn placeholder(input: PlaceholderInput<'_>) -> String {
    if input.working_now {
        return if input.queued {
            "Edit the queued message to change it…".into()
        } else {
            format!(
                "Say what is next — it goes to {} when this finishes…",
                input.working_name
            )
        };
    }
    if !input.route_can_run {
        let name = input.selected_display.unwrap_or("that AI agent");
        return format!("Locust cannot run {name} fully yet — pick another model to start…");
    }
    if input.selected_ready {
        return match input.teammate_name {
            None => "Write a message…".into(),
            Some(name) => format!("Message {name}…"),
        };
    }
    if input.discovery_loading {
        return "Looking for the AI agents on this machine…".into();
    }
    if input.discovery_error {
        return "Locust could not look for AI agents on this machine…".into();
    }
    if let Some(elsewhere) = input.ready_elsewhere {
        return format!("{elsewhere} is ready — pick it in the model menu…");
    }
    if input.auth_required {
        return "Install an AI agent and sign in to start…".into();
    }
    if input.installed_silent == 1 {
        return "An AI agent is installed but not answering — Check again above…".into();
    }
    if input.installed_silent > 1 {
        return format!(
            "{} AI agents are installed but not answering — Check again above…",
            input.installed_silent
        );
    }
    if input.checking {
        return "Checking the AI agents on this machine…".into();
    }
    if input.nothing_installed {
        return "Install an AI agent to start…".into();
    }
    match input.teammate_name {
        None => "Write a message…".into(),
        Some(name) => format!("Message {name}…"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_teammate() {
        assert_eq!(
            placeholder(PlaceholderInput {
                working_now: false,
                working_name: "Codec",
                queued: false,
                route_can_run: true,
                selected_ready: true,
                selected_display: Some("Codex"),
                teammate_name: Some("Codec"),
                discovery_loading: false,
                discovery_error: false,
                ready_elsewhere: None,
                auth_required: false,
                installed_silent: 0,
                checking: false,
                nothing_installed: false,
            }),
            "Message Codec…"
        );
    }

    #[test]
    fn busy_and_install() {
        assert!(placeholder(PlaceholderInput {
            working_now: true,
            working_name: "Codec",
            queued: false,
            route_can_run: true,
            selected_ready: true,
            selected_display: None,
            teammate_name: Some("Codec"),
            discovery_loading: false,
            discovery_error: false,
            ready_elsewhere: None,
            auth_required: false,
            installed_silent: 0,
            checking: false,
            nothing_installed: false,
        })
        .contains("goes to Codec"));
        assert_eq!(
            placeholder(PlaceholderInput {
                working_now: false,
                working_name: "",
                queued: false,
                route_can_run: true,
                selected_ready: false,
                selected_display: None,
                teammate_name: None,
                discovery_loading: false,
                discovery_error: false,
                ready_elsewhere: None,
                auth_required: false,
                installed_silent: 0,
                checking: false,
                nothing_installed: true,
            }),
            "Install an AI agent to start…"
        );
    }
}
