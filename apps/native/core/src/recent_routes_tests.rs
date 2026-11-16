use crate::{ledger::Summary, recent_routes::recently_used_routes};
fn ran(id:&str,runtime:&str,model:&str,at:&str)->Summary { Summary {mission_id:id.into(),runtime:runtime.into(),model:model.into(),created_at:at.into(),updated:at.into(),..Default::default()} }
#[test]
fn lists_each_route_once_newest_first(){
    let missions=[ran("m1","codex","gpt-5","2026-09-01T00:00:00.000Z"),ran("m2","cursor","composer-2.5","2026-09-02T00:00:00.000Z"),ran("m3","codex","gpt-5","2026-09-03T00:00:00.000Z")];
    assert_eq!(recently_used_routes(&missions),vec!["codex:gpt-5","cursor:composer-2.5"]);
}
#[test]
fn says_nothing_when_nothing_has_been_run(){ assert!(recently_used_routes(&[]).is_empty()); }

#[test]
fn shortcuts_only_shorten_a_long_list_of_currently_offered_routes(){
    use crate::recent_routes::shortcut_keys;
    let offered:Vec<_>=(0..10).map(|i|format!("codex:model-{i}")).collect();
    let recent=vec!["gone:model".into(),offered[3].clone(),offered[1].clone()];
    assert_eq!(shortcut_keys(&offered,&recent),vec![offered[3].clone(),offered[1].clone()]);
    assert!(shortcut_keys(&offered,&recent[1..2]).is_empty());
    assert!(shortcut_keys(&offered[..8],&recent).is_empty());
    assert_eq!(shortcut_keys(&offered,&offered).len(),4);
}

// missionView.ts `recentlyUsedRoutes`: a Map keeps the order routes were first seen in and a stable sort keeps it among
// routes of the same time. (The hand-off feared HashMap order: the HashMap here only finds a route's place.)
#[test]
fn routes_last_used_at_the_same_moment_keep_the_order_they_were_first_seen() {
    let at = "2026-09-01T00:00:00.000Z";
    let (cursor, codex) = ("cursor:composer-2.5", "codex:gpt-5");
    let first = [ran("m1", "cursor", "composer-2.5", at), ran("m2", "codex", "gpt-5", at)];
    assert_eq!(recently_used_routes(&first), vec![cursor, codex]);
    let reversed = [ran("m2", "codex", "gpt-5", at), ran("m1", "cursor", "composer-2.5", at)];
    assert_eq!(recently_used_routes(&reversed), vec![codex, cursor]);
    // A route seen again at the same time keeps its first place.
    let again = [ran("m1", "cursor", "composer-2.5", at), ran("m2", "codex", "gpt-5", at), ran("m3", "cursor", "composer-2.5", at)];
    assert_eq!(recently_used_routes(&again), vec![cursor, codex]);
}

// status.ts `recentRouteRows`: "A list short enough to see at once" is `ROUTE_GROUP_LIMIT + 2` rows: eight shows no shortcuts, nine does.
#[test]
fn a_list_of_eight_is_short_enough_to_take_in_at_once_and_nine_is_not() {
    use crate::recent_routes::{ROUTE_GROUP_LIMIT, RECENT_ROUTE_LIMIT, shortcut_keys};
    assert_eq!((ROUTE_GROUP_LIMIT, RECENT_ROUTE_LIMIT), (6, 4));
    let offered: Vec<String> = (0..9).map(|i| format!("codex:model-{i}")).collect();
    let recent = vec![offered[3].clone(), offered[1].clone()];
    assert!(shortcut_keys(&offered[..8], &recent).is_empty());
    assert_eq!(shortcut_keys(&offered, &recent), recent);
}

