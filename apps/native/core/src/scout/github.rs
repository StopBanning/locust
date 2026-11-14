//! What Scout reads from GitHub through the person's own `gh` CLI in the folder.
//!
//! Port of Electron's `main/scout-github.ts`.

use std::path::Path;

use crate::invariants::regex;
use crate::pull_requests::{FolderPullRequest, PULL_REQUEST_FIELDS, pull_requests_of};

use super::types::{BranchRun, IssueOpened, ReviewAsked, branch_runs_of, issues_of, reviews_asked_of};

/// The workflow Scout's away look runs as, in the watched repository (0.829).
pub const AWAY_WORKFLOW_NAME: &str = "Locust Scout";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScoutGithubBranch {
    pub name: String,
    pub runs: Vec<BranchRun>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScoutGithub {
    pub prs: Vec<FolderPullRequest>,
    pub reviews: Vec<ReviewAsked>,
    pub branch: Option<ScoutGithubBranch>,
    pub issues: Vec<IssueOpened>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScoutGithubUnread {
    NotOnGithub,
    SignedOut,
    NoCli,
    Unreadable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GhError {
    NotFound(String),
    Failed(String),
}

impl GhError {
    pub fn message(&self) -> &str {
        match self {
            Self::NotFound(m) | Self::Failed(m) => m,
        }
    }
}

/// Classifies a failure from running `gh`.
pub fn github_unread_why(error: &GhError) -> ScoutGithubUnread {
    if matches!(error, GhError::NotFound(_)) {
        return ScoutGithubUnread::NoCli;
    }
    let said = error.message();
    if regex(
        r"(?i)not a git repository|no git remotes|none of the git remotes|not a github repository|could not determine (?:the )?(?:base )?repo",
    )
    .is_match(said)
    {
        return ScoutGithubUnread::NotOnGithub;
    }
    if regex(r"(?i)gh auth login|not logged in|authentication (?:required|failed)|HTTP 401|bad credentials")
        .is_match(said)
    {
        return ScoutGithubUnread::SignedOut;
    }
    if regex(r"(?i)is not recognized as an internal or external command|command not found")
        .is_match(said)
    {
        return ScoutGithubUnread::NoCli;
    }
    ScoutGithubUnread::Unreadable
}

/// Reads open PRs, review requests, default-branch workflow runs (excluding `AWAY_WORKFLOW_NAME`),
/// and recent open issues from `folder` using `gh`.
pub fn read_scout_github<F>(
    folder: &Path,
    mut gh: F,
    who: Option<&str>,
    mut unread: Option<&mut dyn FnMut(ScoutGithubUnread)>,
) -> Option<ScoutGithub>
where
    F: FnMut(&[&str], &Path) -> Result<String, GhError>,
{
    let who = who.unwrap_or("@me");
    let pr_fields = format!("{PULL_REQUEST_FIELDS},updatedAt");
    let mine = match gh(
        &[
            "pr",
            "list",
            "--author",
            who,
            "--state",
            "open",
            "--limit",
            "30",
            "--json",
            &pr_fields,
        ],
        folder,
    ) {
        Ok(out) => out,
        Err(err) => {
            if let Some(cb) = unread.as_deref_mut() {
                cb(github_unread_why(&err));
            }
            return None;
        }
    };
    let review_query = format!("review-requested:{who}");
    let reviews = gh(
        &[
            "pr",
            "list",
            "--search",
            &review_query,
            "--state",
            "open",
            "--limit",
            "20",
            "--json",
            "number,title,url,author",
        ],
        folder,
    )
    .unwrap_or_else(|_| "[]".to_owned());
    let branch_name = gh(
        &[
            "repo",
            "view",
            "--json",
            "defaultBranchRef",
            "--jq",
            ".defaultBranchRef.name",
        ],
        folder,
    )
    .unwrap_or_default()
    .trim()
    .to_owned();
    let branch = if regex(r"^[\w./-]{1,200}$").is_match(&branch_name) {
        let runs_json = gh(
            &[
                "run",
                "list",
                "--branch",
                &branch_name,
                "--limit",
                "30",
                "--json",
                "databaseId,conclusion,status,workflowName,url",
            ],
            folder,
        )
        .unwrap_or_else(|_| "[]".to_owned());
        let runs = branch_runs_of(&runs_json)
            .into_iter()
            .filter(|run| run.workflow != AWAY_WORKFLOW_NAME)
            .collect();
        Some(ScoutGithubBranch {
            name: branch_name,
            runs,
        })
    } else {
        None
    };
    let issues_json = gh(
        &[
            "issue",
            "list",
            "--state",
            "open",
            "--limit",
            "20",
            "--json",
            "number,title,url,createdAt,author",
        ],
        folder,
    )
    .unwrap_or_else(|_| "[]".to_owned());
    Some(ScoutGithub {
        prs: pull_requests_of(&mine),
        reviews: reviews_asked_of(&reviews),
        branch,
        issues: issues_of(&issues_json),
    })
}
