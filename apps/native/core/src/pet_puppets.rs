//! The pets Colin kept, as puppets on a bot's rig, with a bot's terminal eyes (`petPuppets.ts`, and the pure half of
//! `components/PetPuppet.tsx`; 2026-10-05).
//!
//! Eight pets (Cabin, Astro Bot, Meowbot, Macintosh, Bitty, TmuxAI, Rainbow Terminal Cat, Nori) are drawn from ONE
//! drawing, their resting pose, moved by a bot's own rig (`bot_rig::BotAvatarSim`): it floats, leans, looks about,
//! hops and lands, dozes. Over its face goes a screen with a bot's eyes. A head on a body that walks (Astro Bot,
//! Meowbot) or on a stand (TmuxAI) is cut straight across its neck; nothing of the head is cut. The rest are kept
//! whole. Codex Buddy is not a puppet: he keeps his drawings and has his own rig (`buddy_rig`, `buddy_moves`).
//!
//! Locust ships none of the drawings; it keeps only these numbers. A sheet is checked against the one measured
//! (`print_fits`) before it becomes a puppet; one that is not is drawn as its maker drew it.
//!
//! Units: a drawing's own pixels (192 x 208 a cell); the bot's body box is a hundred units across, -50 to 50, as
//! `bot_rig`'s bodies are, in a canvas `BOT_AVATAR_OVERSCAN` times the face's size.

use crate::bot_rig::{BotAvatarPose, BOT_AVATAR_OVERSCAN, BOT_AVATAR_RISE};
use crate::bot_spec::{BodyBox, PetRef};
use crate::buddy_moves::{painted_box, ScreenRectF, EDGE_SLACK};

/// One kept pet as a puppet (`PetPuppet extends SheetPrint`).
#[derive(Debug, PartialEq)]
pub struct PetPuppet {
    pub id: &'static str,
    /// One drawing's size, in the sheet's pixels.
    pub frame_width: u32,
    pub frame_height: u32,
    /// How many drawings each of the nine rows has.
    pub counts: &'static [usize],
    /// The drawing it is drawn from: its resting pose (row, column).
    pub rest: (usize, usize),
    /// The row its head ends at: what is below it (a body that walks, a stand) is cut away. None: kept whole.
    pub neck: Option<u32>,
    /// What is kept's painted bounds: left, top, right and the row after its bottom. Its ring and dot sit on it.
    pub paint: [u32; 4],
    /// The square of the drawing a bot's box shows: its left, its top and its side.
    pub window: [f64; 3],
    /// Its screen: x, y, w, h.
    pub glass: [u32; 4],
    /// The glass's hue: its own screen's, darkened as a bot's visor is.
    pub tint: &'static str,
    /// How dark the glass is beside a bot's visor (1, as dark).
    pub dark: Option<f64>,
    /// Round the glass, where the pet has no bezel of its own.
    pub ink: Option<&'static str>,
    /// The glass's corners, a fraction of its shorter side.
    pub corner: f64,
    /// The eyes' size beside a bot screen's (1, the same).
    pub eye_scale: Option<f64>,
    /// The eyes' height on the glass: -1 its top, 0 its middle, 1 its bottom.
    pub eye_y: Option<f64>,
    /// Each drawing's painted left, top, right and the row after its bottom (alpha over half): what a sheet is checked by.
    pub edges: &'static [&'static [[u32; 4]]],
}

const E0_0: [[u32; 4]; 6] = [[34, 5, 158, 203], [34, 5, 158, 203], [35, 5, 157, 203], [34, 5, 157, 203], [35, 5, 156, 203], [35, 5, 157, 203]];
const E0_1: [[u32; 4]; 8] = [[23, 5, 168, 203], [23, 5, 168, 203], [31, 5, 161, 203], [34, 5, 157, 203], [24, 5, 167, 203], [26, 5, 165, 203], [28, 5, 164, 203], [34, 5, 158, 203]];
const E0_2: [[u32; 4]; 8] = [[34, 5, 158, 203], [28, 5, 164, 203], [26, 5, 165, 203], [24, 5, 167, 203], [34, 5, 157, 203], [31, 5, 161, 203], [23, 5, 168, 203], [23, 5, 168, 203]];
const E0_3: [[u32; 4]; 4] = [[28, 5, 163, 203], [23, 5, 168, 203], [22, 5, 169, 203], [29, 5, 162, 203]];
const E0_4: [[u32; 4]; 5] = [[5, 31, 187, 177], [20, 5, 172, 203], [24, 5, 168, 203], [19, 5, 172, 203], [5, 30, 187, 177]];
const E0_5: [[u32; 4]; 8] = [[18, 5, 173, 203], [21, 5, 170, 203], [21, 5, 171, 203], [19, 5, 173, 203], [20, 5, 172, 203], [19, 5, 173, 203], [17, 5, 175, 203], [17, 5, 174, 203]];
const E0_6: [[u32; 4]; 6] = [[30, 5, 162, 203], [30, 5, 162, 203], [28, 5, 164, 203], [29, 5, 162, 203], [27, 5, 164, 203], [30, 5, 162, 203]];
const E0_7: [[u32; 4]; 6] = [[30, 5, 161, 203], [31, 5, 161, 203], [31, 5, 161, 203], [31, 5, 161, 203], [31, 5, 161, 203], [31, 5, 161, 203]];
const E0_8: [[u32; 4]; 6] = [[28, 5, 163, 203], [26, 5, 166, 203], [29, 5, 162, 203], [24, 5, 167, 203], [29, 5, 162, 203], [29, 5, 163, 203]];
const E0: [&[[u32; 4]]; 9] = [&E0_0, &E0_1, &E0_2, &E0_3, &E0_4, &E0_5, &E0_6, &E0_7, &E0_8];
const E1_0: [[u32; 4]; 6] = [[18, 5, 174, 203], [20, 5, 172, 203], [22, 5, 170, 203], [22, 5, 170, 203], [22, 5, 170, 203], [22, 5, 170, 203]];
const E1_1: [[u32; 4]; 8] = [[28, 5, 163, 203], [27, 5, 164, 203], [31, 5, 161, 203], [23, 5, 169, 203], [29, 5, 163, 203], [26, 5, 165, 203], [29, 5, 162, 203], [28, 5, 163, 203]];
const E1_2: [[u32; 4]; 8] = [[28, 5, 163, 203], [29, 5, 162, 203], [26, 5, 165, 203], [29, 5, 163, 203], [23, 5, 169, 203], [31, 5, 161, 203], [27, 5, 164, 203], [28, 5, 163, 203]];
const E1_3: [[u32; 4]; 4] = [[17, 5, 174, 203], [12, 5, 180, 203], [9, 5, 182, 203], [18, 5, 173, 203]];
const E1_4: [[u32; 4]; 5] = [[18, 5, 174, 203], [23, 5, 168, 203], [8, 5, 183, 203], [14, 5, 178, 203], [18, 5, 174, 203]];
const E1_5: [[u32; 4]; 8] = [[23, 5, 169, 203], [21, 5, 170, 203], [9, 5, 182, 203], [5, 36, 187, 171], [7, 5, 184, 203], [7, 5, 185, 203], [8, 11, 184, 197], [5, 6, 187, 201]];
const E1_6: [[u32; 4]; 6] = [[22, 5, 170, 203], [25, 5, 166, 203], [22, 5, 169, 203], [27, 5, 164, 203], [21, 5, 170, 203], [27, 5, 165, 203]];
const E1_7: [[u32; 4]; 6] = [[26, 5, 165, 203], [18, 5, 173, 203], [23, 5, 168, 203], [24, 5, 167, 203], [22, 5, 170, 203], [25, 5, 166, 203]];
const E1_8: [[u32; 4]; 6] = [[23, 5, 169, 203], [26, 5, 165, 203], [28, 5, 164, 203], [24, 5, 168, 203], [29, 5, 163, 203], [26, 5, 166, 203]];
const E1: [&[[u32; 4]]; 9] = [&E1_0, &E1_1, &E1_2, &E1_3, &E1_4, &E1_5, &E1_6, &E1_7, &E1_8];
const E2_0: [[u32; 4]; 6] = [[14, 5, 178, 203], [12, 5, 179, 203], [18, 5, 174, 203], [14, 5, 178, 203], [15, 5, 176, 203], [10, 5, 182, 203]];
const E2_1: [[u32; 4]; 8] = [[5, 5, 187, 203], [5, 5, 186, 203], [8, 5, 183, 203], [15, 5, 176, 203], [11, 5, 180, 203], [16, 5, 175, 203], [17, 5, 174, 203], [13, 5, 178, 203]];
const E2_2: [[u32; 4]; 8] = [[13, 5, 178, 203], [17, 5, 174, 203], [16, 5, 175, 203], [11, 5, 180, 203], [15, 5, 176, 203], [8, 5, 183, 203], [5, 5, 186, 203], [5, 5, 187, 203]];
const E2_3: [[u32; 4]; 4] = [[18, 5, 173, 203], [20, 5, 172, 203], [15, 5, 177, 203], [19, 5, 172, 203]];
const E2_4: [[u32; 4]; 5] = [[11, 5, 181, 203], [24, 5, 168, 203], [25, 5, 167, 203], [24, 5, 168, 203], [19, 5, 173, 203]];
const E2_5: [[u32; 4]; 8] = [[22, 5, 170, 203], [14, 5, 177, 203], [9, 5, 183, 203], [10, 5, 182, 203], [9, 5, 183, 203], [5, 5, 187, 203], [6, 5, 186, 203], [14, 5, 177, 203]];
const E2_6: [[u32; 4]; 6] = [[13, 5, 178, 203], [16, 5, 176, 203], [18, 5, 174, 203], [16, 5, 175, 203], [12, 5, 179, 203], [18, 5, 174, 203]];
const E2_7: [[u32; 4]; 6] = [[22, 5, 169, 203], [18, 5, 173, 203], [16, 5, 175, 203], [20, 5, 172, 203], [19, 5, 173, 203], [21, 5, 171, 203]];
const E2_8: [[u32; 4]; 6] = [[17, 5, 174, 203], [21, 5, 170, 203], [21, 5, 171, 203], [17, 5, 174, 203], [19, 5, 172, 203], [17, 5, 175, 203]];
const E2: [&[[u32; 4]]; 9] = [&E2_0, &E2_1, &E2_2, &E2_3, &E2_4, &E2_5, &E2_6, &E2_7, &E2_8];
const E3_0: [[u32; 4]; 6] = [[12, 5, 179, 203], [12, 5, 179, 203], [12, 5, 179, 203], [13, 5, 179, 203], [13, 5, 179, 203], [12, 5, 179, 203]];
const E3_1: [[u32; 4]; 8] = [[9, 5, 183, 203], [10, 5, 182, 203], [9, 5, 182, 203], [11, 5, 180, 203], [12, 5, 179, 203], [10, 5, 181, 203], [11, 5, 181, 203], [11, 5, 180, 203]];
const E3_2: [[u32; 4]; 8] = [[17, 5, 175, 203], [10, 5, 182, 203], [10, 5, 182, 203], [13, 5, 178, 203], [9, 5, 183, 203], [11, 5, 180, 203], [14, 5, 177, 203], [16, 5, 176, 203]];
const E3_3: [[u32; 4]; 4] = [[9, 5, 183, 203], [5, 8, 187, 200], [5, 11, 187, 196], [9, 5, 182, 203]];
const E3_4: [[u32; 4]; 5] = [[9, 5, 183, 203], [9, 5, 182, 203], [5, 8, 187, 200], [13, 5, 178, 203], [8, 5, 183, 203]];
const E3_5: [[u32; 4]; 8] = [[23, 5, 169, 203], [14, 5, 177, 203], [21, 5, 171, 203], [28, 5, 163, 203], [22, 5, 169, 203], [5, 9, 187, 199], [15, 5, 177, 203], [16, 5, 176, 203]];
const E3_6: [[u32; 4]; 6] = [[8, 5, 183, 203], [13, 5, 179, 203], [9, 5, 182, 203], [16, 5, 175, 203], [12, 5, 179, 203], [14, 5, 177, 203]];
const E3_7: [[u32; 4]; 6] = [[12, 5, 180, 203], [10, 5, 181, 203], [11, 5, 180, 203], [6, 5, 186, 203], [10, 5, 182, 203], [11, 5, 180, 203]];
const E3_8: [[u32; 4]; 6] = [[22, 5, 170, 203], [14, 5, 177, 203], [27, 5, 164, 203], [20, 5, 172, 203], [27, 5, 165, 203], [22, 5, 169, 203]];
const E3: [&[[u32; 4]]; 9] = [&E3_0, &E3_1, &E3_2, &E3_3, &E3_4, &E3_5, &E3_6, &E3_7, &E3_8];
const E4_0: [[u32; 4]; 6] = [[5, 8, 187, 199], [5, 8, 187, 199], [5, 7, 187, 200], [5, 7, 187, 200], [5, 7, 187, 200], [5, 8, 187, 199]];
const E4_1: [[u32; 4]; 8] = [[5, 5, 187, 203], [5, 6, 187, 201], [5, 5, 187, 202], [5, 6, 187, 202], [5, 6, 187, 202], [6, 5, 186, 203], [9, 5, 183, 203], [5, 5, 186, 203]];
const E4_2: [[u32; 4]; 8] = [[5, 5, 186, 203], [9, 5, 183, 203], [6, 5, 186, 203], [5, 6, 187, 202], [5, 6, 187, 202], [5, 5, 187, 202], [5, 6, 187, 201], [5, 5, 187, 203]];
const E4_3: [[u32; 4]; 4] = [[8, 5, 184, 203], [8, 5, 184, 203], [5, 8, 187, 199], [14, 5, 177, 203]];
const E4_4: [[u32; 4]; 5] = [[5, 16, 187, 192], [5, 12, 187, 195], [5, 16, 187, 191], [5, 20, 187, 187], [5, 9, 187, 199]];
const E4_5: [[u32; 4]; 8] = [[7, 5, 184, 203], [7, 5, 184, 203], [5, 11, 187, 196], [5, 16, 187, 191], [5, 39, 187, 168], [5, 23, 187, 185], [5, 6, 187, 202], [7, 5, 185, 203]];
const E4_6: [[u32; 4]; 6] = [[7, 5, 185, 203], [5, 7, 187, 201], [5, 7, 187, 201], [5, 58, 187, 150], [5, 43, 187, 164], [5, 58, 187, 150]];
const E4_7: [[u32; 4]; 6] = [[5, 6, 187, 202], [5, 6, 187, 202], [5, 5, 187, 202], [5, 6, 187, 201], [5, 7, 187, 201], [6, 5, 186, 203]];
const E4_8: [[u32; 4]; 6] = [[5, 5, 186, 203], [5, 5, 187, 202], [9, 5, 183, 203], [5, 5, 187, 202], [5, 5, 187, 203], [5, 7, 187, 201]];
const E4: [&[[u32; 4]]; 9] = [&E4_0, &E4_1, &E4_2, &E4_3, &E4_4, &E4_5, &E4_6, &E4_7, &E4_8];
const E5_0: [[u32; 4]; 6] = [[30, 5, 161, 203], [35, 5, 156, 203], [25, 5, 166, 203], [34, 5, 158, 203], [32, 5, 159, 203], [31, 5, 161, 203]];
const E5_1: [[u32; 4]; 8] = [[22, 5, 169, 203], [22, 5, 169, 203], [24, 5, 167, 203], [18, 5, 173, 203], [7, 5, 184, 203], [20, 5, 171, 203], [24, 5, 167, 203], [24, 5, 168, 203]];
const E5_2: [[u32; 4]; 8] = [[24, 5, 168, 203], [24, 5, 167, 203], [20, 5, 171, 203], [7, 5, 184, 203], [18, 5, 173, 203], [24, 5, 167, 203], [22, 5, 169, 203], [22, 5, 169, 203]];
const E5_3: [[u32; 4]; 4] = [[32, 5, 159, 203], [25, 5, 167, 203], [22, 5, 170, 203], [31, 5, 160, 203]];
const E5_4: [[u32; 4]; 5] = [[5, 17, 187, 191], [30, 5, 162, 203], [47, 5, 145, 203], [32, 5, 160, 203], [5, 30, 187, 178]];
const E5_5: [[u32; 4]; 8] = [[27, 5, 165, 203], [27, 5, 164, 203], [27, 5, 165, 203], [22, 5, 170, 203], [7, 5, 185, 203], [5, 11, 187, 197], [5, 24, 187, 183], [5, 56, 187, 152]];
const E5_6: [[u32; 4]; 6] = [[26, 5, 165, 203], [28, 5, 164, 203], [24, 5, 167, 203], [26, 5, 166, 203], [26, 5, 166, 203], [26, 5, 166, 203]];
const E5_7: [[u32; 4]; 6] = [[32, 5, 159, 203], [20, 5, 172, 203], [19, 5, 172, 203], [31, 5, 160, 203], [17, 5, 174, 203], [31, 5, 161, 203]];
const E5_8: [[u32; 4]; 6] = [[32, 5, 160, 203], [26, 5, 165, 203], [17, 5, 175, 203], [20, 5, 171, 203], [29, 5, 163, 203], [32, 5, 160, 203]];
const E5: [&[[u32; 4]]; 9] = [&E5_0, &E5_1, &E5_2, &E5_3, &E5_4, &E5_5, &E5_6, &E5_7, &E5_8];
const E6_0: [[u32; 4]; 6] = [[5, 43, 187, 165], [5, 42, 187, 165], [5, 42, 187, 166], [5, 42, 187, 165], [5, 42, 187, 165], [5, 43, 187, 164]];
const E6_1: [[u32; 4]; 8] = [[5, 39, 187, 168], [5, 30, 187, 177], [5, 23, 187, 185], [5, 30, 187, 178], [5, 29, 187, 178], [5, 39, 187, 168], [5, 36, 187, 172], [5, 36, 187, 171]];
const E6_2: [[u32; 4]; 8] = [[5, 44, 187, 164], [5, 33, 187, 174], [5, 35, 187, 173], [5, 29, 187, 178], [5, 31, 187, 177], [5, 32, 187, 175], [5, 35, 187, 173], [5, 44, 187, 164]];
const E6_3: [[u32; 4]; 4] = [[5, 39, 187, 169], [5, 40, 187, 167], [5, 39, 187, 168], [5, 40, 187, 167]];
const E6_4: [[u32; 4]; 5] = [[5, 45, 187, 162], [5, 42, 187, 165], [5, 40, 187, 167], [5, 41, 187, 166], [5, 46, 187, 162]];
const E6_5: [[u32; 4]; 8] = [[5, 40, 187, 168], [5, 39, 187, 169], [5, 47, 187, 161], [5, 63, 187, 145], [5, 47, 187, 160], [5, 48, 187, 159], [5, 47, 187, 161], [5, 40, 187, 167]];
const E6_6: [[u32; 4]; 6] = [[5, 39, 187, 169], [5, 39, 187, 168], [5, 39, 187, 169], [5, 39, 187, 168], [5, 39, 187, 168], [5, 39, 187, 168]];
const E6_7: [[u32; 4]; 6] = [[5, 39, 187, 169], [5, 42, 187, 166], [5, 39, 187, 168], [5, 38, 187, 169], [5, 39, 187, 169], [5, 42, 187, 166]];
const E6_8: [[u32; 4]; 6] = [[5, 35, 187, 172], [5, 36, 187, 171], [5, 39, 187, 168], [5, 35, 187, 173], [5, 39, 187, 169], [5, 39, 187, 168]];
const E6: [&[[u32; 4]]; 9] = [&E6_0, &E6_1, &E6_2, &E6_3, &E6_4, &E6_5, &E6_6, &E6_7, &E6_8];
const E7_0: [[u32; 4]; 6] = [[5, 41, 187, 166], [5, 37, 187, 170], [5, 40, 187, 168], [5, 39, 187, 169], [5, 40, 187, 168], [5, 38, 187, 170]];
const E7_1: [[u32; 4]; 8] = [[5, 30, 187, 177], [5, 32, 187, 176], [5, 34, 187, 174], [5, 30, 187, 178], [5, 31, 187, 177], [5, 29, 187, 179], [5, 30, 187, 178], [5, 30, 187, 177]];
const E7_2: [[u32; 4]; 8] = [[5, 30, 187, 177], [5, 30, 187, 178], [5, 29, 187, 179], [5, 31, 187, 177], [5, 30, 187, 178], [5, 34, 187, 174], [5, 32, 187, 176], [5, 30, 187, 177]];
const E7_3: [[u32; 4]; 4] = [[5, 40, 187, 168], [5, 44, 187, 163], [5, 44, 187, 164], [5, 38, 187, 169]];
const E7_4: [[u32; 4]; 5] = [[5, 48, 187, 159], [5, 35, 187, 173], [5, 37, 187, 170], [5, 33, 187, 174], [5, 47, 187, 160]];
const E7_5: [[u32; 4]; 8] = [[5, 38, 187, 170], [5, 39, 187, 169], [5, 38, 187, 169], [5, 47, 187, 160], [5, 37, 187, 170], [5, 37, 187, 170], [5, 38, 187, 169], [5, 38, 187, 170]];
const E7_6: [[u32; 4]; 6] = [[5, 37, 187, 170], [5, 35, 187, 172], [5, 26, 187, 181], [5, 35, 187, 172], [5, 33, 187, 174], [5, 35, 187, 173]];
const E7_7: [[u32; 4]; 6] = [[5, 34, 187, 173], [5, 34, 187, 173], [5, 33, 187, 174], [5, 33, 187, 174], [5, 35, 187, 173], [5, 33, 187, 175]];
const E7_8: [[u32; 4]; 6] = [[5, 36, 187, 171], [5, 35, 187, 172], [5, 33, 187, 175], [5, 33, 187, 174], [5, 34, 187, 173], [5, 33, 187, 174]];
const E7: [&[[u32; 4]]; 9] = [&E7_0, &E7_1, &E7_2, &E7_3, &E7_4, &E7_5, &E7_6, &E7_7, &E7_8];

/// The eight pets Colin kept, as `petPuppets.ts` measured them (`_tools/measure-pet-puppets.py`).
/// A `static`, so `&PET_PUPPETS[i]` has one address (BUILD-SPEED-NOTES: a const is a copy at each use).
pub static PET_PUPPETS: [PetPuppet; 8] = [
    // Cabin: a phone that is all screen: kept whole, its glass near black as its own, its eyes as large as its old face.
    PetPuppet {
        id: "cabin-face",
        frame_width: 192,
        frame_height: 208,
        counts: &[6, 8, 8, 4, 5, 8, 6, 6, 6],
        rest: (0, 0),
        neck: None,
        paint: [34, 5, 158, 203],
        window: [-8.9, -0.9, 209.9],
        glass: [44, 22, 104, 160],
        tint: "#121e22",
        dark: Some(0.45),
        ink: None,
        corner: 0.22,
        eye_scale: Some(1.7),
        eye_y: Some(-0.18),
        edges: &E0,
    },
    // Astro Bot: its helmet, antennae and headphones, cut from its body at its neck, its eyes on its own black screen.
    PetPuppet {
        id: "astro-bot",
        frame_width: 192,
        frame_height: 208,
        counts: &[6, 8, 8, 4, 5, 8, 6, 6, 6],
        rest: (0, 0),
        neck: Some(136),
        paint: [18, 5, 174, 136],
        window: [13.3, -12.2, 165.4],
        glass: [55, 55, 85, 66],
        tint: "#141c28",
        dark: None,
        ink: None,
        corner: 0.38,
        eye_scale: None,
        eye_y: None,
        edges: &E1,
    },
    // Meowbot: its cat-eared helmet, antenna and headphones, cut from its body and tail at its collar, its eyes on its own dark visor.
    PetPuppet {
        id: "meowbot",
        frame_width: 192,
        frame_height: 208,
        counts: &[6, 8, 8, 4, 5, 8, 6, 6, 6],
        rest: (0, 0),
        neck: Some(143),
        paint: [24, 5, 178, 143],
        window: [18.3, -8.2, 164.3],
        glass: [65, 63, 89, 67],
        tint: "#141a1e",
        dark: None,
        ink: None,
        corner: 0.42,
        eye_scale: None,
        eye_y: None,
        edges: &E2,
    },
    // Macintosh: kept whole, arms and feet, its screen a terminal now: dark, with a hint of its lavender.
    PetPuppet {
        id: "macintosh",
        frame_width: 192,
        frame_height: 208,
        counts: &[6, 8, 8, 4, 5, 8, 6, 6, 6],
        rest: (0, 0),
        neck: None,
        paint: [12, 5, 179, 203],
        window: [-9.4, -0.9, 209.9],
        glass: [56, 30, 78, 72],
        tint: "#c7bcf4",
        dark: None,
        ink: None,
        corner: 0.14,
        eye_scale: None,
        eye_y: None,
        edges: &E3,
    },
    // Bitty: kept whole, hands, cable and feet, its screen a terminal now: dark, with a hint of its green.
    PetPuppet {
        id: "bitty",
        frame_width: 192,
        frame_height: 208,
        counts: &[6, 8, 8, 4, 5, 8, 6, 6, 6],
        rest: (0, 0),
        neck: None,
        paint: [5, 8, 187, 199],
        window: [-5.2, 2.3, 202.5],
        glass: [41, 38, 66, 57],
        tint: "#9acb6a",
        dark: None,
        ink: None,
        corner: 0.12,
        eye_scale: None,
        eye_y: None,
        edges: &E4,
    },
    // TmuxAI: the cube and its antenna, cut from its stand, a glass on its face where its painted eyes were.
    PetPuppet {
        id: "tmuxai",
        frame_width: 192,
        frame_height: 208,
        counts: &[6, 8, 8, 4, 5, 8, 6, 6, 6],
        rest: (0, 0),
        neck: Some(162),
        paint: [32, 5, 161, 161],
        window: [13.3, 0.3, 166.4],
        glass: [45, 63, 90, 76],
        tint: "#283c46",
        dark: None,
        ink: Some("#24262c"),
        corner: 0.3,
        eye_scale: Some(1.1),
        eye_y: None,
        edges: &E5,
    },
    // Rainbow Terminal Cat: kept whole, curled by its laptop, a visor of glass over its eyes.
    PetPuppet {
        id: "rainbow-terminal-cat",
        frame_width: 192,
        frame_height: 208,
        counts: &[6, 8, 8, 4, 5, 8, 6, 6, 6],
        rest: (0, 0),
        neck: None,
        paint: [5, 43, 187, 165],
        window: [-0.5, 7.5, 192.9],
        glass: [45, 94, 58, 30],
        tint: "#3c325a",
        dark: None,
        ink: Some("#1e1a28"),
        corner: 0.45,
        eye_scale: None,
        eye_y: None,
        edges: &E6,
    },
    // Nori: kept whole, a visor of glass over its face.
    PetPuppet {
        id: "nori",
        frame_width: 192,
        frame_height: 208,
        counts: &[6, 8, 8, 4, 5, 8, 6, 6, 6],
        rest: (0, 0),
        neck: None,
        paint: [5, 41, 187, 166],
        window: [-0.5, 7.0, 192.9],
        glass: [29, 108, 68, 30],
        tint: "#462828",
        dark: None,
        ink: Some("#1e1616"),
        corner: 0.45,
        eye_scale: None,
        eye_y: None,
        edges: &E7,
    },
];


/// The puppet of a pet of this id, before its sheet is checked (`print_fits`); None for any other pet.
pub fn puppet_for(id: &str) -> Option<&'static PetPuppet> {
    PET_PUPPETS.iter().find(|p| p.id == id)
}

// ---------------------------------------------------------------------------
// Its sheet is checked (`petScreens.ts` screenFits, the four-edge print)
// ---------------------------------------------------------------------------

/// Whether a sheet is the one this puppet was measured on: its size (eight across, nine down), and every drawing's
/// painted box within `EDGE_SLACK` of the table's. A drawing where the table has none, or none where it has one, is
/// not the sheet measured. `rgba` is the whole sheet, `width` across.
pub fn print_fits(puppet: &PetPuppet, rgba: &[u8], width: usize, height: usize) -> bool {
    let (fw, fh) = (puppet.frame_width as usize, puppet.frame_height as usize);
    if width != fw * crate::pets::PET_COLUMNS as usize || height != fh * 9 || rgba.len() < width * height * 4 {
        return false;
    }
    for (row, &count) in puppet.counts.iter().enumerate() {
        for column in 0..crate::pets::PET_COLUMNS as usize {
            let want = if column < count { puppet.edges.get(row).and_then(|r| r.get(column)).copied() } else { None };
            let found = painted_box(rgba, width, column * fw, row * fh, fw, fh);
            match (want, found) {
                (None, None) => {}
                (Some(w), Some(f)) => {
                    if w.iter().zip(f.iter()).any(|(w, f)| w.abs_diff(*f) > EDGE_SLACK) {
                        return false;
                    }
                }
                _ => return false,
            }
        }
    }
    true
}

/// The puppet a worn pet becomes on this sheet (`pets.ts` measuredOf): its id's puppet, when the sheet is the one
/// measured; None for any other pet, and for a sheet that is not.
pub fn measured_puppet(pet_id: &str, rgba: &[u8], width: usize, height: usize) -> Option<&'static PetPuppet> {
    puppet_for(pet_id).filter(|p| print_fits(p, rgba, width, height))
}

/// Whether a teammate wearing `pet` shows it as a puppet (`TeammateBot.tsx` 413-415): its sheet measured, Terminal
/// faces on, and its teammate's look not "as drawn" (`screen: false`). Anything else is drawn as its maker drew it.
pub fn wears_puppet(pet: &PetRef, measured: bool, terminal_faces: bool) -> bool {
    measured && terminal_faces && pet.screen != Some(false)
}

// ---------------------------------------------------------------------------
// Each pet moves as what it is (PetPuppet.tsx PUPPET_MOTION)
// ---------------------------------------------------------------------------

/// How much of a screen's squash and stretch a puppet keeps: half (a drawing squashed further is a stretched picture).
pub const PUPPET_SQUASH: f64 = 0.5;
/// How far a full turn moves the drawing, in body units: a hint of the way it looks, not a step.
pub const TURN_SHIFT: f64 = 3.0;
/// How far a full nod moves it.
pub const NOD_SHIFT: f64 = 2.0;

/// A puppet's style: how much of the rig's pose it takes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PuppetMotion {
    /// Of the rig's rise: its float, and its hops.
    pub lift: f64,
    /// Of the rig's squash and stretch.
    pub squash: f64,
    /// Of its lean, and of its little move toward a turn.
    pub lean: f64,
    /// It stands on the ground: never below it, and it leans and squashes about its feet.
    pub grounded: bool,
    /// A soft body's wobble with each breath, a share of its width (0, none).
    pub jiggle: f64,
    /// A lying body's breath: its back rises, a share of its height (0, none).
    pub breathe: f64,
}

/// A head, as a bot is: the rig's float, hops and lean, half a screen's squash.
pub const HEAD_MOTION: PuppetMotion =
    PuppetMotion { lift: 1.0, squash: PUPPET_SQUASH, lean: 1.0, grounded: false, jiggle: 0.0, breathe: 0.0 };

/// Each kept pet's style (measured 2026-10-05 against bot-avatars 0.2.2: the rig rises 2 units at rest and a quarter
/// of the box in a hop, and squashes to three quarters of its height on landing).
pub const PUPPET_MOTION: [(&str, PuppetMotion); 8] = [
    // Heads: they float and hop as bots do.
    ("astro-bot", HEAD_MOTION),
    ("meowbot", HEAD_MOTION),
    ("tmuxai", PuppetMotion { squash: 0.4, ..HEAD_MOTION }),
    // A phone: a hard thing, it tilts and lifts and never squashes.
    ("cabin-face", PuppetMotion { lift: 0.7, squash: 0.0, lean: 1.0, grounded: false, jiggle: 0.0, breathe: 0.0 }),
    // Little machines on their feet: they rock on them and hop small, hardly giving.
    ("macintosh", PuppetMotion { lift: 0.45, squash: 0.1, lean: 0.7, grounded: true, jiggle: 0.0, breathe: 0.0 }),
    ("bitty", PuppetMotion { lift: 0.5, squash: 0.1, lean: 0.9, grounded: true, jiggle: 0.0, breathe: 0.0 }),
    // A cat lying on its laptop: it never hops; it breathes, and sways a little.
    ("rainbow-terminal-cat", PuppetMotion { lift: 0.0, squash: 0.15, lean: 0.35, grounded: true, jiggle: 0.0, breathe: 0.025 }),
    // A sushi: soft, low to the ground, a jelly's squash and wobble, a small boing.
    ("nori", PuppetMotion { lift: 0.25, squash: 1.0, lean: 0.5, grounded: true, jiggle: 0.03, breathe: 0.0 }),
];

/// How a puppet moves: its own style, or a head's.
pub fn motion_of(puppet: &PetPuppet) -> PuppetMotion {
    PUPPET_MOTION.iter().find(|(id, _)| *id == puppet.id).map_or(HEAD_MOTION, |(_, m)| *m)
}

/// The rig's pose as this puppet wears it: its rise, squash and lean by its style, and its own breath.
pub fn puppet_pose(pose: BotAvatarPose, motion: PuppetMotion) -> BotAvatarPose {
    let breath = pose.breath.clamp(-1.0, 1.0);
    let jiggle = motion.jiggle * breath;
    let rise = motion.breathe * (breath + 1.0) * 0.5;
    BotAvatarPose {
        y: (if motion.grounded { pose.y.min(0.0) } else { pose.y }) * motion.lift,
        sx: 1.0 + (pose.sx - 1.0) * motion.squash + jiggle,
        sy: 1.0 + (pose.sy - 1.0) * motion.squash - jiggle + rise,
        roll: pose.roll * motion.lean,
        yaw: pose.yaw * motion.lean,
        ..pose
    }
}

// ---------------------------------------------------------------------------
// Drawn on a bot's rig (PetPuppet.tsx puppetTransform, puppetRect, puppetDrawing, puppetBody)
// ---------------------------------------------------------------------------

/// Where a puppet's drawing goes for a pose it wears (`puppet_pose`): the canvas transform `[a, b, c, d, e, f]` (a
/// point (x, y) of the box lands at (a x + c y + e, b x + d y + f)) in a canvas `size * BOT_AVATAR_OVERSCAN * dpr`
/// device pixels across, with the hint of a turn and a nod. A head squashes about the box's foot and leans about its
/// middle; one that stands on the ground (`feet`, the row its paint ends at in the box's units) does both about its
/// feet: it rocks on them, and never lifts off them to lean.
pub fn puppet_transform(pose: BotAvatarPose, size: f64, dpr: f64, feet: Option<f64>) -> [f64; 6] {
    let side = size * BOT_AVATAR_OVERSCAN * dpr;
    let e = (size * dpr) / 100.0;
    let (sin, cos) = pose.roll.sin_cos();
    let squash_at = feet.unwrap_or(50.0);
    let lean_at = feet.unwrap_or(0.0);
    let foot = squash_at * (1.0 - pose.sy);
    let shift_x = -sin * foot + sin * lean_at;
    let shift_y = cos * foot + lean_at - cos * lean_at;
    let x = side / 2.0 + (pose.x + pose.yaw.sin() * TURN_SHIFT + shift_x) * e;
    let y = side / 2.0 + BOT_AVATAR_RISE * size * dpr + (pose.y - pose.pitch.sin() * NOD_SHIFT + shift_y) * e;
    [cos * pose.sx * e, sin * pose.sx * e, -sin * pose.sy * e, cos * pose.sy * e, x, y]
}

/// A rectangle of the drawing (its pixels), in the box's units under `puppet_transform`.
pub fn puppet_rect(puppet: &PetPuppet, x: f64, y: f64, w: f64, h: f64) -> ScreenRectF {
    let [left, top, side] = puppet.window;
    let k = 100.0 / side;
    ScreenRectF { x: (x - left) * k - 50.0, y: (y - top) * k - 50.0, w: w * k, h: h * k }
}

/// Its screen, in the box's units.
pub fn puppet_glass(puppet: &PetPuppet) -> ScreenRectF {
    let [x, y, w, h] = puppet.glass.map(f64::from);
    puppet_rect(puppet, x, y, w, h)
}

/// Where its feet are, in the box's units, for one that stands on the ground; None for one that floats.
pub fn puppet_feet(puppet: &PetPuppet) -> Option<f64> {
    motion_of(puppet).grounded.then(|| puppet_rect(puppet, 0.0, f64::from(puppet.paint[3]), 0.0, 0.0).y)
}

/// What of its sheet a puppet draws (`from`, the sheet's pixels) and where (`to`, the box's units): its resting
/// drawing, as much of it as its window shows (a window can reach past a drawing's edges), nothing below its neck.
pub fn puppet_drawing(puppet: &PetPuppet, frame_width: f64, frame_height: f64) -> (ScreenRectF, ScreenRectF) {
    let [left, top, side] = puppet.window;
    let (row, column) = puppet.rest;
    let from_x = left.max(0.0);
    let to_x = frame_width.min(left + side);
    let from_y = top.max(0.0);
    let to_y = frame_height.min(top + side).min(puppet.neck.map_or(frame_height, f64::from));
    (
        ScreenRectF {
            x: column as f64 * frame_width + from_x,
            y: row as f64 * frame_height + from_y,
            w: to_x - from_x,
            h: to_y - from_y,
        },
        puppet_rect(puppet, from_x, from_y, to_x - from_x, to_y - from_y),
    )
}

/// Where its ring and dot sit (`botAnchors.ts`): on what is drawn of it, its paint, as fractions of its box.
pub fn puppet_body(puppet: &PetPuppet) -> BodyBox {
    let [left, top, side] = puppet.window;
    let [pl, pt, pr, pb] = puppet.paint.map(f64::from);
    let within = |v: f64| v.clamp(0.0, 1.0);
    BodyBox {
        left: within((pl - left) / side),
        top: within((pt - top) / side),
        right: within((pr - left) / side),
        bottom: within((pb - top) / side),
    }
}

#[cfg(test)]
#[path = "pet_puppets_tests.rs"]
mod tests;
