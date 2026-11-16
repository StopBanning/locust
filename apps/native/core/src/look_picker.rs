//! The Teammate look picker's rules: the Look section of Electron's `NewTeammateDialog.tsx` (lines 416-431 and
//! 509-746), with `shared/avatar.ts`'s `isAvatarSpec` and `cleanAvatar` for what reaches the disk. The window draws it
//! (`src/ui/look_picker.rs`); what is chosen, offered and saved is decided here. See `notes/LOOK-NOTES.md`.

use crate::bot_spec::{
    bot_for, is_pet_id, is_valid_bot_shape, seed_avatar, AvatarSpec, BotFace, BotSpec, PetRef, PetSource,
    ACCESSORY_COUNT, HEADWEAR_COUNT, MOUTH_COUNT,
};
use serde_json::{json, Map, Value};

/// The colour swatches' names, in the dialog's order (`HUES`, NewTeammateDialog.tsx 94-106).
pub const HUE_LABELS: [(&str, &str); 11] = [
    ("lime", "Lime"),
    ("blue", "Blue"),
    ("violet", "Violet"),
    ("clay", "Clay"),
    ("teal", "Teal"),
    ("butter", "Butter"),
    ("rose", "Rose"),
    ("slate", "Slate"),
    ("pearl", "Pearl"),
    ("indigo", "Indigo"),
    ("coral", "Coral"),
];

/// What each shape is called in the Look grid; Locust's own two say so (`SHAPE_NAMES`, 134-158).
pub fn shape_name(shape: &str) -> Option<&'static str> {
    Some(match shape {
        "clover" => "Clover",
        "flower" => "Flower",
        "triangle" => "Triangle",
        "square" => "Square",
        "blob" => "Blob",
        "ghost" => "Ghost",
        "circle" => "Circle",
        "drop" => "Drop",
        "star" => "Star",
        "droid" => "Droid",
        "mech" => "Mech",
        "alien" => "Alien",
        "hexagon" => "Hexagon",
        "cat" => "Cat",
        "cloud" => "Cloud",
        "pill" => "Pill",
        "pebble" => "Pebble",
        "puddle" => "Puddle",
        "hopper" => "Hopper, a Locust",
        "swarm" => "Swarm, a Locust",
        "critter" => "Critter",
        "spark" => "Spark",
        "prompt" => "Prompt",
        _ => return None,
    })
}

/// The title on Screen while it cannot be chosen.
pub const SCREEN_NEEDS_TERMINAL: &str = "Turn Terminal faces on in Settings > Appearance to give a teammate a screen.";
/// The title on Eyes and Mouth for the two mascots.
pub const MASCOT_FACE_IS_SCREEN: &str = "Prompt's face is its screen.";
/// The store's refusal (teammate-store.ts 740, 770).
pub const AVATAR_INVALID: &str = "Teammate avatar is invalid";

/// The Face choice (Eyes, Mouth, Screen), in the dialog's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceChoice {
    Eyes,
    Mouth,
    Screen,
}

impl FaceChoice {
    pub const ALL: [FaceChoice; 3] = [FaceChoice::Eyes, FaceChoice::Mouth, FaceChoice::Screen];

    pub fn label(self) -> &'static str {
        match self {
            Self::Eyes => "Eyes",
            Self::Mouth => "Mouth",
            Self::Screen => "Screen",
        }
    }
}

/// Prompt's and Spark's faces are always their screens.
pub fn always_screen(shape: &str) -> bool {
    matches!(shape, "prompt" | "spark")
}

/// The face the dialog shows as chosen: Screen when the bot wears one, else its face (`faceChoice`, 425-427).
pub fn face_choice(avatar: &AvatarSpec, terminal_faces: bool) -> FaceChoice {
    let look = bot_for(avatar);
    let wears = always_screen(&look.shape)
        || (terminal_faces && look.screen.unwrap_or_else(|| crate::bot_spec::screen_suits(&look.shape)));
    if wears {
        FaceChoice::Screen
    } else if look.face == BotFace::Mouth {
        FaceChoice::Mouth
    } else {
        FaceChoice::Eyes
    }
}

/// Why a Face choice cannot be pressed now, said as its title; None when it can (636-649).
pub fn face_off(avatar: &AvatarSpec, choice: FaceChoice, terminal_faces: bool) -> Option<&'static str> {
    let mascot = always_screen(&bot_for(avatar).shape);
    match choice {
        FaceChoice::Screen => (!terminal_faces && !mascot).then_some(SCREEN_NEEDS_TERMINAL),
        FaceChoice::Eyes | FaceChoice::Mouth => mascot.then_some(MASCOT_FACE_IS_SCREEN),
    }
}

/// The look after a Face choice is pressed (653-659): Screen keeps the face under it; Eyes or Mouth turn the screen off.
pub fn with_face(avatar: &AvatarSpec, choice: FaceChoice) -> AvatarSpec {
    let held = bot_for(avatar);
    let bot = match choice {
        FaceChoice::Screen => BotSpec { shape: held.shape, face: held.face, screen: Some(true) },
        FaceChoice::Eyes => BotSpec { shape: held.shape, face: BotFace::Eyes, screen: Some(false) },
        FaceChoice::Mouth => BotSpec { shape: held.shape, face: BotFace::Mouth, screen: Some(false) },
    };
    AvatarSpec { bot: Some(bot), ..avatar.clone() }
}

/// The look a shape tile shows and gives (`withShape` and `withoutPet`, 212-216 and 429-432): this shape, the face as
/// chosen so far (a screen choice travels with it), and no pet.
pub fn with_shape(avatar: &AvatarSpec, shape: &str) -> AvatarSpec {
    let held = bot_for(avatar);
    AvatarSpec {
        bot: Some(BotSpec { shape: shape.to_owned(), face: held.face, screen: held.screen }),
        pet: None,
        ..avatar.clone()
    }
}

/// A worn pet's "As drawn" (false) or "Screen" (true) (680-696).
pub fn with_pet_screen(avatar: &AvatarSpec, screen: bool) -> AvatarSpec {
    let mut next = avatar.clone();
    if let Some(pet) = next.pet.as_mut() {
        pet.screen = Some(screen);
    }
    next
}

/// Whether a worn pet's "As drawn" (false) or "Screen" (true) is shown as chosen (669-670).
pub fn pet_screen_chosen(pet: &PetRef, screen: bool, terminal_faces: bool) -> bool {
    if screen {
        terminal_faces && pet.screen != Some(false)
    } else {
        !terminal_faces || pet.screen == Some(false)
    }
}

/// The throwaway id a new teammate's first look is seeded from (`draft_${Date.now()}_${Math.random()}`, 311-312), so
/// two dialogs opened in a row start from different faces.
pub fn draft_seed_id(now_ms: u128, random: f64) -> String {
    format!("draft_{now_ms}_{random}")
}

/// A new teammate's first look, seeded from a fresh draft id.
pub fn draft_avatar() -> AvatarSpec {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    // A random number without a dependency: the hasher std seeds afresh for every process and every instance.
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(now);
    let random = (h.finish() >> 11) as f64 / (1u64 << 53) as f64;
    seed_avatar(&draft_seed_id(now, random))
}

/// `isAvatarSpec`: the three parts whole numbers in range, a bot (if any) of a known shape with eyes or a mouth and a
/// screen that is true or false, and a pet (if any) from a known source with a valid id. A key present as `null` is not
/// absent, as in JavaScript. None when it is not one.
pub fn avatar_from_json(value: &Value) -> Option<AvatarSpec> {
    let record = value.as_object()?;
    let part = |name: &str, count: u8| -> Option<u8> {
        let n = record.get(name)?;
        // JSON 1.0 is the number 1 to JavaScript, an integer.
        let v = n.as_u64().or_else(|| n.as_f64().filter(|f| f.fract() == 0.0 && *f >= 0.0).map(|f| f as u64))?;
        (v < u64::from(count)).then_some(v as u8)
    };
    let screen_of = |record: &Map<String, Value>| -> Option<Option<bool>> {
        match record.get("screen") {
            None => Some(None),
            Some(Value::Bool(b)) => Some(Some(*b)),
            Some(_) => None,
        }
    };
    let bot = match record.get("bot") {
        None => None,
        Some(bot) => {
            let bot = bot.as_object()?;
            let shape = bot.get("shape")?.as_str().filter(|s| is_valid_bot_shape(s))?;
            let face = BotFace::from_str(bot.get("face")?.as_str()?)?;
            Some(BotSpec { shape: shape.to_owned(), face, screen: screen_of(bot)? })
        }
    };
    let pet = match record.get("pet") {
        None => None,
        Some(pet) => {
            let pet = pet.as_object()?;
            let source = match pet.get("source")?.as_str()? {
                "gallery" => PetSource::Gallery,
                "codex" => PetSource::Codex,
                "bundled" => PetSource::Bundled,
                _ => return None,
            };
            let id = pet.get("id")?.as_str().filter(|id| is_pet_id(id))?;
            Some(PetRef { source, id: id.to_owned(), screen: screen_of(pet)? })
        }
    };
    Some(AvatarSpec {
        headwear: part("headwear", HEADWEAR_COUNT)?,
        accessory: part("accessory", ACCESSORY_COUNT)?,
        mouth: part("mouth", MOUTH_COUNT)?,
        bot,
        pet,
    })
}

/// `cleanAvatar`, as the record keeps it: only the known fields, a screen only when one was chosen.
pub fn avatar_json(avatar: &AvatarSpec) -> Value {
    let mut out = json!({"headwear": avatar.headwear, "accessory": avatar.accessory, "mouth": avatar.mouth});
    if let Some(bot) = &avatar.bot {
        out["bot"] = json!({"shape": bot.shape, "face": bot.face.as_str()});
        if let Some(screen) = bot.screen {
            out["bot"]["screen"] = json!(screen);
        }
    }
    if let Some(pet) = &avatar.pet {
        let source = match pet.source {
            PetSource::Gallery => "gallery",
            PetSource::Codex => "codex",
            PetSource::Bundled => "bundled",
        };
        out["pet"] = json!({"source": source, "id": pet.id});
        if let Some(screen) = pet.screen {
            out["pet"]["screen"] = json!(screen);
        }
    }
    out
}

#[cfg(test)]
#[path = "look_picker_tests.rs"]
mod tests;
