//! The window side of dictation and Talk. Audio and transcription are supplied by the platform.

use super::*;
use locust_core::{
    runtime::codex_voice::{CallEvent, CallOptions, CallWindow, NativeHost, VoiceCalls},
    voice_audio::{self, Audio, AudioPeer, Recording},
    voice_call_rules::{Target, Teammate as VoiceTeammate},
    voice_typing::{self, Mode, TranscriptionTransport, VoiceSettings},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, TryRecvError},
};

struct Capture {
    cancel: Arc<AtomicBool>,
    target: Option<String>,
    state: CaptureState,
}

enum CaptureState {
    Opening(Receiver<Result<(Mode, Box<dyn Recording>), String>>),
    Recording(Mode, Box<dyn Recording>),
    Transcribing(Receiver<Result<String, String>>),
}

struct Talk {
    cancel: Arc<AtomicBool>,
    target: Option<String>,
    conversation: Option<String>,
    state: TalkState,
}

enum TalkState {
    Opening(Receiver<Result<Box<dyn AudioPeer>, String>>, CallOptions),
    Connected(Box<dyn AudioPeer>, CallWindow, bool),
    Closing(CallWindow),
}

#[derive(Default)]
pub(super) struct Panel {
    platform: Option<&'static str>,
    pub(super) audio: Option<Arc<dyn Audio>>,
    pub(super) transcriber: Option<Arc<dyn TranscriptionTransport>>,
    secrets: Option<Arc<dyn locust_core::own_models::SecretBox>>,
    service: Option<Arc<VoiceCalls>>,
    capture: Option<Capture>,
    call: Option<Talk>,
    note: Option<String>,
    questions: Vec<(String, locust_core::runtime::codex_voice::ConnectorQuestion)>,
}

impl Drop for Panel {
    fn drop(&mut self) {
        if let Some(capture) = &self.capture {
            capture.cancel.store(true, Ordering::Release);
        }
        if let Some(call) = &self.call {
            call.cancel.store(true, Ordering::Release);
        }
        if let Some(service) = &self.service {
            service.dispose();
        }
    }
}

fn platform(s: &State) -> &'static str {
    s.voice.platform.unwrap_or(if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(windows) {
        "win32"
    } else {
        "linux"
    })
}

fn mic(s: &mut State, tx: &mut Tx) {
    let Some(store) = s.data.clone() else { return };
    if s.voice.call.is_some() {
        return;
    }
    s.voice.note = None;
    match s.voice.capture.take() {
        Some(capture) => match capture.state {
            CaptureState::Recording(mode, recording) => {
                let Some(http) = s.voice.transcriber.clone() else {
                    return;
                };
                let platform = platform(s);
                let cancel = capture.cancel.clone();
                let (send, receive) = mpsc::channel();
                let secrets = s
                    .voice
                    .secrets
                    .clone()
                    .unwrap_or_else(locust_core::own_models::secrets);
                std::thread::spawn(move || {
                    let outcome = recording.finish().and_then(|samples| {
                        let wav = voice_typing::wav(&samples).ok_or("No speech was recorded.")?;
                        VoiceSettings::new(&store, secrets.as_ref(), platform).transcribe(
                            &wav,
                            mode,
                            &cancel,
                            http.as_ref(),
                        )
                    });
                    let _ = send.send(outcome);
                });
                s.voice.capture = Some(Capture {
                    state: CaptureState::Transcribing(receive),
                    ..capture
                });
            }
            _ => s.voice.capture = Some(capture),
        },
        None => {
            let Some(audio) = s.voice.audio.clone() else {
                s.voice.note = Some("Voice typing needs native microphone support.".into());
                return;
            };
            if s.voice.transcriber.is_none() {
                s.voice.note = Some("Voice typing needs a transcription service in Locust.".into());
                return;
            }
            let cancel = Arc::new(AtomicBool::new(false));
            let cancelled = cancel.clone();
            let platform = platform(s);
            let (send, receive) = mpsc::channel();
            let secrets = s
                .voice
                .secrets
                .clone()
                .unwrap_or_else(locust_core::own_models::secrets);
            std::thread::spawn(move || {
                let settings = VoiceSettings::new(&store, secrets.as_ref(), platform).settings();
                let result = if settings.mode != Mode::OpenAi {
                    Err(voice_typing::local_only(platform))
                } else if !settings.has_key {
                    Err("Add your OpenAI API key under Voice typing in Settings.".into())
                } else if !settings.openai_consent {
                    Err("Allow sending audio to OpenAI before starting voice typing.".into())
                } else {
                    voice_audio::open_recording(audio.as_ref(), &cancelled)
                        .map(|recording| (settings.mode, recording))
                };
                let _ = send.send(result);
            });
            s.voice.capture = Some(Capture {
                cancel,
                target: conversation_draft_key(s),
                state: CaptureState::Opening(receive),
            });
        }
    }
    tx.wake_after(Duration::from_millis(25));
}

pub(super) fn cancel(s: &mut State) -> bool {
    let Some(capture) = s.voice.capture.take() else {
        return false;
    };
    capture.cancel.store(true, Ordering::Release);
    s.voice.note = None;
    true
}

fn poll_capture(s: &mut State) -> bool {
    let Some(mut capture) = s.voice.capture.take() else {
        return false;
    };
    if capture.target != conversation_draft_key(s) {
        capture.cancel.store(true, Ordering::Release);
        return true;
    }
    let note = match &capture.state {
        CaptureState::Opening(receive) => match receive.try_recv() {
            Ok(Ok((mode, recording))) => {
                capture.state = CaptureState::Recording(mode, recording);
                s.voice.capture = Some(capture);
                return true;
            }
            Ok(Err(why)) => Some(why),
            Err(TryRecvError::Disconnected) => {
                Some("Voice typing could not open the microphone.".into())
            }
            Err(TryRecvError::Empty) => {
                s.voice.capture = Some(capture);
                return false;
            }
        },
        CaptureState::Transcribing(receive) => match receive.try_recv() {
            Ok(Ok(words)) => {
                let kept =
                    s.draft.text.encode_utf16().count() - s.draft.selected().encode_utf16().count();
                let limit = native::MAX_PROMPT_LENGTH.saturating_sub(kept);
                let words = locust_core::people::bounded(&words, limit);
                if !words.is_empty() {
                    s.draft.insert(&words);
                }
                None
            }
            Ok(Err(why)) => Some(why),
            Err(TryRecvError::Disconnected) => {
                Some("Voice typing could not finish. Your message is unchanged.".into())
            }
            Err(TryRecvError::Empty) => {
                s.voice.capture = Some(capture);
                return false;
            }
        },
        CaptureState::Recording(..) => {
            s.voice.capture = Some(capture);
            return false;
        }
    };
    s.voice.note = note;
    true
}

fn call_options(s: &State, store: &store::Store) -> Option<CallOptions> {
    let (teammate, conversation) = selected_conversation(s)?;
    let folder = teammate
        .folder
        .as_deref()
        .or(s.home.folder_path.as_deref())?;
    let voice_teammate = (!native::is_nobody(teammate)).then(|| {
        let voice = locust_core::people::team(store).ok().and_then(|team| {
            team["teammates"]
                .as_array()?
                .iter()
                .find(|entry| entry["teammateId"] == teammate.id)?["voice"]
                .as_str()
                .filter(|voice| locust_core::voice_call_rules::is_voice(voice))
                .map(str::to_owned)
        });
        VoiceTeammate {
            target: Target {
                teammate_id: teammate.id.clone(),
                conversation_id: conversation.conversation_id.clone(),
            },
            name: teammate.name.clone(),
            role: Some(teammate.role.clone()),
            runtime: teammate
                .runtime
                .as_deref()
                .map(engine::runtime_name)
                .unwrap_or("its own AI agent")
                .into(),
            voice,
        }
    });
    Some(CallOptions {
        folder: folder.into(),
        offer: String::new(),
        teammate: voice_teammate,
        voice: locust_core::talk_shortcut::chosen_voice(store),
        preview: None,
        connectors: true,
        anchor_turn: s
            .ledger
            .as_ref()
            .map(|pane| pane.snapshot.tip.clone())
            .filter(|tip| !tip.is_empty())
            .or(conversation.conversation_id.clone()),
    })
}

fn talk(s: &mut State, tx: &mut Tx) {
    if let Some(mut call) = s.voice.call.take() {
        call.cancel.store(true, Ordering::Release);
        if let Some(service) = &s.voice.service {
            service.stop();
        }
        if let TalkState::Connected(_, window, _) = call.state {
            call.state = TalkState::Closing(window);
            s.voice.call = Some(call);
        }
        s.voice.questions.clear();
        tx.wake_after(Duration::from_millis(25));
        return;
    }
    cancel(s);
    let Some(audio) = s.voice.audio.clone() else {
        s.voice.note = Some("Talk needs native microphone, playback and WebRTC support.".into());
        return;
    };
    let Some(store) = s.data.clone() else { return };
    let Some(options) = call_options(s, &store) else {
        s.voice.note = Some("Pick a folder first.".into());
        return;
    };
    if s.voice.service.is_none() {
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        s.voice.service = Some(Arc::new(VoiceCalls::new(Arc::new(NativeHost::new(
            store, exe,
        )))));
    }
    s.voice.note = None;
    let (send, receive) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let cancelled = cancel.clone();
    std::thread::spawn(move || {
        let _ = send.send(audio.call(&cancelled));
    });
    s.voice.call = Some(Talk {
        cancel,
        target: conversation_draft_key(s),
        conversation: selected_conversation(s)
            .and_then(|(_, conversation)| conversation.conversation_id.clone()),
        state: TalkState::Opening(receive, options),
    });
    tx.wake_after(Duration::from_millis(25));
}

fn poll_call(s: &mut State, now: f64, tx: &mut Tx) -> bool {
    let Some(mut call) = s.voice.call.take() else {
        return false;
    };
    if call.target != conversation_draft_key(s) && !matches!(call.state, TalkState::Closing(_)) {
        s.voice.call = Some(call);
        talk(s, tx);
        return true;
    }
    let mut changed = false;
    let mut ended = false;
    match &mut call.state {
        TalkState::Opening(receive, options) => match receive.try_recv() {
            Ok(Ok(peer)) => {
                options.offer = peer.offer().to_owned();
                let Some(service) = &s.voice.service else {
                    return true;
                };
                match service.start(options.clone()) {
                    Ok(window) => call.state = TalkState::Connected(peer, window, false),
                    Err(why) => {
                        s.voice.note = Some(why);
                        ended = true;
                    }
                }
                changed = true;
            }
            Ok(Err(why)) => {
                s.voice.note = Some(why);
                ended = true;
            }
            Err(TryRecvError::Disconnected) => {
                s.voice.note = Some("Talk could not open the microphone.".into());
                ended = true;
            }
            Err(TryRecvError::Empty) => {}
        },
        TalkState::Connected(peer, window, connected) => {
            if !*connected {
                match window.started.try_recv() {
                    Ok(Ok(sdp)) => match peer.accept(&sdp) {
                        Ok(()) => *connected = true,
                        Err(why) => {
                            s.voice.note = Some(why);
                            ended = true;
                        }
                    },
                    Ok(Err(why)) => {
                        s.voice.note = Some(why);
                        ended = true;
                    }
                    Err(TryRecvError::Disconnected) => {
                        s.voice.note = Some("Talk could not start.".into());
                        ended = true;
                    }
                    Err(TryRecvError::Empty) => {}
                }
                changed |= *connected;
            }
            while let Ok(event) = window.events.try_recv() {
                match event {
                    CallEvent::Asks { id, question } => s.voice.questions.push((id, question)),
                    CallEvent::Asked { id, .. } => s.voice.questions.retain(|(key, _)| *key != id),
                    CallEvent::Ended(why) => {
                        if why.is_some() {
                            s.voice.note = why;
                        }
                        ended = true;
                    }
                    CallEvent::Error(why) => s.voice.note = Some(why),
                    _ => continue,
                }
                changed = true;
            }
        }
        TalkState::Closing(window) => {
            ended = match window.events.try_recv() {
                Ok(CallEvent::Ended(_)) | Err(TryRecvError::Disconnected) => true,
                _ => false,
            };
        }
    }
    if ended {
        call.cancel.store(true, Ordering::Release);
        if let Some(service) = &s.voice.service {
            service.stop();
        }
        s.voice.questions.clear();
        wake_ledger_pane(s, now, tx);
    } else {
        s.voice.call = Some(call);
    }
    changed || ended
}

pub(super) fn tick(s: &mut State, now: f64, tx: &mut Tx) -> bool {
    let capture = poll_capture(s);
    let call = poll_call(s, now, tx);
    if s.voice.capture.is_some() || s.voice.call.is_some() {
        tx.wake_after(Duration::from_millis(25));
    }
    capture || call
}

pub(super) fn calling(s: &State, id: &str) -> bool {
    s.voice.call.as_ref().is_some_and(|call| {
        call.conversation.as_deref() == Some(id)
            && matches!(call.state, TalkState::Connected(_, _, true))
    })
}

pub(super) fn controls(s: &State, p: &Palette) -> Option<El> {
    if !voice_audio::talk_runs_on(platform(s)) || s.demo || s.voice.audio.is_none() {
        return None;
    }
    let mic_label = match s.voice.capture.as_ref().map(|capture| &capture.state) {
        Some(CaptureState::Opening(_)) => "Opening microphone…",
        Some(CaptureState::Recording(..)) => "Stop voice typing",
        Some(CaptureState::Transcribing(_)) => "Transcribing…",
        None => "Voice typing",
    };
    let call_label = if s.voice.call.is_some() {
        "End call"
    } else {
        "Talk"
    };
    let mut controls = row().gap(6.0).center_y().child(
        text(mic_label)
            .key("voice-typing")
            .role("button")
            .font(type_::LABEL)
            .color(p.secondary)
            .on_press_tx(mic),
    );
    if s.ledger
        .as_ref()
        .is_some_and(|pane| pane.header.folder.is_some())
        || s.home.folder_path.is_some()
        || s.voice.call.is_some()
    {
        controls = controls.child(text(call_label).key("talk").role("button").font(type_::LABEL).color(p.secondary)
            .description("Talk through ChatGPT voice. Read the folder, ask about connectors, and hand work to teammates.")
            .on_press_tx(talk));
    }
    Some(controls)
}

pub(super) fn notices(s: &State, p: &Palette) -> Vec<El> {
    let mut notices = Vec::new();
    if let Some(note) = &s.voice.note {
        notices.push(
            text(note.clone())
                .key("voice-note")
                .font(type_::LABEL)
                .color(p.muted)
                .wrap(),
        );
    }
    for (id, question) in &s.voice.questions {
        let mut card = col()
            .key(format!("voice-question-{id}"))
            .gap(6.0)
            .padding(10.0)
            .radius(8.0)
            .fill(p.raised)
            .child(
                text("Approval required — exact action")
                    .font(type_::LABEL)
                    .color(p.amber),
            )
            .child(text(question.what.clone()).font(type_::LABEL).color(p.text))
            .child(
                text("A connector needs your approval. This action cannot be undone in Locust.")
                    .font(type_::LABEL)
                    .color(p.muted)
                    .wrap(),
            );
        if s.voice
            .call
            .as_ref()
            .is_some_and(|call| call.conversation.is_none())
            && !question.detail.is_empty()
        {
            card = card.child(
                text(question.detail.clone())
                    .font(type_::LABEL)
                    .color(p.text)
                    .wrap(),
            );
        } else {
            for (name, value) in &question.fields {
                card = card.child(
                    text(format!("{name}: {value}"))
                        .font(type_::LABEL)
                        .color(p.text)
                        .wrap(),
                );
            }
        }
        let allow = id.clone();
        let deny = id.clone();
        notices.push(
            card.child(
                row()
                    .gap(12.0)
                    .child(
                        text("Deny")
                            .role("button")
                            .key(format!("voice-deny-{id}"))
                            .font(type_::LABEL)
                            .color(p.text)
                            .on_press(move |s: &mut State| {
                                if let Some(service) = &s.voice.service {
                                    service.answer(deny.clone(), false);
                                }
                            }),
                    )
                    .child(
                        text("Allow once")
                            .role("button")
                            .key(format!("voice-allow-{id}"))
                            .font(type_::LABEL)
                            .color(p.text)
                            .on_press(move |s: &mut State| {
                                if let Some(service) = &s.voice.service {
                                    service.answer(allow.clone(), true);
                                }
                            }),
                    ),
            ),
        );
    }
    notices
}

#[cfg(test)]
#[path = "voice_panel_tests.rs"]
mod tests;
