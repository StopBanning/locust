//! Routes the person actually used, deduplicated by their most recent update.
use crate::ledger::Summary;
use std::collections::HashMap;
pub fn recently_used_routes(missions: &[Summary]) -> Vec<String> {
    let mut positions = HashMap::new();
    let mut routes: Vec<(String, i64)> = Vec::new();
    for m in missions {
        let key = format!("{}:{}", m.runtime, m.model);
        let at = chrono::DateTime::parse_from_rfc3339(&m.updated).ok().map(|t| t.timestamp_millis()).unwrap_or(0);
        if let Some(&index) = positions.get(&key) {
            let held: &mut (String, i64) = &mut routes[index];
            held.1 = held.1.max(at);
        } else {
            positions.insert(key.clone(), routes.len());
            routes.push((key, at));
        }
    }
    routes.sort_by(|left,right| right.1.cmp(&left.1));
    routes.into_iter().map(|(key,_)|key).collect()
}
#[cfg(test)]
#[path="recent_routes_tests.rs"]
mod tests;

/// Rows an agent's group shows before "show more" (`ROUTE_GROUP_LIMIT` in `status.ts`).
pub const ROUTE_GROUP_LIMIT: usize = 6;
/// Recent routes the picker offers as shortcuts (`RECENT_ROUTE_LIMIT` in `status.ts`).
pub const RECENT_ROUTE_LIMIT: usize = 4;

/// A shortcut must shorten the offered list; never resurrect an unavailable route (`recentRouteRows`). Nothing for fewer
/// than two recents, for a list short enough to take in at once (`ROUTE_GROUP_LIMIT + 2` rows or fewer), or for recents
/// that are half the list or more.
pub fn shortcut_keys(offered: &[String], recent: &[String]) -> Vec<String> {
    let picked: Vec<_> = recent.iter().filter(|key| offered.contains(key)).take(RECENT_ROUTE_LIMIT).cloned().collect();
    if picked.len() < 2 || offered.len() <= ROUTE_GROUP_LIMIT + 2 || picked.len() * 2 >= offered.len() { Vec::new() } else { picked }
}
