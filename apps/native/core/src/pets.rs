//! The pet sheet format (Electron `shared/pets.ts` and `main/webp-size.ts`, 0.563): a pet is `pet.json` beside
//! `spritesheet.webp`, an atlas of 8 columns of 192 x 208 frames, 9 rows or (version 2) 11. The WebP header is read
//! by hand, as Electron does: no image library, only the few fields a sheet is checked by before it is kept.

pub const PET_COLUMNS: u32 = 8;
pub const PET_FRAME_WIDTH: u32 = 192;
pub const PET_FRAME_HEIGHT: u32 = 208;
/// A version 2 sheet is exactly this size, and transparent.
pub const PET_V2_WIDTH: u32 = PET_COLUMNS * PET_FRAME_WIDTH;
pub const PET_V2_HEIGHT: u32 = 11 * PET_FRAME_HEIGHT;
/// How many bytes `webp_size` needs to read.
pub const WEBP_HEADER_BYTES: usize = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebpSize {
    pub width: u32,
    pub height: u32,
    pub alpha: bool,
    /// More than one frame: a pet sheet must be one still image.
    pub animated: bool,
}

fn little(bytes: &[u8], at: usize, length: usize) -> u64 {
    let mut value = 0u64;
    for index in (0..length).rev() {
        value = value * 256 + u64::from(bytes.get(at + index).copied().unwrap_or(0));
    }
    value
}

/// The size of the WebP these bytes begin, or None when they are not one this reads. `file_length`, when known, must
/// agree with the RIFF length (or be one more, the padding byte): a sheet cut short in a download fails here.
pub fn webp_size(bytes: &[u8], file_length: Option<u64>) -> Option<WebpSize> {
    if bytes.len() < WEBP_HEADER_BYTES || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return None;
    }
    let riff = little(bytes, 4, 4);
    if riff < 12 {
        return None;
    }
    if let Some(length) = file_length
        && riff + 8 != length
        && riff + 8 + 1 != length
    {
        return None;
    }
    match &bytes[12..16] {
        b"VP8 " => {
            if bytes[23] != 0x9d || bytes[24] != 0x01 || bytes[25] != 0x2a {
                return None;
            }
            let width = (little(bytes, 26, 2) & 0x3fff) as u32;
            let height = (little(bytes, 28, 2) & 0x3fff) as u32;
            (width != 0 && height != 0).then_some(WebpSize { width, height, alpha: false, animated: false })
        }
        b"VP8L" => {
            if bytes[20] != 0x2f {
                return None;
            }
            let bits = little(bytes, 21, 4);
            // The version is the top three bits and is always 0.
            if bits >> 29 != 0 {
                return None;
            }
            Some(WebpSize {
                width: ((bits & 0x3fff) + 1) as u32,
                height: (((bits >> 14) & 0x3fff) + 1) as u32,
                alpha: (bits >> 28) & 1 == 1,
                animated: false,
            })
        }
        b"VP8X" => {
            let flags = bytes[20];
            Some(WebpSize {
                width: (little(bytes, 24, 3) + 1) as u32,
                height: (little(bytes, 27, 3) + 1) as u32,
                alpha: flags & 0x10 != 0,
                animated: flags & 0x02 != 0,
            })
        }
        _ => None,
    }
}

/// Whether a sheet of this size can be a pet of this version: its rows (9 or 11), or in words a person can read why
/// not (Electron `petRowsFor`). Version 2 is exact; version 1 reads at its own scale.
pub fn pet_rows_for(size: WebpSize, version: u8) -> Result<u8, String> {
    if size.animated {
        return Err("Its sheet is an animation, not one still image.".into());
    }
    if !size.alpha {
        return Err("Its sheet has no transparency, so it would be drawn on a box.".into());
    }
    if version == 2 {
        return if size.width == PET_V2_WIDTH && size.height == PET_V2_HEIGHT {
            Ok(11)
        } else {
            Err(format!(
                "Its sheet is {} x {}; a version 2 pet's is {} x {}.",
                size.width, size.height, PET_V2_WIDTH, PET_V2_HEIGHT
            ))
        };
    }
    if size.width % PET_COLUMNS != 0 || size.height % 9 != 0 {
        return Err("Its sheet is not eight frames across and nine down.".into());
    }
    let frame_width = f64::from(size.width) / f64::from(PET_COLUMNS);
    let frame_height = f64::from(size.height) / 9.0;
    let aspect = frame_width / frame_height / (f64::from(PET_FRAME_WIDTH) / f64::from(PET_FRAME_HEIGHT));
    if !(0.98..=1.02).contains(&aspect) {
        return Err("Its frames are not the shape a pet is drawn in.".into());
    }
    if frame_height < f64::from(PET_FRAME_HEIGHT) / 2.0 || frame_height > f64::from(PET_FRAME_HEIGHT) * 2.0 {
        return Err("Its frames are too small or too large to be a pet.".into());
    }
    Ok(9)
}

/// What a pet can be doing, by the name of its sheet's row (OpenPets' own names).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetState {
    Idle,
    Review,
    Running,
    Waving,
    Waiting,
    Jumping,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PetRow {
    pub row: u32,
    pub frames: u32,
    /// One pass through the row, milliseconds.
    pub ms: f64,
    /// Played this many times, then it settles; None: it loops.
    pub plays: Option<u32>,
}

/// OpenPets' `defaultPetSprite.states`, the rows a teammate uses (`PET_ROWS`).
pub fn pet_row(state: PetState) -> PetRow {
    let (row, frames, ms, plays) = match state {
        PetState::Idle => (0, 6, 5500.0, None),
        PetState::Waving => (3, 4, 700.0, Some(2)),
        PetState::Jumping => (4, 5, 840.0, Some(2)),
        PetState::Failed => (5, 8, 1220.0, Some(2)),
        PetState::Waiting => (6, 6, 1010.0, None),
        PetState::Running => (7, 6, 820.0, None),
        PetState::Review => (8, 6, 1030.0, None),
    };
    PetRow { row, frames, ms, plays }
}

/// A version 2 sheet's resting pose: row 0, column 6.
pub const PET_NEUTRAL: (u32, u32) = (0, 6);

/// A version 2 head turned toward one of 16 directions, 0 straight up, clockwise in 22.5-degree steps: 0-7 on row 9,
/// 8-15 on row 10. Returns (row, column).
pub fn pet_gaze_cell(index: i64) -> Option<(u32, u32)> {
    match index {
        0..=7 => Some((9, index as u32)),
        8..=15 => Some((10, (index - 8) as u32)),
        _ => None,
    }
}
