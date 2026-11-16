use super::*;
use serde_json::Value;

// The role defaults and empty-folder variants are IdleTeammate.tsx's words.
fn role_starters(role: &str) -> Vec<String> {
    let words: &[&str] = match role {
        "Code & Migrations" => &[
            "Read this project and tell me what it does, in one paragraph.",
            "Find the riskiest file in this repo and explain why.",
            "What would break first if traffic doubled?",
        ],
        "Research & Briefs" => &[
            "Summarize what is in this folder, for someone seeing it for the first time.",
            "List the decisions already made here that would be expensive to reverse.",
            "What would you want to know before you started on anything here?",
        ],
        "Ops & Scheduling" => &[
            "What work here repeats, and which of it could run on a schedule?",
            "Which failures here would nobody notice for a week?",
            "Make me a checklist for the routine work this folder seems to need.",
        ],
        "Docs & QA" => &[
            "Find anything written here that no longer matches what it describes.",
            "What here is unchecked that would hurt most if it were wrong?",
            "Read the main document here and tell me what a new reader would still get wrong.",
        ],
        "Data & Reporting" => &[
            "Find the numbers in this folder and say where each one comes from.",
            "What would a weekly summary of the work here contain?",
            "Which figure here would you check first, and why?",
        ],
        "Chief of Staff" => &[
            "Look at the team and tell me who should take what.",
            "Ask each teammate for one thing they would do first, and bring me the list.",
            "What is the state of the work here? Delegate the reading and report back.",
        ],
        _ => &[
            "What can you help me with? Give me three concrete examples.",
            "Ask me three questions about what I need, then suggest where to start.",
            "Look at the files in this folder and tell me what you could do with them.",
        ],
    };
    words.iter().map(|s| (*s).into()).collect()
}
fn fresh_role_starters(role: &str) -> Vec<String> {
    let words: &[&str] = match role {
        "Code & Migrations" => &[
            "Make a small web page here that shows today's date and a quote, and tell me how to open it.",
            "Ask me what I want to build, then set up its first files here.",
            "Write a small script here that renames photos by the date they were taken, and explain how to run it.",
        ],
        "Research & Briefs" => &[
            "Ask me what I am researching, then write a one-page brief on it, with sources.",
            "Compare three ways to make a simple website today: what each costs and how hard it is, with sources.",
            "What should I know before I start a project like mine? Ask me two questions first.",
        ],
        "Ops & Scheduling" => &[
            "Ask me what I do every week, then say which of it could run on a schedule.",
            "Write a weekly plan I can fill in each Monday, as a file here.",
            "Make me a checklist for the routine work of a small business, as a file here.",
        ],
        "Docs & QA" => &[
            "Write me a checklist for testing a website before I share it.",
            "Ask me about a project of mine, then write the README it should have.",
            "Explain what a careful review looks for, with one short example of each point.",
        ],
        "Data & Reporting" => &[
            "Make me a monthly budget spreadsheet here, with categories, totals and a line for savings.",
            "Make a spreadsheet here to track hours worked, with a weekly total.",
            "Ask me what I want to keep track of, then set up a spreadsheet for it here.",
        ],
        "Chief of Staff" => &[
            "Look at the team and tell me who should take what.",
            "Ask each teammate for one thing they would do first, and bring me the list.",
            "Ask me what I am working on, then split it between the team.",
        ],
        _ => &[
            "What can you help me with? Give me three concrete examples.",
            "Ask me three questions about what I need, then suggest where to start.",
            "Make something small here that shows what you can do, and tell me how to open it.",
        ],
    };
    words.iter().map(|s| (*s).into()).collect()
}

fn starters(entry: &Value, role: &str, fresh: bool) -> Vec<String> {
    let own = entry["starters"].as_array().map(|a| {
        a.iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect::<Vec<_>>()
    });
    if fresh {
        if let Some(own) = &own {
            let refs: Vec<_> = own.iter().map(String::as_str).collect();
            if let Some(words) = crate::starters::fresh_starters_for(&refs) {
                return words.iter().map(|s| (*s).into()).collect();
            }
        } else {
            return fresh_role_starters(role);
        }
    }
    own.unwrap_or_else(|| role_starters(role))
}
fn description(role: &str) -> Option<&'static str> {
    Some(match role {
        "Code & Migrations" => "Repo work, refactors, test runs",
        "Research & Briefs" => "Reading, comparing, summarising",
        "Ops & Scheduling" => "Routine jobs and reminders",
        "Docs & QA" => "Written output and checking",
        "Data & Reporting" => "Spreadsheets, figures, digests",
        "Chief of Staff" => "Routes work to the team, reports back",
        _ => return None,
    })
}
fn mode_sentence(mode: &str) -> &'static str {
    match mode {
        "accept-edits" => {
            "Edit is on, so it may change files here; switch to Ask below to keep it read-only."
        }
        "plan" => "In Plan mode it answers with the steps it would take and changes nothing.",
        "approve-each" => "Every change waits for your approval before it lands.",
        "auto" => {
            "Auto is on: it runs without asking and may change files anywhere on this machine, not only in this folder."
        }
        _ => "In Ask mode nothing is changed: every write is refused.",
    }
}

pub(super) fn view(s: &State, mate: &Teammate, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let entry = s.people_panel.team["teammates"]
        .as_array()
        .and_then(|a| {
            a.iter()
                .find(|e| e["teammateId"].as_str() == Some(&mate.id))
        })
        .unwrap_or(&Value::Null);
    let role = entry["role"].as_str().unwrap_or(&mate.role);
    let label = if role == "Custom" {
        entry["roleTitle"]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(role)
    } else {
        role
    };
    let about = description(role)
        .map(|d| format!(" · {d}"))
        .unwrap_or_default();
    let mut body = col()
        .key("messages")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .center()
        .padding(24.0)
        .gap(10.0)
        .child(crate::look::faces::face(
            s.image_cache.as_deref(),
            &mate.id,
            &mate.name,
            s.looks.get(&mate.id),
            56.0,
            s.settings_panel.settings.terminal_faces,
            &l,
        ))
        .child(text(mate.name.clone()).font(type_::TITLE).color(p.text))
        .child(
            text(format!(
                "{label}{about}. {}",
                mode_sentence(s.composer_mode.as_deref().unwrap_or(&mate.runs_in))
            ))
            .color(p.secondary)
            .wrap(),
        );
    let blocked = s
        .home
        .boot
        .probes
        .iter()
        .find(|probe| Some(probe.id.as_str()) == mate.runtime.as_deref())
        .and_then(|probe| match probe.outcome {
            Some("missing") => Some("AI agent not installed"),
            Some("needs-signin") => Some("Sign-in needed"),
            _ => None,
        });
    for (i, prompt) in starters(entry, role, s.home.folder_invented)
        .into_iter()
        .enumerate()
    {
        let enabled = !mate.busy && blocked.is_none() && !turn_running(s);
        let mut button = row()
            .w_full()
            .max_w(640.0)
            .min_h(32.0)
            .padding(10.0)
            .radius(l.t.px("radius-control"))
            .border(1.0, l.c("border-strong"))
            .focusable(enabled)
            .focus_ring(2.0, 2.0, l.c("focus-ring"))
            .role("button")
            .label(prompt.clone())
            .key(format!("idle-starter-{i}"))
            .child(
                text(prompt.clone())
                    .font(l.ty.control)
                    .color(l.c("text-secondary"))
                    .w_full()
                    .wrap(),
            );
        if enabled {
            button = button.hover(l.c("hover-surface")).press_scale(0.98);
            button = button.on_press_tx(move |s: &mut State, tx: &mut Tx| {
                s.draft.replace_text(prompt.clone());
                send(s, tx);
            });
        }
        body = body.child(button);
    }
    body.child(
        text(blocked.unwrap_or(crate::starters::PICK_ONE).to_owned())
            .color(if blocked.is_some() { p.red } else { p.muted })
            .wrap(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::starters::{TEAM_TEMPLATES, TemplateTeammate};
    fn made(m: &TemplateTeammate) -> Value {
        serde_json::json!({"teammateId": format!("tm_{}", m.name.to_lowercase()), "name":m.name,
            "hue":m.hue,"role":m.role,"roleTitle":m.role_title,"starters":m.starters,"createdAt":"2026-09-26T09:00:00.000Z"})
    }
    fn state(entry: Value) -> State {
        let mut s = State::default();
        s.demo = true;
        s.mode = "accept-edits".into();
        s.teammates = vec![Teammate {
            id: entry["teammateId"].as_str().unwrap().into(),
            name: entry["name"].as_str().unwrap().into(),
            role: entry["role"].as_str().unwrap().into(),
            runtime: None,
            model: None,
            account: None,
            busy: false,
            runs_in: "accept-edits".into(),
            folder: None,
        }];
        s.selected = 0;
        s.conversations
            .insert(s.teammates[0].id.clone(), Conversation::default());
        s.people_panel.team = serde_json::json!({"teammates":[entry]});
        s
    }
    fn draw(entry: Value) -> Vec<(Option<String>, Option<String>)> {
        let mut rt = rune::Runtime::new(
            state(entry),
            |s, _| crate::thread_view::thread_view(s, &Palette::new(true)),
            (1200.0, 1800.0),
            rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        rt.elements().into_iter().map(|e| (e.key, e.text)).collect()
    }
    fn words(entry: Value) -> String {
        draw(entry)
            .into_iter()
            .filter_map(|e| e.1)
            .collect::<Vec<_>>()
            .join("\n")
    }
    fn no_code(s: &str) {
        assert!(
            !regex::Regex::new(r"(?i)\b(code|codebase|repo|repository|pull request|README)\b")
                .unwrap()
                .is_match(s),
            "{s}"
        );
    }
    #[test]
    fn offers_a_template_teammates_own_first_messages() {
        for template in &TEAM_TEMPLATES {
            for mate in template.teammates {
                let shown = words(made(mate));
                for starter in mate.starters {
                    assert!(shown.contains(starter), "{}: {shown}", mate.name);
                }
            }
        }
    }
    #[test]
    fn never_offers_code_to_the_money_or_the_writing_team() {
        for template in TEAM_TEMPLATES
            .iter()
            .filter(|t| t.template_id != "software")
        {
            for mate in template.teammates {
                for starter in mate.starters {
                    no_code(starter);
                }
                let shown = words(made(mate));
                assert!(
                    !shown.contains("codebase") && !shown.contains("reads this workspace"),
                    "{shown}"
                );
            }
        }
    }
    #[test]
    fn never_asks_a_non_coder_for_a_page_the_app_would_only_show_as_code() {
        let pattern = regex::Regex::new(r"(?i)\b(HTML|CSS|JavaScript|website|web page)\b").unwrap();
        for template in TEAM_TEMPLATES
            .iter()
            .filter(|t| t.template_id != "software")
        {
            for mate in template.teammates {
                for starter in mate.starters {
                    assert!(!pattern.is_match(starter), "{starter}");
                }
            }
        }
    }
    #[test]
    fn says_what_the_role_is_for_not_that_every_role_reads_the_workspace() {
        assert!(
            words(made(&TEAM_TEMPLATES[1].teammates[0]))
                .contains("Research & Briefs · Reading, comparing, summarising.")
        );
        assert!(words(made(&TEAM_TEMPLATES[2].teammates[0])).contains("Writer. Edit is on"));
    }
    #[test]
    fn offers_a_hand_made_teammate_its_roles_and_speaks_of_code_only_for_the_roles_that_are_code() {
        let hand = |role| serde_json::json!({"teammateId":"tm_hand","name":"Hand","hue":"lime","role":role,"createdAt":"2026-09-26T09:00:00.000Z"});
        assert!(words(hand("Code & Migrations")).contains("Find the riskiest file in this repo"));
        for role in [
            "Research & Briefs",
            "Ops & Scheduling",
            "Docs & QA",
            "Data & Reporting",
            "Chief of Staff",
            "Custom",
        ] {
            let elements = draw(hand(role));
            let buttons: Vec<_> = elements
                .iter()
                .filter(|e| {
                    e.0.as_deref()
                        .is_some_and(|k| k.starts_with("idle-starter-"))
                })
                .collect();
            assert_eq!(buttons.len(), 3, "{role}");
            for prompt in role_starters(role) {
                no_code(&prompt);
                assert!(elements.iter().any(|e| e.1.as_deref() == Some(&prompt)));
            }
        }
    }
    #[test]
    fn the_idle_sentence_uses_the_composers_mode_instead_of_a_stale_global_mode() {
        let mut s = state(made(&TEAM_TEMPLATES[1].teammates[0]));
        s.mode = "auto".into();
        s.composer_mode = Some("ask".into());
        let mut rt = rune::Runtime::new(
            s,
            |s, _| crate::thread_view::thread_view(s, &Palette::new(true)),
            (1200.0, 1800.0),
            rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        let shown = rt
            .elements()
            .into_iter()
            .filter_map(|e| e.text)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            shown.contains("In Ask mode nothing is changed: every write is refused."),
            "{shown}"
        );
        assert!(!shown.contains("Auto is on:"), "{shown}");
    }
    #[test]
    fn roster_refresh_keeps_the_template_starters_before_the_team_screen_is_opened() {
        let root =
            std::env::temp_dir().join(format!("idle-roster-{}", crate::composer::new_queue_id()));
        let store = store::Store::new(root.join("native"), root.join("electron")).unwrap();
        let mut entry = made(&TEAM_TEMPLATES[2].teammates[0]);
        entry.as_object_mut().unwrap().remove("teammateId");
        let id = people::save_teammate_id(&store, entry).unwrap();
        let mut s = State::default();
        s.demo = true;
        s.data = Some(store.clone());
        s.composer_mode = Some("accept-edits".into());
        s.teammates = native::roster_in(&store).unwrap();
        s.selected = s.teammates.iter().position(|t| t.id == id).unwrap();
        s.conversations.insert(id, Conversation::default());
        assert!(s.people_panel.team.is_null());
        sync_teammate_connectors(&mut s);
        let mut rt = rune::Runtime::new(
            s,
            |s, _| crate::thread_view::thread_view(s, &Palette::new(true)),
            (1200.0, 1800.0),
            rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        let shown = rt
            .elements()
            .into_iter()
            .filter_map(|e| e.text)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(shown.contains("Writer. Edit is on"), "{shown}");
        for prompt in TEAM_TEMPLATES[2].teammates[0].starters {
            assert!(shown.contains(prompt), "{shown}");
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
