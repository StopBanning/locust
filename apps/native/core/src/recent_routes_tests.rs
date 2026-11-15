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
