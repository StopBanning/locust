//! THE JUDGE'S SCORECARD (Electron `shared/compare-score.ts`):
//! weighted/drafted rubric points and one/three blind judges.

use crate::compare::CompareSlotId;
use std::collections::BTreeMap;

pub const MAX_SCORE_POINTS: usize = 8;
const MAX_POINT_CHARS: usize = 160;
const MAX_WHY_CHARS: usize = 160;
pub const JUDGE_POINTS: usize = 3;
pub const DRAFT_MIN_POINTS: usize = 3;
pub const DRAFT_MAX_POINTS: usize = 5;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScoreVerdict {
    pub met: bool,
    pub why: String,
    pub votes: Option<(usize, usize)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scorecard {
    pub points: Vec<String>,
    pub by_judge: bool,
    pub verdicts: BTreeMap<String, Vec<Option<ScoreVerdict>>>,
    pub weights: Option<Vec<usize>>,
}

fn clip(text: &str, max: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let count = flat.chars().count();
    if count <= max {
        flat
    } else {
        let prefix: String = flat.chars().take(max.saturating_sub(1)).collect();
        format!("{}…", prefix.trim_end())
    }
}

fn strip_weight(line: &str) -> (&str, Option<usize>) {
    let re = crate::invariants::regex(r"(?i)(?:\s*\(\s*[x×]\s*([2-5])\s*\)|(?:^|\s+)[x×]\s*([2-5]))\s*$");
    if let Some(m) = re.find(line)
        && let Some(caps) = re.captures(line)
    {
        let w = caps
            .get(1)
            .or_else(|| caps.get(2))
            .and_then(|c| c.as_str().parse::<usize>().ok());
        return (&line[..m.start()], w);
    }
    (line, None)
}

fn point_lines(criteria: Option<&str>) -> Vec<String> {
    let Some(raw) = criteria else {
        return Vec::new();
    };
    let bullet_re = crate::invariants::regex(r"^\s*(?:[-*•]|\d{1,2}[.)])\s+");
    raw.split('\n')
        .map(|l| l.trim_end_matches('\r'))
        .map(|l| bullet_re.replace(l, "").trim().to_owned())
        .filter(|l| !strip_weight(l).0.trim().is_empty())
        .take(MAX_SCORE_POINTS)
        .collect()
}

pub fn criteria_lines(criteria: Option<&str>) -> Vec<String> {
    point_lines(criteria)
        .into_iter()
        .map(|line| clip(strip_weight(&line).0.trim(), MAX_POINT_CHARS))
        .collect()
}

pub fn criteria_weights(criteria: Option<&str>) -> Vec<usize> {
    point_lines(criteria)
        .into_iter()
        .map(|line| strip_weight(&line).1.unwrap_or(1))
        .collect()
}

pub fn weighted(card: Option<Scorecard>, weights: &[usize]) -> Option<Scorecard> {
    let mut card = card?;
    if card.by_judge || weights.len() != card.points.len() || weights.iter().all(|&w| w == 1) {
        return Some(card);
    }
    card.weights = Some(weights.to_vec());
    Some(card)
}

pub fn scorecard_request(points: &[String], letters: &[&str]) -> String {
    let first = letters.first().copied().unwrap_or("A");
    let second = letters.get(1).copied().unwrap_or(first);
    let form = format!(
        "Score every answer on every point, one line each, in exactly this form and with nothing else on the line:\nSCORE {first} 1: met. <why, in at most twelve words>\nSCORE {second} 1: not met. <why, in at most twelve words>"
    );
    if !points.is_empty() {
        let numbered = points
            .iter()
            .enumerate()
            .map(|(i, p)| format!("{}. {p}", i + 1))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "Last, after your view, a scorecard. The points to score are what the person says a good answer does:\n\n{numbered}\n\n{form}"
        )
    } else {
        let own = (1..=JUDGE_POINTS)
            .map(|i| format!("POINT {i}: <a point, in at most ten words>"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut form_lower = form;
        if let Some(first_ch) = form_lower.get_mut(0..1) {
            first_ch.make_ascii_lowercase();
        }
        format!(
            "Last, after your view, a scorecard. First set {JUDGE_POINTS} points a good answer to this request must meet, one line each, in exactly this form:\n\n{own}\n\nThen {form_lower}"
        )
    }
}

fn plain(line: &str) -> String {
    let stripped = line.replace(['*', '_', '`'], "");
    let re = crate::invariants::regex(r"^\s*(?:[-•>]\s*)+");
    re.replace(&stripped, "").trim().to_owned()
}

fn is_point_line(bare: &str) -> Option<(usize, String)> {
    let re = crate::invariants::regex(r"(?i)^POINT\s*(\d{1,2})\s*[:.\-–—]\s*(.+)$");
    let caps = re.captures(bare)?;
    let num = caps.get(1)?.as_str().parse::<usize>().ok()?;
    let body = caps.get(2)?.as_str().to_owned();
    Some((num, body))
}

fn parse_score_line(bare: &str) -> Option<(String, usize, bool, String)> {
    let re = crate::invariants::regex(
        r"(?i)^SCORE\s*:?\s*(?:answer\s+)?([a-z])\s*[,:]?\s*(?:point\s*)?(\d{1,2})\s*[:.\-–—]\s*(not\s+met|met)\b[\s.:;,\-–—]*(.*)$",
    );
    let caps = re.captures(bare)?;
    let slot = caps.get(1)?.as_str().to_ascii_lowercase();
    let num = caps.get(2)?.as_str().parse::<usize>().ok()?;
    let met = !caps.get(3)?.as_str().to_ascii_lowercase().starts_with("not");
    let why = caps.get(4)?.as_str().to_owned();
    Some((slot, num, met, why))
}

fn is_heading_line(bare: &str) -> bool {
    crate::invariants::regex(r"(?i)^#{0,6}\s*(?:the\s+)?score\s*card\s*:?$").is_match(bare)
}

fn is_being_written(bare: &str) -> bool {
    crate::invariants::regex(r"^(?:SCORE|POINT)\b").is_match(bare)
}

pub fn parse_scorecard(text: &str, slots: &[&str], given: &[String]) -> Option<Scorecard> {
    let lines: Vec<String> = text.split('\n').map(|l| plain(l.trim_end_matches('\r'))).collect();
    let by_judge = given.is_empty();
    let points: Vec<String> = if by_judge {
        let mut set = Vec::new();
        for line in &lines {
            if let Some((at, body)) = is_point_line(line)
                && at == set.len() + 1
                && set.len() < MAX_SCORE_POINTS
            {
                set.push(clip(&body, MAX_POINT_CHARS));
            }
        }
        set.into_iter().filter(|p| !p.is_empty()).collect()
    } else {
        given.to_vec()
    };
    if points.is_empty() {
        return None;
    }
    let mut verdicts: BTreeMap<String, Vec<Option<ScoreVerdict>>> = slots
        .iter()
        .map(|&s| (s.to_owned(), vec![None; points.len()]))
        .collect();
    let mut read = 0usize;
    for line in &lines {
        let Some((slot, num, met, why)) = parse_score_line(line) else {
            continue;
        };
        if num == 0 || num > points.len() {
            continue;
        }
        let at = num - 1;
        let Some(row) = verdicts.get_mut(&slot) else {
            continue;
        };
        if row[at].is_some() {
            continue;
        }
        row[at] = Some(ScoreVerdict {
            met,
            why: clip(&why, MAX_WHY_CHARS),
            votes: None,
        });
        read += 1;
    }
    if read == 0 {
        None
    } else {
        Some(Scorecard {
            points,
            by_judge,
            verdicts,
            weights: None,
        })
    }
}

pub fn without_scorecard(text: &str) -> String {
    let kept: Vec<&str> = text
        .split('\n')
        .map(|l| l.trim_end_matches('\r'))
        .filter(|line| {
            let bare = plain(line);
            is_point_line(&bare).is_none()
                && parse_score_line(&bare).is_none()
                && !is_heading_line(&bare)
                && !is_being_written(&bare)
        })
        .collect();
    let joined = kept.join("\n");
    crate::invariants::regex(r"\n{3,}")
        .replace_all(&joined, "\n\n")
        .trim()
        .to_owned()
}

pub fn panel_vote(cards: &[Option<&Scorecard>]) -> Option<Scorecard> {
    let read: Vec<&Scorecard> = cards.iter().flatten().copied().collect();
    let first = *read.first()?;
    let same: Vec<&Scorecard> = read
        .into_iter()
        .filter(|c| c.points == first.points)
        .collect();
    let mut verdicts = BTreeMap::new();
    for slot in first.verdicts.keys() {
        let mut row = Vec::with_capacity(first.points.len());
        for index in 0..first.points.len() {
            let cast: Vec<&ScoreVerdict> = same
                .iter()
                .filter_map(|c| c.verdicts.get(slot).and_then(|r| r.get(index)).and_then(|v| v.as_ref()))
                .collect();
            if cast.is_empty() {
                row.push(None);
                continue;
            }
            let met_count = cast.iter().filter(|v| v.met).count();
            let passed = met_count * 2 > cast.len();
            let why = cast
                .iter()
                .find(|v| v.met == passed)
                .map(|v| v.why.clone())
                .unwrap_or_default();
            row.push(Some(ScoreVerdict {
                met: passed,
                why,
                votes: Some((met_count, cast.len())),
            }));
        }
        verdicts.insert(slot.clone(), row);
    }
    Some(Scorecard {
        points: first.points.clone(),
        by_judge: first.by_judge,
        verdicts,
        weights: first.weights.clone(),
    })
}

pub fn panel_pick(picks: &[Option<CompareSlotId>], answered: usize) -> Option<CompareSlotId> {
    let mut counts: BTreeMap<CompareSlotId, usize> = BTreeMap::new();
    for pick in picks.iter().flatten() {
        *counts.entry(*pick).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .find(|(_, count)| count * 2 > answered)
        .map(|(slot, _)| slot)
}

pub fn points_draft_request(ask: &str) -> String {
    format!(
        "Several AI models are about to be compared on this request:\n\n{}\n\nDo not answer it. Write the points a good answer to it must meet, so that a judge can check each one: {DRAFT_MIN_POINTS} to {DRAFT_MAX_POINTS} of them, each specific and checkable, one line each, in exactly this form and with nothing else:\n\nPOINT 1: <a point, in at most ten words>\nPOINT 2: <a point, in at most ten words>",
        clip(ask, 4_000)
    )
}

pub fn drafted_points(text: &str) -> Vec<String> {
    let mut set = Vec::new();
    for line in text.split('\n').map(|l| plain(l.trim_end_matches('\r'))) {
        if let Some((num, body)) = is_point_line(&line)
            && num == set.len() + 1
            && set.len() < DRAFT_MAX_POINTS
        {
            let clipped = clip(&body, MAX_POINT_CHARS);
            if !clipped.is_empty() {
                set.push(clipped);
            }
        }
    }
    set
}

pub fn score_of(row: Option<&[Option<ScoreVerdict>]>, weights: Option<&[usize]>) -> (usize, usize) {
    let Some(cells) = row else {
        return (0, 0);
    };
    let weight_of = |i: usize| weights.and_then(|w| w.get(i).copied()).unwrap_or(1);
    let met = cells
        .iter()
        .enumerate()
        .map(|(i, v)| if v.as_ref().is_some_and(|v| v.met) { weight_of(i) } else { 0 })
        .sum();
    let total = (0..cells.len()).map(weight_of).sum();
    (met, total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_judge_scores_each_answer() {
        assert_eq!(
            criteria_lines(Some("- Exactly four lines\n2. About a swarm at dusk\n\n* Rhymes")),
            vec!["Exactly four lines", "About a swarm at dusk", "Rhymes"]
        );
        let many = (1..=12).map(|i| format!("Point {i}")).collect::<Vec<_>>().join("\n");
        assert_eq!(criteria_lines(Some(&many)).len(), MAX_SCORE_POINTS);
        assert!(criteria_lines(None).is_empty());
        assert!(criteria_lines(Some("  \n ")).is_empty());

        let given = scorecard_request(&["Exactly four lines".into(), "Rhymes".into()], &["A", "B"]);
        assert!(given.contains("1. Exactly four lines\n2. Rhymes"));
        assert!(given.contains("SCORE A 1: met. <why, in at most twelve words>"));
        assert!(given.contains("SCORE B 1: not met. <why, in at most twelve words>"));

        let own = scorecard_request(&[], &["A", "B", "C"]);
        assert!(own.contains(&format!("First set {JUDGE_POINTS} points")));
        assert!(own.contains("POINT 1: <a point, in at most ten words>\nPOINT 2: <a point, in at most ten words>\nPOINT 3: <a point, in at most ten words>"));

        let said = [
            "Answer A keeps to four lines and rhymes throughout. Answer B rhymes only half the time.",
            "I would keep Answer A.",
            "",
            "**Scorecard**",
            "- **SCORE A 1: met.** Four lines exactly.",
            "- SCORE Answer A, point 2 - met - every pair rhymes",
            "SCORE B 1: met. Four lines.",
            "SCORE B 2: not met. Lines 1 and 3 do not rhyme.",
            "SCORE B 2: met. A second verdict on the same cell is not read.",
            "SCORE C 1: met. There is no column C.",
            "SCORE A 3: met. There is no point 3.",
        ]
        .join("\n");
        let card = parse_scorecard(
            &said,
            &["a", "b"],
            &["Exactly four lines".into(), "Each pair rhymes".into()],
        )
        .unwrap();
        assert!(!card.by_judge);
        assert_eq!(card.points, vec!["Exactly four lines", "Each pair rhymes"]);
        assert_eq!(
            card.verdicts["a"],
            vec![
                Some(ScoreVerdict { met: true, why: "Four lines exactly.".into(), votes: None }),
                Some(ScoreVerdict { met: true, why: "every pair rhymes".into(), votes: None }),
            ]
        );
        assert_eq!(
            card.verdicts["b"],
            vec![
                Some(ScoreVerdict { met: true, why: "Four lines.".into(), votes: None }),
                Some(ScoreVerdict { met: false, why: "Lines 1 and 3 do not rhyme.".into(), votes: None }),
            ]
        );
        assert!(!card.verdicts.contains_key("c"));

        let own_text = "POINT 1: Four lines\nPOINT 3: Out of order\nPOINT 2: About a swarm\nSCORE A 1: met. Yes.\nSCORE A 2: not met. A field, not a swarm.";
        let own_card = parse_scorecard(own_text, &["a", "b"], &[]).unwrap();
        assert!(own_card.by_judge);
        assert_eq!(own_card.points, vec!["Four lines", "About a swarm"]);
        assert_eq!(own_card.verdicts["b"], vec![None, None]);

        assert!(parse_scorecard("I would keep Answer A.", &["a", "b"], &["Four lines".into()]).is_none());
        assert!(parse_scorecard("SCORE A 1: met. Yes.", &["a", "b"], &[]).is_none());

        assert_eq!(
            score_of(
                Some(&[
                    Some(ScoreVerdict { met: true, why: "".into(), votes: None }),
                    Some(ScoreVerdict { met: false, why: "".into(), votes: None }),
                    None
                ]),
                None
            ),
            (1, 3)
        );
        assert_eq!(score_of(None, None), (0, 0));

        assert_eq!(
            without_scorecard("Answer A is better.\n\n**Scorecard**\nSCORE A 1: met. Yes.\nPOINT 1: Four lines"),
            "Answer A is better."
        );
        assert_eq!(
            without_scorecard("Answer A is better.\n\nSCORE B 2: no"),
            "Answer A is better."
        );
        assert_eq!(
            without_scorecard("Score aside, both read well."),
            "Score aside, both read well."
        );
    }

    #[test]
    fn weighted_points_and_three_judges_vote() {
        let raw = "Correctness x3\nStyle (×2)\n4x4 grid";
        assert_eq!(criteria_lines(Some(raw)), vec!["Correctness", "Style", "4x4 grid"]);
        assert_eq!(criteria_weights(Some(raw)), vec![3, 2, 1]);

        let make_card = |a: [bool; 2], b: [bool; 2]| Scorecard {
            points: vec!["Names a price".into(), "Says which to buy".into()],
            by_judge: false,
            verdicts: BTreeMap::from([
                ("a".into(), a.iter().map(|&met| Some(ScoreVerdict { met, why: if met { "it does" } else { "it does not" }.into(), votes: None })).collect()),
                ("b".into(), b.iter().map(|&met| Some(ScoreVerdict { met, why: if met { "it does" } else { "it does not" }.into(), votes: None })).collect()),
            ]),
            weights: None,
        };

        let c1 = make_card([true, true], [false, true]);
        let c2 = make_card([true, false], [false, true]);
        let c3 = make_card([false, false], [true, true]);
        let vote = panel_vote(&[Some(&c1), Some(&c2), Some(&c3)]).unwrap();
        assert_eq!(vote.verdicts["a"][0].as_ref().unwrap().met, true);
        assert_eq!(vote.verdicts["a"][0].as_ref().unwrap().votes, Some((2, 3)));
        assert_eq!(vote.verdicts["a"][1].as_ref().unwrap().met, false);
        assert_eq!(vote.verdicts["a"][1].as_ref().unwrap().votes, Some((1, 3)));
        assert_eq!(vote.verdicts["a"][1].as_ref().unwrap().why, "it does not");
        assert_eq!(vote.verdicts["b"][1].as_ref().unwrap().votes, Some((3, 3)));

        assert_eq!(panel_pick(&[Some(CompareSlotId::A), Some(CompareSlotId::A), Some(CompareSlotId::B)], 3), Some(CompareSlotId::A));
        assert_eq!(panel_pick(&[Some(CompareSlotId::A), Some(CompareSlotId::B), None], 3), None);
        assert_eq!(panel_pick(&[Some(CompareSlotId::B), Some(CompareSlotId::B)], 2), Some(CompareSlotId::B));
    }
}
