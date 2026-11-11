//! Home starters and team templates — ports of StarterList.tsx and team-templates.ts.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Starter {
    pub id: String,
    pub label: &'static str,
    pub line: &'static str,
    pub prompt: &'static str,
    pub mode: Option<&'static str>,
    pub compare_changes: bool,
}

pub const START_A_CONVERSATION: &str = "Start a conversation";
pub const COMPARE_MODELS: &str = "Compare models";
pub const START_FROM_ISSUE: &str = "Start from an issue";
pub const COMPARE: &str = "Compare";
pub const START_WITH_A_TEAM: &str = "Start with a team";
pub const TEAM_FROM_IMAGE: &str = "Team from an image";
pub const NEW_TEAMMATE: &str = "New teammate";
pub const YOUR_TEAM: &str = "Your team";
pub const COVER_CLAIM: &str = "Autonomous teammates on your own machine";
pub const PICK_ONE: &str = "Pick one, or say what you need below.";
pub const CONNECT_RUNTIME: &str = "Connect a runtime to start a mission.";

#[derive(Clone, Copy)]
struct StarterDef {
    id: &'static str,
    label: &'static str,
    line: &'static str,
    prompt: &'static str,
    mode: Option<&'static str>,
    compare_changes: bool,
}

const STARTER_DEFS: [StarterDef; 4] = [
    StarterDef {
        id: "plan",
        label: "Plan before building",
        line: "Agree on the approach before anything changes",
        prompt: "Read the code first, change nothing, and plan how to: ",
        mode: Some("plan"),
        compare_changes: false,
    },
    StarterDef {
        id: "debug",
        label: "Debug an issue",
        line: "Find the root cause and fix it",
        prompt: "Find the root cause of this bug, fix it, and show what you changed: ",
        mode: None,
        compare_changes: true,
    },
    StarterDef {
        id: "review",
        label: "Review my changes",
        line: "Read this branch’s changes like a careful reviewer",
        prompt: "Review the changes on this branch as a careful reviewer. List bugs and risks by severity, each with its file and line. Change nothing.",
        mode: Some("ask"),
        compare_changes: false,
    },
    StarterDef {
        id: "page",
        label: "Build a landing page",
        line: "For whatever you’re launching, built and running",
        prompt: "Make index.html: a one-page landing page with a headline, what it offers, and a clear way to get in touch. One file, all CSS inside it. It is for: ",
        mode: None,
        compare_changes: true,
    },
];

const PLAN_MAKING: StarterDef = StarterDef {
    id: "plan-make",
    label: "Plan before making",
    line: "Agree on the approach before anything is made",
    prompt: "Change nothing yet, and plan how to: ",
    mode: Some("plan"),
    compare_changes: false,
};

fn starter_from(def: StarterDef) -> Starter {
    Starter {
        id: def.id.into(),
        label: def.label,
        line: def.line,
        prompt: def.prompt,
        mode: def.mode,
        compare_changes: def.compare_changes,
    }
}

pub fn coder_starters() -> Vec<Starter> {
    STARTER_DEFS.into_iter().map(starter_from).collect()
}

fn role_starter(role: &str) -> Option<(&'static str, &'static str, &'static str)> {
    Some(match role {
        "Research & Briefs" => (
            "Research a question",
            "What the sources say, then a conclusion",
            "Research this and write a short brief: what the sources say, each with its date, then your own conclusion, kept apart: ",
        ),
        "Data & Reporting" => (
            "Work out the numbers",
            "Every step shown, the result in a table",
            "Work this out with every step of the arithmetic shown, and put the result in a table: ",
        ),
        "Chief of Staff" => (
            "Ask the whole team",
            "Each teammate’s part, brought back as one answer",
            "Ask the team about this: get each teammate’s part, then bring me one answer: ",
        ),
        "Ops & Scheduling" => (
            "Put it on a schedule",
            "What comes first, and when each part is done",
            "Put this on a schedule: what comes first, what can wait, and when each part will be done: ",
        ),
        "Docs & QA" => (
            "Check a document",
            "What is wrong, unclear or missing",
            "Check this carefully and list what is wrong, unclear or missing, most important first: ",
        ),
        "Writer" => (
            "Write a first draft",
            "One page, in the voice you ask for",
            "Write a first draft of this, one page, and say which voice you chose: ",
        ),
        "Designer" => (
            "Make a design",
            "Colours, fonts and a layout, as a file",
            "Design this as a file in this folder: a colour palette with hex codes, two font pairings and a layout: ",
        ),
        "Editor" => (
            "Edit for clarity",
            "What is unclear, wrong or too long",
            "Edit this for clarity: mark what is unclear, wrong or too long, then give me the edited version: ",
        ),
        _ => return None,
    })
}

pub fn starters_for_roles(roles: &[&str]) -> Vec<Starter> {
    if roles.is_empty() || roles.contains(&"Code & Migrations") {
        return coder_starters();
    }
    let mut rows = Vec::new();
    for role in roles {
        let Some((label, line, prompt)) = role_starter(role) else {
            continue;
        };
        let id = format!("role:{role}");
        if rows.iter().any(|r: &Starter| r.id == id) {
            continue;
        }
        rows.push(Starter {
            id,
            label,
            line,
            prompt,
            mode: None,
            compare_changes: false,
        });
        if rows.len() == STARTER_DEFS.len() - 1 {
            break;
        }
    }
    if rows.is_empty() {
        coder_starters()
    } else {
        rows.push(starter_from(PLAN_MAKING));
        rows
    }
}

pub fn teammate_for_starter(starter_id: &str, team: &[(String, String)]) -> Option<String> {
    if let Some(role) = starter_id.strip_prefix("role:") {
        return team
            .iter()
            .find(|(_, r)| r == role)
            .map(|(id, _)| id.clone());
    }
    if starter_id == PLAN_MAKING.id {
        return team
            .iter()
            .find(|(_, r)| r == "Chief of Staff")
            .or_else(|| team.first())
            .map(|(id, _)| id.clone());
    }
    let wanted: &[&str] = if starter_id == "review" {
        &["Docs & QA", "Code & Migrations"]
    } else {
        &["Code & Migrations"]
    };
    for role in wanted {
        if let Some((id, _)) = team.iter().find(|(_, r)| r == role) {
            return Some(id.clone());
        }
    }
    None
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TemplateTeammate {
    pub name: &'static str,
    pub role: &'static str,
    pub role_title: Option<&'static str>,
    pub hue: &'static str,
    pub starters: &'static [&'static str],
    pub fresh_starters: Option<&'static [&'static str]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamTemplate {
    pub template_id: &'static str,
    pub title: &'static str,
    pub line: &'static str,
    pub teammates: &'static [TemplateTeammate],
}

pub const TEAM_TEMPLATES: [TeamTemplate; 3] = [
    TeamTemplate {
        template_id: "software",
        title: "Build software",
        line: "A coder, a reviewer who tests, and a researcher.",
        teammates: &[
            TemplateTeammate {
                name: "Wren",
                role: "Code & Migrations",
                role_title: None,
                hue: "lime",
                starters: &[
                    "Read this project and tell me what it does, in one paragraph.",
                    "Find the riskiest file in this repo and explain why.",
                    "Pick one small thing here that is broken or missing, fix it, and show me what you changed.",
                ],
                fresh_starters: Some(&[
                    "Make a small web page here that shows today's date and a quote, and tell me how to open it.",
                    "Ask me what I want to build, then set up its first files here.",
                    "Write a small script here that renames photos by the date they were taken, and explain how to run it.",
                ]),
            },
            TemplateTeammate {
                name: "Juno",
                role: "Docs & QA",
                role_title: None,
                hue: "violet",
                starters: &[
                    "What is untested here that would hurt most if it broke?",
                    "Find documentation in this repo that no longer matches the code.",
                    "Review the most recent change in this repo as if it were a pull request.",
                ],
                fresh_starters: Some(&[
                    "Write me a checklist for testing a website before I share it.",
                    "Ask me about a project of mine, then write the README it should have.",
                    "Explain what a careful code review looks for, with one short example of each point.",
                ]),
            },
            TemplateTeammate {
                name: "Atlas",
                role: "Research & Briefs",
                role_title: None,
                hue: "blue",
                starters: &[
                    "Summarize what this codebase is for, for someone joining tomorrow.",
                    "List the decisions this project has already made that would be expensive to reverse.",
                    "Ask Wren what is hardest in this code, then find how other projects solve it, with sources.",
                ],
                fresh_starters: Some(&[
                    "Compare three ways to make a simple website today: what each costs and how hard it is, with sources.",
                    "Ask me what I am researching, then write a one-page brief on it, with sources.",
                    "Ask Wren what we could build first, then find two or three projects like it, with sources.",
                ]),
            },
        ],
    },
    TeamTemplate {
        template_id: "money",
        title: "Research & money",
        line: "An analyst, a keeper of the numbers, and a planner.",
        teammates: &[
            TemplateTeammate {
                name: "Sable",
                role: "Research & Briefs",
                role_title: None,
                hue: "teal",
                starters: &[
                    "Where can I keep savings I won't need for three years? Compare the options with today's rates, and name your sources.",
                    "Explain index funds, bonds and CDs in plain words: what each is for, and how each can lose money.",
                    "Make me a checklist for judging whether an article about money is worth trusting.",
                ],
                fresh_starters: None,
            },
            TemplateTeammate {
                name: "Penny",
                role: "Data & Reporting",
                role_title: None,
                hue: "butter",
                starters: &[
                    "Make me a monthly budget spreadsheet in this folder, with categories, totals and a line for savings.",
                    "If I save $400 a month at 4.5% a year, what will I have in five years? Show the arithmetic.",
                    "Read the spreadsheets in this folder and tell me what the numbers say.",
                ],
                fresh_starters: None,
            },
            TemplateTeammate {
                name: "Rook",
                role: "Chief of Staff",
                role_title: None,
                hue: "slate",
                starters: &[
                    "Plan my next three months of saving: ask Sable for the options and Penny for the numbers, then bring me one plan.",
                    "Ask Sable and Penny what each would need from me to help with my money, and bring me one list.",
                    "I want to pay off a $6,000 card balance within a year. Work out a plan with the team.",
                ],
                fresh_starters: None,
            },
        ],
    },
    TeamTemplate {
        template_id: "writing",
        title: "Write & design",
        line: "A writer, a designer, and an editor.",
        teammates: &[
            TemplateTeammate {
                name: "Quill",
                role: "Custom",
                role_title: Some("Writer"),
                hue: "rose",
                starters: &[
                    "Ask me five questions about my business, then write a one-page introduction to it.",
                    "Show me three ways to open a newsletter, each in a different voice, and say when each works.",
                    "Turn the notes in this folder into a clear one-page summary.",
                ],
                fresh_starters: None,
            },
            TemplateTeammate {
                name: "Iris",
                role: "Custom",
                role_title: Some("Designer"),
                hue: "clay",
                starters: &[
                    "Make a one-page brand guide for a small bakery in this folder: a colour palette with hex codes, two font pairings, and the voice to write in.",
                    "Suggest a colour palette and two font pairings for a calm, modern brand, with hex codes.",
                    "Look at the files in this folder and suggest how to make them look more polished.",
                ],
                fresh_starters: None,
            },
            TemplateTeammate {
                name: "Moss",
                role: "Custom",
                role_title: Some("Editor"),
                hue: "pearl",
                starters: &[
                    "Read the documents in this folder and mark what is unclear, wrong or too long.",
                    "Give me a checklist for editing my own writing, with an example for each point.",
                    "Ask Quill for a short piece about this team, then edit it and show me both versions.",
                ],
                fresh_starters: None,
            },
        ],
    },
];

pub fn fresh_starters_for(starters: &[&str]) -> Option<&'static [&'static str]> {
    for template in &TEAM_TEMPLATES {
        for mate in template.teammates {
            if mate.starters.len() == starters.len()
                && mate
                    .starters
                    .iter()
                    .zip(starters.iter())
                    .all(|(a, b)| a == b)
            {
                return mate.fresh_starters;
            }
        }
    }
    None
}

pub fn template_by_id(id: &str) -> Option<&'static TeamTemplate> {
    TEAM_TEMPLATES.iter().find(|t| t.template_id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coder_starters_match_electron() {
        let rows = coder_starters();
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].label, "Plan before building");
        assert_eq!(rows[3].label, "Build a landing page");
        assert!(rows[3].prompt.starts_with("Make index.html:"));
        assert_eq!(rows[2].mode, Some("ask"));
        assert!(rows[1].compare_changes);
    }

    #[test]
    fn team_without_coder_gets_role_starters() {
        let rows = starters_for_roles(&["Research & Briefs", "Data & Reporting", "Chief of Staff"]);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].label, "Research a question");
        assert_eq!(rows[1].label, "Work out the numbers");
        assert_eq!(rows[2].label, "Ask the whole team");
        assert_eq!(rows[3].label, "Plan before making");
        assert_eq!(rows[3].id, "plan-make");
    }

    #[test]
    fn teammate_for_starter_picks_by_role() {
        let team = vec![
            ("tm_wren".into(), "Code & Migrations".into()),
            ("tm_juno".into(), "Docs & QA".into()),
            ("tm_atlas".into(), "Research & Briefs".into()),
        ];
        assert_eq!(
            teammate_for_starter("plan", &team).as_deref(),
            Some("tm_wren")
        );
        assert_eq!(
            teammate_for_starter("review", &team).as_deref(),
            Some("tm_juno")
        );
        assert_eq!(
            teammate_for_starter("role:Research & Briefs", &team).as_deref(),
            Some("tm_atlas")
        );
    }

    #[test]
    fn templates_match_electron_titles() {
        assert_eq!(TEAM_TEMPLATES[0].title, "Build software");
        assert_eq!(TEAM_TEMPLATES[1].title, "Research & money");
        assert_eq!(TEAM_TEMPLATES[2].title, "Write & design");
        assert_eq!(TEAM_TEMPLATES[0].teammates[0].name, "Wren");
        assert_eq!(
            fresh_starters_for(TEAM_TEMPLATES[0].teammates[0].starters).map(|s| s[0]),
            Some(
                "Make a small web page here that shows today's date and a quote, and tell me how to open it."
            )
        );
        assert!(fresh_starters_for(&["hand made"]).is_none());
    }

    #[test]
    fn home_words_exact() {
        assert_eq!(START_WITH_A_TEAM, "Start with a team");
        assert_eq!(COVER_CLAIM, "Autonomous teammates on your own machine");
        assert_eq!(COMPARE_MODELS, "Compare models");
        assert_eq!(START_FROM_ISSUE, "Start from an issue");
        assert_eq!(COMPARE, "Compare");
        assert_eq!(CONNECT_RUNTIME, "Connect a runtime to start a mission.");
        assert_eq!(START_A_CONVERSATION, "Start a conversation");
        assert_eq!(NEW_TEAMMATE, "New teammate");
        assert_eq!(PICK_ONE, "Pick one, or say what you need below.");
        assert!(template_by_id("software").is_some());
    }
}
