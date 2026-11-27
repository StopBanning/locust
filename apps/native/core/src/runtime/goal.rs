//! A Codex teammate's standing goal: Codex app-server's `thread/goal/set|get|clear` requests and its
//! `thread/goal/updated|cleared` notifications. Engine only; the card that sets and clears one is on the shelf (after the
//! Dots/Duet study).
use serde_json::{Value, json};

/// A goal's status as Codex says it. One Codex names later reads as `Unknown`, never as a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalStatus {
    Active,
    Paused,
    Blocked,
    UsageLimited,
    BudgetLimited,
    Complete,
    Unknown,
}

impl GoalStatus {
    pub fn parse(said: &str) -> Self {
        match said {
            "active" => GoalStatus::Active,
            "paused" => GoalStatus::Paused,
            "blocked" => GoalStatus::Blocked,
            "usageLimited" => GoalStatus::UsageLimited,
            "budgetLimited" => GoalStatus::BudgetLimited,
            "complete" => GoalStatus::Complete,
            _ => GoalStatus::Unknown,
        }
    }
    /// The protocol's word for it (`unknown` for one Locust does not know).
    pub fn as_str(self) -> &'static str {
        match self {
            GoalStatus::Active => "active",
            GoalStatus::Paused => "paused",
            GoalStatus::Blocked => "blocked",
            GoalStatus::UsageLimited => "usageLimited",
            GoalStatus::BudgetLimited => "budgetLimited",
            GoalStatus::Complete => "complete",
            GoalStatus::Unknown => "unknown",
        }
    }
}

/// The protocol's `ThreadGoal`: {threadId, objective, status, tokenBudget?, tokensUsed, timeUsedSeconds, createdAt,
/// updatedAt}.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadGoal {
    pub thread_id: String,
    pub objective: String,
    pub status: GoalStatus,
    pub token_budget: Option<u64>,
    pub tokens_used: u64,
    pub time_used_seconds: u64,
    pub created_at: i64,
    pub updated_at: i64,
}

impl ThreadGoal {
    /// A `ThreadGoal` as the server sent one; `thread` stands in for a goal that leaves its thread out. No objective,
    /// no goal.
    pub fn parse(goal: &Value, thread: Option<&str>) -> Option<Self> {
        let thread_id = goal["threadId"].as_str().or(thread).filter(|t| !t.is_empty())?.to_owned();
        let objective = goal["objective"].as_str()?.to_owned();
        Some(Self {
            thread_id,
            objective,
            status: GoalStatus::parse(goal["status"].as_str().unwrap_or("")),
            token_budget: goal["tokenBudget"].as_u64(),
            tokens_used: goal["tokensUsed"].as_u64().unwrap_or(0),
            time_used_seconds: goal["timeUsedSeconds"].as_u64().unwrap_or(0),
            created_at: goal["createdAt"].as_i64().unwrap_or(0),
            updated_at: goal["updatedAt"].as_i64().unwrap_or(0),
        })
    }
}

/// What a goal notification says happened on a thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoalEvent {
    Updated(ThreadGoal),
    Cleared { thread_id: String },
}

impl GoalEvent {
    pub fn thread_id(&self) -> &str {
        match self {
            GoalEvent::Updated(goal) => &goal.thread_id,
            GoalEvent::Cleared { thread_id } => thread_id,
        }
    }
}

/// `thread/goal/updated` {threadId, goal} or `thread/goal/cleared` {threadId}; anything else, or one that does not
/// read, is nothing.
pub fn from_notification(method: &str, params: &Value) -> Option<GoalEvent> {
    let thread = params["threadId"].as_str().filter(|t| !t.is_empty());
    match method {
        "thread/goal/updated" => ThreadGoal::parse(&params["goal"], thread).map(GoalEvent::Updated),
        "thread/goal/cleared" => Some(GoalEvent::Cleared { thread_id: thread?.to_owned() }),
        _ => None,
    }
}

/// A goal the person sets: only what is given is sent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GoalSet {
    pub objective: Option<String>,
    pub status: Option<GoalStatus>,
    pub token_budget: Option<u64>,
}

/// What is asked of a runtime's standing goal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoalAsk {
    Set(GoalSet),
    Get,
    Clear,
}

/// What a runtime answers: never a turn-ending error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoalAnswer {
    /// The goal the thread holds, none when it holds none.
    Held(Option<ThreadGoal>),
    Cleared,
    /// A runtime, or a Codex, that keeps no standing goals.
    NoStandingGoals,
    /// Asked before the runtime has a thread to hold one on.
    NoThreadYet,
    /// Codex said no, in its words.
    Refused(String),
}

/// How a goal's status is said in its ledger event: "Codex's standing goal {words}: {objective}".
pub fn status_words(status: GoalStatus) -> &'static str {
    match status {
        GoalStatus::Active => "is active",
        GoalStatus::Paused => "is paused",
        GoalStatus::Blocked => "is blocked",
        GoalStatus::UsageLimited => "stopped at a usage limit",
        GoalStatus::BudgetLimited => "ran out of its token budget",
        GoalStatus::Complete => "is complete",
        GoalStatus::Unknown => "is in a state Locust does not know",
    }
}

/// `thread/goal/set`'s params: the thread, what the person gave, and `origin: "user"`. A status Locust does not know is
/// not sent.
pub fn set_params(thread: &str, set: &GoalSet) -> Value {
    let mut params = json!({"threadId": thread});
    if let Some(objective) = &set.objective {
        params["objective"] = json!(objective);
    }
    if let Some(status) = set.status.filter(|s| *s != GoalStatus::Unknown) {
        params["status"] = json!(status.as_str());
    }
    if let Some(budget) = set.token_budget {
        params["tokenBudget"] = json!(budget);
    }
    params["origin"] = json!("user");
    params
}

/// A refusal that says the server does not know the request: JSON-RPC's "method not found" (-32601), or Codex's
/// `unknown variant \`thread/goal/..\`` (its words for a request it does not know, as MEASURED of `config/read` on
/// Codex 0.163.0).
pub fn not_known(refused: &Value) -> bool {
    let said = refused["message"].as_str().unwrap_or("");
    refused["code"].as_i64() == Some(-32601) || said.contains("unknown variant `thread/goal/") || said.eq_ignore_ascii_case("method not found")
}

#[cfg(test)]
#[path = "goal_tests.rs"]
mod goal_tests;
