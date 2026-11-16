//! The look picker, drawn in the real teammate form and read through its accessibility tree, as a person (or a screen
//! reader) meets it. Electron's two dialog cases of `every-teammate-is-a-bot.test.tsx` (the shapes and the face, and
//! Shuffle) and its colours case are named after them; the rest are one behaviour each of `NewTeammateDialog.tsx`.

use super::*;
use crate::look::bot_stage::PresenceDot;
use serde_json::{json, Value};

fn wren(avatar: Value) -> Value {
    json!({"teammateId":"tm_wren", "name":"Wren", "hue":"teal", "role":"Custom", "createdAt":"2026-09-23T05:00:00.000Z", "avatar": avatar})
}

fn form(team: Value, initial: Option<&str>, terminal: bool) -> State {
    let mut s = State::default();
    s.settings_panel.settings.terminal_faces = terminal;
    s.people_panel.team = json!({"teammates": team});
    crate::people_panel::edit(&mut s, initial);
    s
}

fn render(s: State) -> rune::Runtime<State> {
    let mut rt = rune::Runtime::new(s, |s, _| crate::people_panel::edit_form(s, &Palette::new(true)), (1100.0, 2400.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn node<'a>(tree: &'a Value, key: &str) -> Option<&'a Value> {
    if tree["key"] == key {
        return Some(tree);
    }
    tree["children"].as_array()?.iter().find_map(|child| node(child, key))
}

fn has(state: &Value, which: &str) -> bool {
    state["states"].as_array().is_some_and(|all| all.iter().any(|s| s == which))
}

fn radios(node: &Value, out: &mut Vec<Value>) {
    if node["role"] == "radio" || node["role"] == "radiobutton" || node["role"] == "radio-button" {
        out.push(node.clone());
        return;
    }
    for child in node["children"].as_array().into_iter().flatten() {
        radios(child, out);
    }
}

/// The radios in the group with this key, in order.
pub(crate) fn radios_in(rt: &rune::Runtime<State>, group: &str) -> Vec<Value> {
    let tree = rt.accessibility_json();
    let group = node(&tree, group).unwrap_or_else(|| panic!("no {group} in the accessibility tree"));
    let mut out = Vec::new();
    radios(group, &mut out);
    out
}

/// The names of the checked radios in the group with this key.
pub(crate) fn checked_in(rt: &rune::Runtime<State>, group: &str) -> Vec<String> {
    radios_in(rt, group).iter().filter(|r| has(r, "checked")).filter_map(|r| r["name"].as_str().map(str::to_owned)).collect()
}

fn names(radios: &[Value]) -> Vec<String> {
    radios.iter().map(|r| r["name"].as_str().unwrap_or("").to_owned()).collect()
}

fn click(rt: &mut rune::Runtime<State>, key: &str) {
    rt.frame(0.016);
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
}

fn avatar(rt: &rune::Runtime<State>) -> Value {
    rt.state.people_panel.entry["avatar"].clone()
}

// every-teammate-is-a-bot.test.tsx, "the new teammate dialog".

#[test]
fn offers_every_shape_to_choose_from_and_the_face() {
    let rt = render(form(json!([]), None, true));
    let tree = rt.accessibility_json();
    let grid = node(&tree, "team-look-grid").expect("the Look grid");
    assert_eq!(grid["name"], "Look");
    let shapes = radios_in(&rt, "team-look-grid");
    assert_eq!(shapes.len(), 23);
    let said = names(&shapes);
    for name in ["Critter", "Prompt", "Hopper, a Locust", "Swarm, a Locust"] {
        assert!(said.iter().any(|n| n == name), "{name} in {said:?}");
    }
    assert_eq!(node(&tree, "team-look-face").expect("the Face choice")["name"], "Face");
}

#[test]
fn keeps_shuffle_once_with_the_look_it_shuffles() {
    let rt = render(form(json!([]), None, true));
    let tree = rt.accessibility_json();
    fn count(node: &Value, name: &str) -> usize {
        usize::from(node["name"] == name && node["role"] == "button") + node["children"].as_array().into_iter().flatten().map(|c| count(c, name)).sum::<usize>()
    }
    assert_eq!(count(&tree, "Shuffle look"), 1);
    assert_eq!(count(node(&tree, "team-look-head").expect("the Look head"), "Shuffle look"), 1, "in the Look head");
}

#[test]
fn has_eleven_colours_indigo_and_coral_drawn_as_the_teammates_own_bot() {
    let rt = render(form(json!([]), None, true));
    let tree = rt.accessibility_json();
    assert_eq!(node(&tree, "team-hues").expect("the colours")["name"], "Avatar colour");
    assert_eq!(
        names(&radios_in(&rt, "team-hues")),
        ["Lime", "Blue", "Violet", "Clay", "Teal", "Butter", "Rose", "Slate", "Pearl", "Indigo", "Coral"]
    );
    assert!(rt.find("bot-static-look-hue-coral-24").is_some(), "each swatch is the bot at 24");
}

// NewTeammateDialog.tsx, one behaviour each.

#[test]
fn the_preview_is_the_teammates_own_bot_at_work_with_its_working_dot() {
    let rt = render(form(json!([wren(json!({"headwear":0,"accessory":0,"mouth":0,"bot":{"shape":"cat","face":"eyes"}}))]), Some("tm_wren"), true));
    assert_eq!(rt.state.bot_stage.desired_activity("team-look-preview"), Some(locust_core::bot_rig::FaceActivity::Working));
    assert_eq!(crate::look::faces::drawn_presence("team-look-preview"), Some(Some(PresenceDot::Working)));
    let preview = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("bot-team-look-preview")).expect("the preview");
    assert_eq!((preview.rect[2], preview.rect[3]), (64.0, 64.0));
}

#[test]
fn a_new_teammate_starts_on_a_fresh_drafted_look_and_an_edit_on_its_own() {
    let fresh = form(json!([]), None, true);
    assert!(locust_core::look_picker::avatar_from_json(&fresh.people_panel.entry["avatar"]).is_some(), "a look from the start");
    let own = json!({"headwear":2,"accessory":1,"mouth":3,"bot":{"shape":"spark","face":"mouth"}});
    let edit = form(json!([wren(own.clone())]), Some("tm_wren"), true);
    assert_eq!(edit.people_panel.entry["avatar"], own);
    // A record without a look opens on the one the store shows for it, seedAvatar(teammateId).
    let bare = form(json!([{"teammateId":"tm_wren", "name":"Wren", "hue":"teal", "role":"Custom"}]), Some("tm_wren"), true);
    let seeded = locust_core::bot_spec::seed_avatar("tm_wren");
    assert_eq!(bare.people_panel.entry["avatar"], json!({"headwear": seeded.headwear, "accessory": seeded.accessory, "mouth": seeded.mouth}));
}

#[test]
fn picking_a_shape_keeps_the_face_chosen_and_create_saves_what_the_preview_shows() {
    let (_dir, store) = crate::org::tests::temp_store();
    let mut s = form(json!([]), None, false);
    s.data = Some(store.clone());
    s.people_panel.fields[0].replace_text("Iris");
    let mut rt = render(s);
    click(&mut rt, "team-look-face-mouth");
    click(&mut rt, "team-look-shape-ghost");
    assert_eq!(checked_in(&rt, "team-look-grid"), ["Ghost"]);
    assert_eq!(checked_in(&rt, "team-look-face"), ["Mouth"]);
    click(&mut rt, "team-hue-rose");
    assert_eq!(checked_in(&rt, "team-hues"), ["Rose"]);
    let shown = avatar(&rt);
    click(&mut rt, "team-save");
    let saved = locust_core::people::team(&store).unwrap()["teammates"][0].clone();
    assert_eq!(saved["name"], "Iris");
    assert_eq!(saved["hue"], "rose");
    assert_eq!(saved["avatar"], shown);
    assert_eq!(saved["avatar"]["bot"], json!({"shape":"ghost","face":"mouth","screen":false}));
}

#[test]
fn screen_waits_for_terminal_faces_and_says_where_to_turn_it_on() {
    let rt = render(form(json!([wren(json!({"headwear":0,"accessory":0,"mouth":0,"bot":{"shape":"clover","face":"eyes"}}))]), Some("tm_wren"), false));
    let face = radios_in(&rt, "team-look-face");
    assert_eq!(names(&face), ["Eyes", "Mouth", "Screen"]);
    assert!(has(&face[2], "disabled"));
    assert_eq!(face[2]["description"], "Turn Terminal faces on in Settings > Appearance to give a teammate a screen.");
    assert!(!has(&face[0], "disabled") && !has(&face[1], "disabled"));
    let on = render(form(json!([wren(json!({"headwear":0,"accessory":0,"mouth":0,"bot":{"shape":"clover","face":"eyes"}}))]), Some("tm_wren"), true));
    assert!(!has(&radios_in(&on, "team-look-face")[2], "disabled"));
}

#[test]
fn prompts_face_is_its_screen_so_eyes_and_mouth_are_off() {
    let rt = render(form(json!([wren(json!({"headwear":0,"accessory":0,"mouth":0,"bot":{"shape":"prompt","face":"eyes"}}))]), Some("tm_wren"), false));
    let face = radios_in(&rt, "team-look-face");
    assert_eq!(checked_in(&rt, "team-look-face"), ["Screen"]);
    for off in &face[..2] {
        assert!(has(off, "disabled"));
        assert_eq!(off["description"], "Prompt's face is its screen.");
    }
    assert!(!has(&face[2], "disabled"));
}

#[test]
fn pressing_screen_keeps_the_face_under_it_and_eyes_turn_it_off() {
    let mut rt = render(form(json!([wren(json!({"headwear":0,"accessory":0,"mouth":0,"bot":{"shape":"clover","face":"mouth"}}))]), Some("tm_wren"), true));
    click(&mut rt, "team-look-face-screen");
    assert_eq!(avatar(&rt)["bot"], json!({"shape":"clover","face":"mouth","screen":true}));
    assert_eq!(checked_in(&rt, "team-look-face"), ["Screen"]);
    click(&mut rt, "team-look-face-eyes");
    assert_eq!(avatar(&rt)["bot"], json!({"shape":"clover","face":"eyes","screen":false}));
}

#[test]
fn shuffle_rolls_a_derived_look_and_lets_go_of_the_picked_bot() {
    let mut rt = render(form(json!([wren(json!({"headwear":1,"accessory":2,"mouth":3,"bot":{"shape":"hopper","face":"mouth"}}))]), Some("tm_wren"), true));
    click(&mut rt, "team-look-shuffle");
    assert_eq!(avatar(&rt), json!({"headwear":2,"accessory":0,"mouth":0}));
    let shown = locust_core::bot_spec::bot_for(&locust_core::look_picker::avatar_from_json(&avatar(&rt)).unwrap()).shape;
    assert_eq!(checked_in(&rt, "team-look-grid"), [locust_core::look_picker::shape_name(&shown).unwrap()]);
}

#[test]
fn a_worn_pet_keeps_its_own_face_the_colours_become_chips_and_a_shape_takes_it_off() {
    let buddy = json!({"headwear":0,"accessory":0,"mouth":0,"pet":{"source":"codex","id":"codex-buddy"}});
    let mut rt = render(form(json!([wren(buddy)]), Some("tm_wren"), true));
    assert!(rt.find("bot-static-look-hue-coral-24").is_none(), "plain colours, not the pet tinted");
    assert_eq!(radios_in(&rt, "team-hues").len(), 11);
    assert_eq!(names(&radios_in(&rt, "team-look-face")), ["As drawn", "Screen"]);
    assert_eq!(checked_in(&rt, "team-look-face"), ["Screen"], "Codex Buddy wears its screen while Terminal faces is on");
    assert!(checked_in(&rt, "team-look-grid").is_empty(), "no shape is chosen while a pet is worn");
    click(&mut rt, "team-look-pet-drawn");
    assert_eq!(avatar(&rt)["pet"]["screen"], false);
    click(&mut rt, "team-look-shape-star");
    assert_eq!(avatar(&rt).get("pet"), None);
    assert_eq!(checked_in(&rt, "team-look-grid"), ["Star"]);
    assert_eq!(names(&radios_in(&rt, "team-look-face")), ["Eyes", "Mouth", "Screen"]);
}

// NewTeammateDialog.tsx 511-559 (`.lc-dialog__identity`): the preview on the left; beside it, one column of Name, its box
// and the colour swatches under them. The swatches were on a row of their own above Name.
#[test]
fn puts_the_swatches_beside_the_preview_under_the_name_box() {
    let rt = render(form(json!([wren(json!(null))]), Some("tm_wren"), false));
    let rect = |key: &str| rt.presented(key).unwrap_or_else(|| panic!("{key} is not drawn")).0;
    let (preview, name, hues) = (rect("bot-team-look-preview"), rect("person-field-0"), rect("team-hues"));
    assert!(hues[0] >= preview[0] + preview[2] && name[0] >= preview[0] + preview[2], "beside the preview: {preview:?} {name:?} {hues:?}");
    assert_eq!(hues[0], name[0], "one column");
    assert!(hues[1] >= name[1] + name[3], "under the name box: {name:?} {hues:?}");
    let identity = rect("team-identity");
    assert!(preview[1] > identity[1] && preview[1] + preview[3] < identity[1] + identity[3], "the preview is centred on the column: {identity:?} {preview:?}");
}
