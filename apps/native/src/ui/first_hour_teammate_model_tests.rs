use super::*;
use rune::{Env, Runtime};

// case: a new teammate's model is chosen right under its name, before how it looks
#[test]
fn a_new_teammate_puts_the_model_between_the_name_and_look() {
    let mut s = State::default();
    open_new(&mut s);
    let mut rt = Runtime::new(
        s,
        |s, _| edit_form(s, &Palette::new(true)),
        (1200.0, 1600.0),
        Env::new(true, 1.0),
    );
    rt.frame(0.0);
    let all = rt.elements();
    let index = |key| {
        all.iter()
            .position(|el| el.key.as_deref() == Some(key))
            .unwrap_or_else(|| panic!("missing {key}"))
    };
    let name = index("person-field-0");
    let model = index("team-model-name");
    let look = index("team-look");
    assert!(
        name < model && model < look,
        "name {name}, model {model}, look {look}"
    );
}
