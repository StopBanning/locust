# Chip 1: the composer's chips say what Electron's say (26 Nov 2026)

Driving both apps on the same teammate-less Claude Code conversation (it ran on Opus 5.5, in Auto), the Rust composer's
model chip read `claude-opus-5-5 · Fixed` where Electron's reads `Opus 5.5 · Medium`, and its mode chip read `Ask` where
Electron's reads `Auto`.

## Row 1: the model chip

**Why it read `claude-opus-5-5 · Fixed`.** The conversation is stored on the alias `opus`; its run reported
`claude-opus-5-5` (`model_alias::resolved_model_names`, key `claude:opus`). Two faults:

- The name. The chip printed the reported id as it stood (`else if let Some(resolved) = resolved_id { resolved }`).
  Electron names it with `routeModelName(runtime, model, earned)` (routeName.ts:273, Composer.tsx `namedModel`), which
  for Claude gives the version the run reported: "Opus 5.5".
- The level. A conversation with no teammate is drawn from the door's `list_models` with no teammate
  (door_tools.rs `list_models`, `None` arm), which names each model with no levels at all. The chip took the `opus` row's
  empty list as "this model lists none" and said "Fixed". Electron's chip reads `modelFamily(models, ...)` over the whole
  catalogue (Composer.tsx:1639), where `opus` carries Claude Code's levels, so it says the default: Medium
  (status.ts `defaultEffort` prefers medium).

The brief's other guess, that some list supplies `claude-opus-5-5` with no levels, does not hold: neither the door's
list (it leaves out `older` rows) nor the catalogue (`claude_models_from` skips a version an alias means now) has a row
`claude-opus-5-5`, and with no row the old code could not have said "Fixed". A conversation stored on the full id
`claude-opus-5-5` has no catalogue row in Electron either (`modelFamily` matches ids exactly, model-catalog.ts
`claudeModelsFrom` folds it under `opus`), so Electron's chip reads "Opus 5.5" with no level, and so does this one now.

**What changed** (models.rs, thread_view.rs's chip):

- `models::chip_family` is `modelFamily`; `chip_family_as_read` asks it of the catalogue this process read last
  (`keep_for_the_chip`, written by `models::local` every time it reads; a store apart from `LOCAL_CATALOG`, whose lock a
  sweep holds while it reads, so drawing never waits; per test thread under test).
- `models::chip_levels`: the window's own row speaks only where its list carries levels (a teammate's agent); for the
  conversation-with-no-teammate list (`LEDGER_TARGET`) an empty row is not an answer, the catalogue's family is, and
  with neither nothing is claimed -- not "Fixed" (Composer.tsx `effortIsGenuinelyFixed`: "Only true once the catalog
  has actually answered for THIS model").
- `models::chip_model_name` is `namedModel` then `modelLabel`: a family by the family's name, Claude by
  `routeModelName` with the run's report, never the reported id itself; the level a name ends in is not said twice.
  Before this process has read the catalogue, the window's row name stands in for the catalogue's (the old chip's
  `model_chip_name`), unless it is only the id.
- Codex (`gpt-6.2-sol`) reads "GPT-6.2 Sol · Medium", not its run's dated id; an OpenCode free route reads
  "Nemotron 3 Ultra · Medium" with "Free" as its own tag (`modelDisplayName`, a-model-has-one-name.test.ts).

## Row 2: the mode chip

**Why it read `Ask`.** The chip showed `composer_mode` else the teammate's `runs_in`. A conversation with no teammate
has `runs_in` empty, and `mode_facts::fact("")` falls back to the first fact, Ask. Electron's App.tsx `followRouteOf`
(App.tsx:7421-7422) sets the window's mode to the mode the conversation's newest turn recorded
(`run.restoredMission?.mode`) when its agent runs it: Auto.

**What changed.** `composer_mode::mode_shown` (new, core/src/composer_mode.rs) is App.tsx's mode state for one
conversation: a mode picked on the box, else the open conversation's newest turn's recorded mode (where the agent runs
it), else the teammate's own (`selectTeammate`, `setMode(own.mode)`), else App.tsx's first mode, Accept edits
(`useState<MissionMode>('accept-edits')`); then Composer.tsx `effectiveMode`: never a mode the route cannot run.
thread_view.rs's mode chip reads it, with the recorded mode from `snapshot.turn_metas.last()`.

**Left for the lane check, outside this lane's files (important).** The send does not read the same rule yet:
main.rs:1146 sends `s.composer_mode` only (None here), so native.rs (`requested_mode(Some(&job.teammate.runs_in))`)
runs a conversation with no teammate in Ask, the old chip's word. With row 2 alone the chip says Auto and the turn runs
in Ask. What makes them agree, in main.rs `send` (not this lane's to touch): compute the mode as the chip does, with the
same three inputs thread_view.rs passes, and send it:

    let recorded = s.ledger.as_ref().filter(|p| p.header.id == teammate_id)
        .and_then(|p| p.snapshot.turn_metas.last()).and_then(|t| t.mode.as_deref());
    let shown = locust_core::composer_mode::mode_shown(s.composer_mode.as_deref(), recorded, &header.runs_in, runtime);
    let mode = locust_core::approval::Mode::parse(&shown);

(Sending `Some` always also changes a teammate's turn from "the engine takes the teammate's own" to the same mode said
explicitly, which is what Electron does: its send carries the window's mode.) Also main.rs `open_recent` (1667) keeps a
mode picked in the previous conversation (`composer_mode` is not cleared there, as `select` clears it at main.rs:549),
where Electron's `followRouteOf` replaces it with the opened conversation's recorded mode.

## Row 3: the rest of the composer foot, listed (none fixed here)

| Rust | Electron | Electron's source |
| --- | --- | --- |
| The comparison chip says "Direct" (compare_view.rs `composer_mode_chip`, `CompareComposerMode::label`). | Direct shows only its icon; the word is said for Compare, Blind, Cloud. | Composer.tsx:2509 `{chatMode !== 'direct' && <span>{...name}</span>}` |
| The folder chip names `teammate.folder`, else the folder Locust has open (`folders::folder_name`, as stored). | The window's folder, `basename(workspacePath)`; the case seen differed between the two, so the two apps hold the path in different spellings (which one is stored is the folder lane's to find; the chip prints what it is given). | main/index.ts:9438, Composer.tsx:2718 |
| The mode chip's spoken label is "Mode: Auto". | "Permission mode", the mode said by the chip's own word. | Composer.tsx:2618 |
| The mode chip shows Auto while Settings' Auto switch is off; the send then refuses. | Turning Auto off moves the window off it, to Accept edits, once the setting is known. | App.tsx:2775-2777 |
| `mode_shown` asks `runs_on` with no platform. | `modeRunsOn(mode, runtime, platform)` with the build's platform (Cursor's read-only modes are barred on Windows). | Composer.tsx:863 |
