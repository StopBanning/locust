// Scout's away look, from Locust (https://github.com/StopBanning/locust). While your computer is off, this looks at
// this repository's pull requests, reviews, checks and issues for you, with this workflow's read-only token, and
// leaves what it found at Locust's relay, sealed with a key only your devices have. Built from
// apps/desktop/src/main/scout-away-entry.ts; Locust's pull request put it here. Delete it, the workflow
// .github/workflows/locust-scout.yml and the LOCUST_SCOUT secret to stop it.


// src/main/scout-away-entry.ts
import { execFile } from "node:child_process";
import { readFile } from "node:fs/promises";

// src/shared/away-letter.ts
var AWAY_LETTER_VERSION = 1;
var MAX_TOLD = 500;
var AWAY_ASK_KEY = "away-ask:";
var MAX_FOUND = 40;
var MAX_SAID = 400;
var REPO = /^[\w.-]{1,100}\/[\w.-]{1,100}$/;
var TONES = ["red", "amber", "green", "blue", "quiet"];
var URGENCIES = ["now", "soon", "later"];
var b64 = (bytes) => {
  let text = "";
  for (const byte of bytes) text += String.fromCharCode(byte);
  return btoa(text).replaceAll("+", "-").replaceAll("/", "_").replaceAll("=", "");
};
var unb64 = (text) => Uint8Array.from(atob(text.replaceAll("-", "+").replaceAll("_", "/")), (character) => character.charCodeAt(0));
var awayAad = (room, from) => new TextEncoder().encode(`locust-away/v1|${room}|${from}`);
var keyOf = (key) => crypto.subtle.importKey("raw", unb64(key), "AES-GCM", false, ["encrypt", "decrypt"]);
async function sealAwayLetter(key, letter, room, from) {
  const nonce = crypto.getRandomValues(new Uint8Array(12));
  const sealed = await crypto.subtle.encrypt({ name: "AES-GCM", iv: nonce, additionalData: awayAad(room, from) }, await keyOf(key), new TextEncoder().encode(JSON.stringify(letter)));
  return { n: b64(nonce), c: b64(new Uint8Array(sealed)) };
}
async function openAwayLetter(key, box, room, from) {
  let text;
  try {
    const opened = await crypto.subtle.decrypt({ name: "AES-GCM", iv: unb64(box.n), additionalData: awayAad(room, from) }, await keyOf(key), unb64(box.c));
    text = new TextDecoder().decode(opened);
  } catch {
    return void 0;
  }
  let value;
  try {
    value = JSON.parse(text);
  } catch {
    return void 0;
  }
  return awayLetterFrom(value, from);
}
var clip = (text, most) => text.length > most ? `${text.slice(0, most - 1)}…` : text;
function repoLink(repo2, url) {
  if (typeof url !== "string" || url.length > 300) return void 0;
  const prefix = `https://github.com/${repo2}/`;
  return url.toLowerCase().startsWith(prefix.toLowerCase()) && /^https:\/\/github\.com\/[\w./#?=&%-]+$/.test(url) ? url : void 0;
}
function findingFrom(value, repo2) {
  if (typeof value !== "object" || value === null) return void 0;
  const fields = value;
  if (typeof fields.key !== "string" || fields.key.length === 0 || fields.key.length > 300) return void 0;
  if (typeof fields.urgency !== "string" || !URGENCIES.includes(fields.urgency)) return void 0;
  if (typeof fields.said !== "string" || fields.said.trim() === "") return void 0;
  const lines = Array.isArray(fields.lines) ? fields.lines.filter((line) => typeof line === "string").slice(0, 8).map((line) => clip(line, 200)) : [];
  const chips = Array.isArray(fields.chips) ? fields.chips.flatMap((chip) => {
    if (typeof chip !== "object" || chip === null) return [];
    const { text, tone } = chip;
    return typeof text === "string" && text !== "" && typeof tone === "string" && TONES.includes(tone) ? [{ text: clip(text, 40), tone }] : [];
  }).slice(0, 4) : [];
  const url = repoLink(repo2, fields.url);
  return {
    key: fields.key,
    urgency: fields.urgency,
    said: clip(fields.said, MAX_SAID),
    ...typeof fields.detail === "string" && fields.detail !== "" ? { detail: clip(fields.detail, MAX_SAID) } : {},
    ...lines.length > 0 ? { lines } : {},
    ...chips.length > 0 ? { chips } : {},
    ...url === void 0 ? {} : { url }
  };
}
function pullRequestFrom(value, repo2) {
  if (typeof value !== "object" || value === null) return void 0;
  const pr = value;
  const url = repoLink(repo2, pr.url);
  if (typeof pr.number !== "number" || !Number.isInteger(pr.number) || pr.number <= 0 || url === void 0 || typeof pr.title !== "string") return void 0;
  if (!["open", "draft", "merged", "closed"].includes(String(pr.state)) || !["passing", "failing", "pending", "none"].includes(String(pr.checks))) return void 0;
  const count = (field) => typeof field === "number" && Number.isInteger(field) && field >= 0 ? field : 0;
  return {
    number: pr.number,
    title: clip(pr.title, 300),
    url,
    state: pr.state,
    checks: pr.checks,
    checkCount: count(pr.checkCount),
    notPassed: count(pr.notPassed),
    ...pr.review === "approved" || pr.review === "changes-requested" || pr.review === "review-required" ? { review: pr.review } : {},
    ...pr.conflicts === true ? { conflicts: true } : {},
    ...typeof pr.updatedAt === "string" && !Number.isNaN(Date.parse(pr.updatedAt)) ? { updatedAt: pr.updatedAt } : {}
  };
}
function awayLetterFrom(value, from) {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return void 0;
  const fields = value;
  if (fields.v !== AWAY_LETTER_VERSION || fields.kind !== "look" && fields.kind !== "told") return void 0;
  if (fields.kind === "told" !== (from === "pc")) return void 0;
  if (typeof fields.at !== "string" || Number.isNaN(Date.parse(fields.at)) || typeof fields.repo !== "string" || !REPO.test(fields.repo)) return void 0;
  const repo2 = fields.repo;
  const found = Array.isArray(fields.found) ? fields.found.slice(0, MAX_FOUND).map((one) => findingFrom(one, repo2)).filter((one) => one !== void 0) : [];
  const prs = Array.isArray(fields.prs) ? fields.prs.slice(0, 50).map((one) => pullRequestFrom(one, repo2)).filter((one) => one !== void 0) : [];
  const told = Array.isArray(fields.told) ? fields.told.filter((key) => typeof key === "string" && key.length > 0 && key.length <= 300).slice(-MAX_TOLD) : [];
  return {
    v: AWAY_LETTER_VERSION,
    kind: fields.kind,
    at: fields.at,
    repo: repo2,
    read: fields.read === true,
    found,
    prs,
    told,
    ...typeof fields.briefDay === "string" && /^\d{4}-\d{2}-\d{2}$/.test(fields.briefDay) ? { briefDay: fields.briefDay } : {}
  };
}
function findingOf(fact) {
  const url = fact.open?.kind === "url" ? fact.open.url : void 0;
  return {
    key: fact.key,
    urgency: fact.urgency,
    said: fact.said,
    ...fact.detail === void 0 ? {} : { detail: fact.detail },
    ...fact.lines === void 0 ? {} : { lines: fact.lines },
    ...fact.chips === void 0 ? {} : { chips: fact.chips },
    ...url === void 0 ? {} : { url }
  };
}

// src/shared/pull-request.ts
var PULL_REQUEST_FIELDS = "number,title,url,state,isDraft,statusCheckRollup,reviewDecision,mergeable";
var FAILED = /* @__PURE__ */ new Set(["FAILURE", "TIMED_OUT", "CANCELLED", "ACTION_REQUIRED", "STARTUP_FAILURE", "ERROR"]);
var PASSED = /* @__PURE__ */ new Set(["SUCCESS", "NEUTRAL", "SKIPPED"]);
function checkOf(check) {
  if (check.__typename === "StatusContext") {
    const state = String(check.state ?? "");
    return PASSED.has(state) ? "passed" : FAILED.has(state) ? "failed" : "pending";
  }
  if (check.status !== "COMPLETED") return "pending";
  const conclusion = String(check.conclusion ?? "");
  return PASSED.has(conclusion) ? "passed" : FAILED.has(conclusion) ? "failed" : "pending";
}
function pullRequestsOf(json) {
  let parsed;
  try {
    parsed = JSON.parse(json);
  } catch {
    return [];
  }
  return Array.isArray(parsed) ? parsed.map(pullRequestFrom2).filter((one) => one !== void 0) : [];
}
function pullRequestFrom2(parsed) {
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) return void 0;
  const pr = parsed;
  const number = typeof pr.number === "number" && Number.isInteger(pr.number) && pr.number > 0 ? pr.number : void 0;
  const url = typeof pr.url === "string" && /^https:\/\/github\.com\/[\w.-]+\/[\w.-]+\/pull\/\d+$/.test(pr.url) ? pr.url : void 0;
  if (number === void 0 || url === void 0) return void 0;
  const raw = String(pr.state ?? "").toUpperCase();
  const state = raw === "MERGED" ? "merged" : raw === "CLOSED" ? "closed" : pr.isDraft === true ? "draft" : "open";
  const checks = (Array.isArray(pr.statusCheckRollup) ? pr.statusCheckRollup : []).filter(
    (one) => typeof one === "object" && one !== null
  );
  const read = checks.map(checkOf);
  const failed = read.filter((one) => one === "failed").length;
  const pending = read.filter((one) => one === "pending").length;
  const review = pr.reviewDecision === "APPROVED" ? "approved" : pr.reviewDecision === "CHANGES_REQUESTED" ? "changes-requested" : pr.reviewDecision === "REVIEW_REQUIRED" ? "review-required" : void 0;
  return {
    number,
    title: typeof pr.title === "string" ? pr.title.replace(/\s+/g, " ").trim().slice(0, 200) : "",
    url,
    state,
    checks: read.length === 0 ? "none" : failed > 0 ? "failing" : pending > 0 ? "pending" : "passing",
    checkCount: read.length,
    notPassed: failed + pending,
    ...review === void 0 ? {} : { review },
    ...pr.mergeable === "CONFLICTING" ? { conflicts: true } : {},
    ...typeof pr.updatedAt === "string" && Number.isFinite(Date.parse(pr.updatedAt)) ? { updatedAt: pr.updatedAt } : {}
  };
}
function pullRequestNews(before, now) {
  if (before.number !== now.number) return void 0;
  const said = [];
  let needsWork = false;
  if (before.state !== now.state && (now.state === "merged" || now.state === "closed")) {
    said.push(now.state === "merged" ? "it was merged" : "it was closed");
  } else {
    if (now.checks === "failing" && before.checks !== "failing") {
      said.push(`${String(now.notPassed)} of ${String(now.checkCount)} checks did not pass`);
      needsWork = true;
    } else if (now.checks === "passing" && before.checks !== "passing" && before.checks !== "none") {
      said.push(now.checkCount === 1 ? "its check passed" : `all ${String(now.checkCount)} checks passed`);
    }
    if (now.review !== before.review && now.review === "changes-requested") {
      said.push("changes were requested");
      needsWork = true;
    } else if (now.review !== before.review && now.review === "approved") {
      said.push("it was approved");
    }
    if (now.conflicts === true && before.conflicts !== true) {
      said.push("it conflicts with its base branch");
      needsWork = true;
    }
  }
  if (said.length === 0) return void 0;
  const sentence = said.join(", and ");
  return { number: now.number, url: now.url, said: sentence.charAt(0).toUpperCase() + sentence.slice(1), needsWork };
}

// src/shared/scout.ts
var SCOUT_WAIT_MOST_MS = 30 * 6e4;
var SCOUT_WAITING_MS = 10 * 6e4;
var clip2 = (text, max) => {
  const flat = text.replace(/\s+/g, " ").trim();
  return flat.length <= max ? flat : `${flat.slice(0, max - 1).trimEnd()}…`;
};
var READY_WAITING_DAYS = 3;
function pullRequestFacts(before, now, look, at = /* @__PURE__ */ new Date()) {
  const facts = [];
  for (const pr of now) {
    const was = before.get(pr.url);
    const title = clip2(pr.title, 120);
    const open = { kind: "url", url: pr.url, label: "Open the pull request" };
    const told = facts.length;
    if (was === void 0) {
      const needs = [];
      if (pr.checks === "failing") needs.push(`${String(pr.notPassed)} of ${String(pr.checkCount)} checks did not pass`);
      if (pr.review === "changes-requested") needs.push("changes were asked for");
      if (pr.conflicts === true) needs.push("it conflicts with its base branch");
      if (needs.length > 0) {
        facts.push({
          key: `pr:${pr.url}:${pr.checks}:${pr.review ?? ""}:${pr.conflicts === true ? "c" : ""}`,
          urgency: pr.checks === "failing" ? "now" : "soon",
          said: `Your pull request #${String(pr.number)}, ${title}: ${needs.join(", and ")}.`,
          chips: [{ text: `#${String(pr.number)}`, tone: "quiet" }, ...pr.checks === "failing" ? [{ text: `${String(pr.notPassed)} of ${String(pr.checkCount)} checks not passed`, tone: "red" }] : []],
          open
        });
      }
    } else {
      const news = pullRequestNews(was, pr);
      if (news !== void 0) {
        facts.push({
          key: `pr:${pr.url}:${look}`,
          urgency: news.needsWork ? pr.checks === "failing" ? "now" : "soon" : "later",
          said: `Your pull request #${String(pr.number)}, ${title}: ${news.said.charAt(0).toLowerCase()}${news.said.slice(1)}.`,
          chips: [{ text: `#${String(pr.number)}`, tone: "quiet" }, { text: news.said, tone: news.needsWork ? "red" : "green" }],
          open
        });
      }
    }
    if (facts.length > told) continue;
    const days = pr.updatedAt === void 0 ? Number.NaN : Math.floor((at.getTime() - Date.parse(pr.updatedAt)) / 864e5);
    if (pr.state === "open" && pr.checks === "passing" && pr.review !== "changes-requested" && pr.conflicts !== true && Number.isFinite(days) && days >= READY_WAITING_DAYS) {
      facts.push({
        key: `pr:${pr.url}:ready`,
        urgency: "later",
        said: `Your pull request #${String(pr.number)}, ${title}, passes its checks and has waited ${String(days)} days. It can be merged, or closed.`,
        chips: [{ text: `#${String(pr.number)}`, tone: "quiet" }, { text: "ready", tone: "green" }],
        open
      });
    }
  }
  return facts;
}
function reviewsAskedOf(json) {
  let parsed;
  try {
    parsed = JSON.parse(json);
  } catch {
    return [];
  }
  if (!Array.isArray(parsed)) return [];
  return parsed.flatMap((one) => {
    if (typeof one !== "object" || one === null) return [];
    const pr = one;
    if (typeof pr.number !== "number" || !Number.isInteger(pr.number) || pr.number <= 0) return [];
    if (typeof pr.url !== "string" || !/^https:\/\/github\.com\/[\w.-]+\/[\w.-]+\/pull\/\d+$/.test(pr.url)) return [];
    const author = typeof pr.author === "object" && pr.author !== null && typeof pr.author.login === "string" ? clip2(String(pr.author.login), 60) : void 0;
    return [{ number: pr.number, title: typeof pr.title === "string" ? clip2(pr.title, 120) : "", url: pr.url, ...author === void 0 ? {} : { author } }];
  });
}
function reviewFacts(asked) {
  return asked.map((pr) => ({
    // By its address: another repository's #214 is another review.
    key: `review:${pr.url}`,
    urgency: "soon",
    said: `${pr.author ?? "Someone"} asked you to review pull request #${String(pr.number)}, ${pr.title}.`,
    chips: [{ text: `#${String(pr.number)}`, tone: "quiet" }, { text: "review asked", tone: "amber" }],
    open: { kind: "url", url: pr.url, label: "Open the pull request" }
  }));
}
function issuesOf(json) {
  let parsed;
  try {
    parsed = JSON.parse(json);
  } catch {
    return [];
  }
  if (!Array.isArray(parsed)) return [];
  return parsed.flatMap((one) => {
    if (typeof one !== "object" || one === null) return [];
    const issue = one;
    if (typeof issue.number !== "number" || !Number.isInteger(issue.number) || issue.number <= 0) return [];
    if (typeof issue.url !== "string" || !/^https:\/\/github\.com\/[\w.-]+\/[\w.-]+\/issues\/\d+$/.test(issue.url)) return [];
    if (typeof issue.createdAt !== "string" || !Number.isFinite(Date.parse(issue.createdAt))) return [];
    const author = typeof issue.author === "object" && issue.author !== null && typeof issue.author.login === "string" ? clip2(String(issue.author.login), 60) : void 0;
    return [{ number: issue.number, title: typeof issue.title === "string" ? clip2(issue.title, 120) : "", url: issue.url, createdAt: issue.createdAt, ...author === void 0 ? {} : { author } }];
  });
}
var NEW_ISSUE_DAYS = 7;
var ISSUES_A_LOOK = 3;
function newIssues(issues, at = /* @__PURE__ */ new Date(), told = /* @__PURE__ */ new Set()) {
  const all = [...issues].filter((issue) => at.getTime() - Date.parse(issue.createdAt) <= NEW_ISSUE_DAYS * 864e5).sort((left, right) => Date.parse(right.createdAt) - Date.parse(left.createdAt)).map(issueFact);
  const fresh = all.filter((fact) => !told.has(fact.key));
  return { facts: [...all.filter((fact) => told.has(fact.key)), ...fresh.slice(0, ISSUES_A_LOOK)], more: fresh.slice(ISSUES_A_LOOK) };
}
function issueFact(issue) {
  return {
    key: `issue:${issue.url}`,
    urgency: "soon",
    said: `${issue.author ?? "Someone"} opened issue #${String(issue.number)}: ${issue.title}.`,
    chips: [{ text: `#${String(issue.number)}`, tone: "quiet" }, { text: "new issue", tone: "amber" }],
    open: { kind: "url", url: issue.url, label: "Open the issue" }
  };
}
function branchRunsOf(json) {
  let parsed;
  try {
    parsed = JSON.parse(json);
  } catch {
    return [];
  }
  if (!Array.isArray(parsed)) return [];
  const latest = /* @__PURE__ */ new Map();
  for (const one of parsed) {
    if (typeof one !== "object" || one === null) continue;
    const run = one;
    if (run.status !== "completed" || typeof run.databaseId !== "number" || typeof run.url !== "string" || !/^https:\/\/github\.com\//.test(run.url)) continue;
    const workflow = typeof run.workflowName === "string" ? clip2(run.workflowName, 80) : "checks";
    if (latest.has(workflow)) continue;
    latest.set(workflow, { id: run.databaseId, workflow, url: run.url, failed: run.conclusion === "failure" || run.conclusion === "timed_out" || run.conclusion === "startup_failure" });
  }
  return [...latest.values()];
}
function branchFacts(branch, runs) {
  return runs.filter((run) => run.failed).map((run) => ({
    key: `branch:${String(run.id)}`,
    urgency: "soon",
    said: `${run.workflow} failed on ${clip2(branch, 60)}.`,
    chips: [{ text: clip2(branch, 60), tone: "quiet" }, { text: `${run.workflow} · failed`, tone: "red" }],
    open: { kind: "url", url: run.url, label: "Open the run" }
  }));
}
var waitedFor = (minutes) => minutes < 60 ? `${String(minutes)} min` : `${String(Math.round(minutes / 60))} h`;
var inWords = (parts) => parts.length < 2 ? parts.join("") : `${parts.slice(0, -1).join(", ")} and ${parts.at(-1)}`;
function checkedLines(brief) {
  const lines = [];
  if (brief.quiet !== void 0 && brief.quiet.length > 0) lines.push(`Checked and quiet: ${inWords(brief.quiet)}.`);
  const missed = brief.githubMissed;
  if (missed !== void 0 && missed.missed > 0) {
    lines.push(missed.missed >= missed.looks ? "GitHub could not be read since the last brief, so nothing there is called quiet." : `GitHub could not be read on ${String(missed.missed)} of ${String(missed.looks)} looks.`);
  }
  if (brief.setAside !== void 0 && brief.setAside > 0) lines.push(`Set aside: ${String(brief.setAside)} thing${brief.setAside === 1 ? "" : "s"} Scout had already told you.`);
  for (const why of ["quiet hours", "daily limit"]) {
    const titles = (brief.held ?? []).filter((one) => one.why === why).map((one) => one.title);
    if (titles.length === 0) continue;
    const counted = [...new Set(titles)].map((title) => {
      const times = titles.filter((one) => one === title).length;
      return times === 1 ? title : `${title} (${String(times)})`;
    });
    lines.push(`Not sent to your phone, ${why === "quiet hours" ? "in quiet hours" : "past the day’s limit"}: ${counted.slice(0, 4).join("; ")}${counted.length > 4 ? `; ${String(counted.length - 4)} more` : ""}.`);
  }
  return lines;
}
function briefFact(brief, day) {
  const span = brief.span ?? "Since yesterday";
  const lines = [];
  if (brief.waiting.length > 0) lines.push(`Waiting on you: ${brief.waiting.slice(0, 4).map((one) => `${one.name ?? "a teammate"}, ${waitedFor(one.minutes)}`).join("; ")}.`);
  if (brief.finished + brief.failed > 0) {
    lines.push(`${span}: ${brief.more === true ? "at least " : ""}${String(brief.finished)} turn${brief.finished === 1 ? "" : "s"} finished${brief.failed === 0 ? "" : `, ${String(brief.failed)} failed`}.`);
  }
  if (brief.check !== void 0) {
    lines.push(brief.check.stale === true ? `The check ${clip2(brief.check.command, 80)} ${brief.check.passed ? "passed" : "failed"} when it last ran; the folder has changed since.` : `The check ${clip2(brief.check.command, 80)} ${brief.check.passed ? "passes" : "fails"}.`);
  }
  if (brief.prs !== void 0 && brief.prs.open > 0) lines.push(`Your pull requests: ${String(brief.prs.open)} open${brief.prs.needsWork === 0 ? "" : `, ${String(brief.prs.needsWork)} needing work`}.`);
  if (brief.cards.total > 0) lines.push(`Scout has ${String(brief.cards.total)} more card${brief.cards.total === 1 ? "" : "s"} for you${brief.cards.now === 0 ? "" : `, ${String(brief.cards.now)} needing you now`}.`);
  const pressing = brief.waiting.length > 0 || brief.failed > 0 || brief.check?.passed === false || brief.cards.now > 0;
  const news = lines.length > 0;
  lines.push(...checkedLines(brief));
  return {
    key: `brief:${day}`,
    urgency: pressing ? "soon" : "later",
    said: news ? "Good morning. Here is where things stand." : `Good morning. Nothing waits on you, and nothing ran ${span.replace(/^Since/, "since")}.`,
    ...lines.length === 0 ? {} : { lines, detail: lines.join("\n") },
    chips: [{ text: "morning brief", tone: "blue" }]
  };
}

// src/main/scout-github.ts
var AWAY_WORKFLOW_NAME = "Locust Scout";
function githubUnreadWhy(error) {
  const code = typeof error === "object" && error !== null ? error.code : void 0;
  if (code === "ENOENT") return "no-cli";
  const said = error instanceof Error ? error.message : String(error);
  if (/not a git repository|no git remotes|none of the git remotes|not a github repository|could not determine (?:the )?(?:base )?repo/i.test(said)) return "not-on-github";
  if (/gh auth login|not logged in|authentication (?:required|failed)|HTTP 401|bad credentials/i.test(said)) return "signed-out";
  if (/is not recognized as an internal or external command|command not found/i.test(said)) return "no-cli";
  return "unreadable";
}
async function readScoutGithub(folder, gh, who = "@me", unread) {
  const mine = await gh(["pr", "list", "--author", who, "--state", "open", "--limit", "30", "--json", `${PULL_REQUEST_FIELDS},updatedAt`], folder).catch((error) => {
    unread?.(githubUnreadWhy(error));
    return void 0;
  });
  if (mine === void 0) return void 0;
  const reviews = await gh(["pr", "list", "--search", `review-requested:${who}`, "--state", "open", "--limit", "20", "--json", "number,title,url,author"], folder).catch(() => "[]");
  const branch = (await gh(["repo", "view", "--json", "defaultBranchRef", "--jq", ".defaultBranchRef.name"], folder).catch(() => "")).trim();
  const runs = /^[\w./-]{1,200}$/.test(branch) ? await gh(["run", "list", "--branch", branch, "--limit", "30", "--json", "databaseId,conclusion,status,workflowName,url"], folder).catch(() => "[]") : void 0;
  const issues = await gh(["issue", "list", "--state", "open", "--limit", "20", "--json", "number,title,url,createdAt,author"], folder).catch(() => "[]");
  return {
    prs: pullRequestsOf(mine),
    reviews: reviewsAskedOf(reviews),
    ...runs === void 0 ? {} : { branch: { name: branch, runs: branchRunsOf(runs).filter((run) => run.workflow !== AWAY_WORKFLOW_NAME) } },
    issues: issuesOf(issues)
  };
}

// src/main/scout-away-look.ts
function awayConfigOf(text) {
  if (text === void 0 || text.trim() === "") return void 0;
  let value;
  try {
    value = JSON.parse(text);
  } catch {
    return void 0;
  }
  if (typeof value !== "object" || value === null) return void 0;
  const fields = value;
  const hour = (field) => typeof field === "number" && Number.isInteger(field) && field >= 0 && field <= 23;
  if (fields.v !== 1 || typeof fields.relay !== "string" || !/^https?:\/\/[^\s/]+$/.test(fields.relay)) return void 0;
  if (typeof fields.room !== "string" || !/^[A-Za-z0-9_-]{22}$/.test(fields.room) || typeof fields.token !== "string" || !/^[A-Za-z0-9_-]{43}$/.test(fields.token)) return void 0;
  if (typeof fields.key !== "string" || !/^[A-Za-z0-9_-]{43}$/.test(fields.key) || typeof fields.login !== "string" || !/^[A-Za-z0-9-]{1,39}$/.test(fields.login)) return void 0;
  if (typeof fields.tz !== "string" || fields.tz.length > 64) return void 0;
  const quiet = typeof fields.quiet === "object" && fields.quiet !== null ? fields.quiet : void 0;
  return {
    v: 1,
    relay: fields.relay,
    room: fields.room,
    token: fields.token,
    key: fields.key,
    login: fields.login,
    tz: fields.tz,
    ...quiet !== void 0 && hour(quiet.from) && hour(quiet.to) ? { quiet: { from: quiet.from, to: quiet.to } } : {},
    ...hour(fields.briefAt) ? { briefAt: fields.briefAt } : {}
  };
}
function localTime(at, tz) {
  let parts;
  try {
    parts = new Intl.DateTimeFormat("en-CA", { timeZone: tz, year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", hourCycle: "h23" }).formatToParts(at);
  } catch {
    parts = new Intl.DateTimeFormat("en-CA", { timeZone: "UTC", year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", hourCycle: "h23" }).formatToParts(at);
  }
  const part = (type) => parts.find((one) => one.type === type)?.value ?? "0";
  return { day: `${part("year")}-${part("month")}-${part("day")}`, hour: Number(part("hour")) % 24 };
}
var quietAt = (quiet, hour) => quiet !== void 0 && quiet.from !== quiet.to && (quiet.from < quiet.to ? hour >= quiet.from && hour < quiet.to : hour >= quiet.from || hour < quiet.to);
async function askRelay(deps, base, headers, open) {
  const { config: config2 } = deps;
  let asked;
  try {
    asked = await deps.fetch(base, { method: "GET", headers });
  } catch {
    return { said: "Locust’s relay could not be reached. The next look tries again.", ok: false };
  }
  if (asked.status === 403) return { said: "Scout’s away look is off in Locust. Delete .github/workflows/locust-scout.yml and the LOCUST_SCOUT secret to stop these runs.", ok: true };
  if (asked.status === 401 || asked.status === 404) return { said: "Locust’s relay did not know this look’s token. Turn “Look while my computer is off” off and on again in Locust.", ok: false };
  if (!asked.ok) return { said: `Locust’s relay answered ${String(asked.status)}. The next look tries again.`, ok: false };
  const answer = await asked.json().catch(() => ({}));
  if (answer.pc === true) return { said: open, ok: true };
  const from = answer.latest?.from === "pc" ? "pc" : "away";
  return { before: answer.latest === void 0 ? void 0 : await openAwayLetter(config2.key, answer.latest, config2.room, from) };
}
async function awayLook(deps) {
  const { config: config2 } = deps;
  const at = (deps.now ?? (() => /* @__PURE__ */ new Date()))();
  const base = `${config2.relay}/v1/room/${config2.room}/away`;
  const headers = { authorization: `Bearer ${config2.token}`, "content-type": "application/json" };
  const relayed = await askRelay(deps, base, headers, "Locust is open on your computer, so Scout looks from there.");
  if ("said" in relayed) return relayed;
  const { before } = relayed;
  const told = new Set(before?.told ?? []);
  const local = localTime(at, config2.tz);
  const quiet = quietAt(config2.quiet, local.hour);
  const github = await readScoutGithub(".", (args) => deps.gh(args), config2.login).catch(() => void 0);
  const stamp = at.toISOString();
  let found = [];
  let prs = before?.prs ?? [];
  if (github !== void 0) {
    const facts = [
      ...pullRequestFacts(new Map(prs.map((pr) => [pr.url, pr])), github.prs, stamp, at),
      ...reviewFacts(github.reviews),
      ...github.branch === void 0 ? [] : branchFacts(github.branch.name, github.branch.runs),
      ...newIssues(github.issues ?? [], at, told).facts
    ];
    found = facts.filter((fact, index) => !told.has(fact.key) && facts.findIndex((other) => other.key === fact.key) === index).slice(0, MAX_FOUND);
    prs = github.prs;
  }
  const first = before === void 0;
  if (first) {
    for (const fact of found) told.add(fact.key);
    found = [];
  }
  const briefDue = config2.briefAt !== void 0 && local.hour >= config2.briefAt && before?.briefDay !== local.day && !quiet;
  if (briefDue) {
    const needsWork = (pr) => pr.checks === "failing" || pr.review === "changes-requested" || pr.conflicts === true;
    const open = prs.filter((pr) => pr.state === "open");
    const brief = briefFact({
      waiting: [],
      finished: 0,
      failed: 0,
      ...github !== void 0 && open.length > 0 ? { prs: { open: open.length, needsWork: open.filter(needsWork).length } } : {},
      cards: { total: found.length, now: found.filter((fact) => fact.urgency === "now").length },
      ...github === void 0 ? { githubMissed: { missed: 1, looks: 1 } } : found.length === 0 && !open.some(needsWork) ? { quiet: ["GitHub"] } : {}
    }, local.day);
    const lines = [...brief.lines ?? [], "Your computer is off, so Scout looked at GitHub only."];
    found = [...found, { ...brief, said: brief.said.replace(/, and nothing ran since yesterday\./, "."), lines, detail: lines.join("\n") }];
  }
  const letter = {
    v: 1,
    kind: "look",
    at: stamp,
    repo: deps.repo,
    read: github !== void 0,
    found: found.map(findingOf),
    prs,
    told: [...told, ...found.map((fact) => fact.key)].slice(-MAX_TOLD),
    ...briefDue ? { briefDay: local.day } : before?.briefDay === void 0 ? {} : { briefDay: before.briefDay }
  };
  const pressing = found.some((fact) => fact.urgency === "now");
  const alert = quiet ? void 0 : briefDue ? "brief" : pressing ? "found" : void 0;
  const box = await sealAwayLetter(config2.key, letter, config2.room, "away");
  let posted;
  try {
    posted = await deps.fetch(base, { method: "POST", headers, body: JSON.stringify({ ...box, ...alert === void 0 ? {} : { alert } }) });
  } catch {
    return { said: "Scout looked, but Locust’s relay could not be reached to leave what it found.", ok: false };
  }
  if (!posted.ok) return { said: `Scout looked, but Locust’s relay answered ${String(posted.status)}.`, ok: false };
  const rung = Number((await posted.json().catch(() => ({}))).rung ?? 0);
  const news = found.filter((fact) => !fact.key.startsWith("brief:")).length;
  return {
    said: [
      github === void 0 ? "Scout could not read GitHub this time." : first ? "Scout’s first away look: it noted what is open now, and tells only what changes." : news === 0 ? "Scout looked: nothing new." : `Scout looked: ${String(news)} new thing${news === 1 ? "" : "s"}.`,
      ...briefDue ? ["It brought the morning brief."] : [],
      ...rung > 0 ? [`It rang ${String(rung)} phone${rung === 1 ? "" : "s"}.`] : quiet && (pressing || briefDue) ? ["Quiet hours: no phone was rung."] : []
    ].join(" "),
    ok: true
  };
}
function awayCommentOf(event) {
  if (typeof event !== "object" || event === null) return void 0;
  const { action, comment, issue } = event;
  if (action !== "created" || typeof comment !== "object" || comment === null || typeof issue !== "object" || issue === null) return void 0;
  const login = comment.user?.login;
  if (typeof comment.id !== "number" || !Number.isSafeInteger(comment.id) || typeof login !== "string" || typeof comment.body !== "string" || typeof comment.html_url !== "string") return void 0;
  if (typeof issue.number !== "number" || !Number.isSafeInteger(issue.number) || issue.number < 1) return void 0;
  return { id: comment.id, login, body: comment.body, url: comment.html_url, number: issue.number, title: typeof issue.title === "string" ? issue.title : "" };
}
function askOf(body) {
  const match = /^\/locust(?:[ \t]+|(?=\r?$))/m.exec(body);
  if (match === null) return void 0;
  const task = body.slice(match.index + match[0].length).trim();
  return task.length === 0 ? void 0 : task;
}
async function awayAsk(deps) {
  const { config: config2, comment } = deps;
  if (comment.login.toLowerCase() !== config2.login.toLowerCase()) return { said: "This comment is not yours, so Scout leaves it.", ok: true };
  const task = askOf(comment.body);
  if (task === void 0) return { said: "This comment asks Locust nothing.", ok: true };
  if (!new RegExp(`^https://github\\.com/${deps.repo.replace(/\./g, "\\.")}/(?:issues|pull)/${String(comment.number)}#issuecomment-${String(comment.id)}$`).test(comment.url)) return { said: "This comment’s link is not this repository’s, so Scout leaves it.", ok: true };
  const at = (deps.now ?? (() => /* @__PURE__ */ new Date()))();
  const base = `${config2.relay}/v1/room/${config2.room}/away`;
  const headers = { authorization: `Bearer ${config2.token}`, "content-type": "application/json" };
  const relayed = await askRelay(deps, base, headers, "Locust is open on your computer, so it reads your ask from there.");
  if ("said" in relayed) return relayed;
  const { before } = relayed;
  const key = `${AWAY_ASK_KEY}${deps.repo}:${String(comment.id)}`;
  const told = before?.told ?? [];
  if (told.includes(key)) return { said: "Your phone has this ask already.", ok: true };
  const line = task.split(/\r?\n/)[0].trim();
  const fact = {
    key,
    urgency: "now",
    said: `You asked on #${String(comment.number)}${comment.title === "" ? "" : ` ${comment.title}`}: ${line.length > 140 ? `${line.slice(0, 139)}…` : line}`,
    detail: task,
    lines: ["Your computer is off: your ask waits on GitHub, and becomes a card for a teammate when Locust is open again."],
    chips: [{ text: "while your PC was off", tone: "quiet" }],
    open: { kind: "url", url: comment.url, label: "Open your comment" }
  };
  const letter = {
    v: 1,
    kind: "look",
    at: at.toISOString(),
    repo: deps.repo,
    read: true,
    found: [findingOf(fact)],
    prs: before?.prs ?? [],
    told: [...told, key].slice(-MAX_TOLD),
    ...before?.briefDay === void 0 ? {} : { briefDay: before.briefDay }
  };
  const quiet = quietAt(config2.quiet, localTime(at, config2.tz).hour);
  const box = await sealAwayLetter(config2.key, letter, config2.room, "away");
  let posted;
  try {
    posted = await deps.fetch(base, { method: "POST", headers, body: JSON.stringify({ ...box, ...quiet ? {} : { alert: "found" } }) });
  } catch {
    return { said: "Locust’s relay could not be reached to pass your ask on.", ok: false };
  }
  if (!posted.ok) return { said: `Locust’s relay answered ${String(posted.status)}, so your ask was not passed on.`, ok: false };
  const rung = Number((await posted.json().catch(() => ({}))).rung ?? 0);
  return { said: `Scout passed your ask on.${rung > 0 ? ` It rang ${String(rung)} phone${rung === 1 ? "" : "s"}.` : quiet ? " Quiet hours: no phone was rung." : ""}`, ok: true };
}

// src/main/scout-away-entry.ts
var config = awayConfigOf(process.env.LOCUST_SCOUT);
var repo = process.env.GH_REPO ?? process.env.GITHUB_REPOSITORY ?? "";
if (config === void 0) {
  console.log("The LOCUST_SCOUT secret is missing or cannot be read, so Scout does not look. Turn “Look while my computer is off” on in Locust to set it.");
} else if (!/^[\w.-]+\/[\w.-]+$/.test(repo)) {
  console.log("This look does not know its repository.");
  process.exitCode = 1;
} else {
  const gh = (args) => new Promise((resolve, reject) => {
    execFile("gh", [...args], { env: { ...process.env, GH_REPO: repo, GH_PROMPT_DISABLED: "1", NO_COLOR: "1" }, maxBuffer: 16 * 1024 * 1024, timeout: 6e4, windowsHide: true }, (error, stdout) => {
      if (error) reject(error);
      else resolve(stdout);
    });
  });
  let result;
  if (process.env.GITHUB_EVENT_NAME === "issue_comment") {
    const event = await readFile(process.env.GITHUB_EVENT_PATH ?? "", "utf8").then((text) => JSON.parse(text), () => void 0);
    const comment = awayCommentOf(event);
    result = comment === void 0 ? { said: "This run is not about a comment that was written.", ok: true } : await awayAsk({ config, repo, gh, fetch, comment });
  } else {
    result = await awayLook({ config, repo, gh, fetch });
  }
  console.log(result.said);
  if (!result.ok) process.exitCode = 1;
}
