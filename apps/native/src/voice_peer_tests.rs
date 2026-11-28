use super::*;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Default)]
struct Heard {
    input: Option<peer::Input>,
    output: Option<peer::Output>,
    order: Vec<&'static str>,
    fed: Vec<Vec<f32>>,
}

type Shared = Arc<Mutex<Heard>>;

#[derive(Clone)]
struct FakeDevices {
    shared: Shared,
    speaker_fails: bool,
    cancel_after_mic: Option<Arc<AtomicBool>>,
}

struct Lease(Shared, &'static str);

impl Drop for Lease {
    fn drop(&mut self) {
        let mut shared = self.0.lock().unwrap();
        shared.order.push(self.1);
        match self.1 {
            "mic shut" => {
                shared.input.take();
            }
            "speaker shut" => {
                shared.output.take();
            }
            _ => {}
        }
    }
}

impl peer::Devices for FakeDevices {
    fn microphone(&self, input: peer::Input) -> Result<Box<dyn Send>, String> {
        let mut shared = self.shared.lock().unwrap();
        shared.order.push("mic open");
        shared.input = Some(input);
        if let Some(cancel) = &self.cancel_after_mic {
            cancel.store(true, Ordering::Release);
        }
        Ok(Box::new(Lease(self.shared.clone(), "mic shut")))
    }

    fn speaker(&self, output: peer::Output) -> Result<Box<dyn Send>, String> {
        let mut shared = self.shared.lock().unwrap();
        shared.order.push("speaker tried");
        if self.speaker_fails {
            return Err("audio device busy".into());
        }
        shared.output = Some(output);
        Ok(Box::new(Lease(self.shared.clone(), "speaker shut")))
    }
}

struct Loopback {
    _lease: Lease,
    echo: VecDeque<Vec<f32>>,
}

impl peer::Transport for Loopback {
    fn offer(&self) -> &str {
        "loopback offer"
    }

    fn accept(&mut self, answer: &str) -> Result<(), String> {
        self._lease.0.lock().unwrap().order.push("peer answer");
        if answer == "loopback answer" {
            Ok(())
        } else {
            Err("The answer was refused.".into())
        }
    }

    fn send(&mut self, samples: &[f32]) -> Result<(), String> {
        self._lease.0.lock().unwrap().fed.push(samples.to_vec());
        self.echo.push_back(samples.iter().map(|v| -*v).collect());
        Ok(())
    }

    fn receive(&mut self) -> Result<Option<Vec<f32>>, String> {
        Ok(self.echo.pop_front())
    }
}

fn fixture(
    speaker_fails: bool,
    cancel_after_mic: Option<Arc<AtomicBool>>,
) -> (NativeAudio, Shared) {
    let shared: Shared = Arc::default();
    let transport = shared.clone();
    let access = shared.clone();
    let audio = NativeAudio {
        access: Box::new(move |_| {
            access.lock().unwrap().order.push("access allowed");
            Access::Allowed
        }),
        peer: Some(Box::new(move |_| {
            transport.lock().unwrap().order.push("peer open");
            Ok(Box::new(Loopback {
                _lease: Lease(transport.clone(), "peer shut"),
                echo: VecDeque::new(),
            }))
        })),
        call_devices: Box::new(FakeDevices {
            shared: shared.clone(),
            speaker_fails,
            cancel_after_mic,
        }),
        ..NativeAudio::new()
    };
    (audio, shared)
}

fn next_sample(shared: &Shared) -> f32 {
    shared.lock().unwrap().output.as_mut().unwrap().next()
}

#[test]
fn talk_peer_sends_and_plays_samples_only_after_the_answer_and_ends_cleanly() {
    let (audio, shared) = fixture(false, None);
    let mut call = audio.call(&AtomicBool::new(false)).unwrap();
    assert_eq!(call.offer(), "loopback offer");
    assert_eq!(
        &shared.lock().unwrap().order,
        &["access allowed", "peer open", "mic open", "speaker tried"]
    );
    shared
        .lock()
        .unwrap()
        .input
        .as_ref()
        .unwrap()
        .samples(&[0.9]);
    assert_eq!(next_sample(&shared), 0.0);
    assert!(shared.lock().unwrap().fed.is_empty());

    call.accept("loopback answer").unwrap();
    shared
        .lock()
        .unwrap()
        .input
        .as_ref()
        .unwrap()
        .samples(&[0.25, -0.5, 0.75]);
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let heard = next_sample(&shared);
        if heard != 0.0 {
            assert_eq!(heard, -0.25);
            break;
        }
        assert!(Instant::now() < deadline, "no playback arrived");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(next_sample(&shared), 0.5);
    assert_eq!(next_sample(&shared), -0.75);
    assert_eq!(next_sample(&shared), 0.0);
    assert_eq!(&shared.lock().unwrap().fed, &[vec![0.25, -0.5, 0.75]]);
    drop(call);
    let ended = shared.lock().unwrap();
    assert!(ended.input.is_none());
    assert!(ended.output.is_none());
    assert_eq!(
        ended
            .order
            .iter()
            .filter(|&&event| event == "peer shut")
            .count(),
        1
    );
    assert_eq!(
        ended
            .order
            .iter()
            .filter(|&&event| event == "mic shut")
            .count(),
        1
    );
    assert_eq!(
        ended
            .order
            .iter()
            .filter(|&&event| event == "speaker shut")
            .count(),
        1
    );
}

#[test]
fn talk_peer_cancel_before_open_never_asks_and_cancel_after_mic_does_not_open_speaker() {
    let (audio, shared) = fixture(false, None);
    assert_eq!(
        audio.call(&AtomicBool::new(true)).err().unwrap(),
        peer::CANCELLED
    );
    assert!(shared.lock().unwrap().order.is_empty());

    let cancel = Arc::new(AtomicBool::new(false));
    let (audio, shared) = fixture(false, Some(cancel.clone()));
    assert_eq!(audio.call(&cancel).err().unwrap(), peer::CANCELLED);
    assert_eq!(
        &shared.lock().unwrap().order,
        &[
            "access allowed",
            "peer open",
            "mic open",
            "mic shut",
            "peer shut"
        ]
    );
}

#[test]
fn talk_peer_a_refused_answer_closes_both_audio_devices_before_returning() {
    let (audio, shared) = fixture(false, None);
    let mut call = audio.call(&AtomicBool::new(false)).unwrap();
    assert_eq!(
        call.accept("bad answer").unwrap_err(),
        "The answer was refused."
    );
    let (mic_closed, speaker_closed, silent, peer_closed) = {
        let state = shared.lock().unwrap();
        (
            state.input.is_none(),
            state.output.is_none(),
            state.fed.is_empty(),
            state.order.contains(&"peer shut"),
        )
    };
    assert!(mic_closed);
    assert!(speaker_closed);
    assert!(silent);
    assert!(peer_closed);
}

#[test]
fn talk_peer_an_unanswered_offer_releases_everything_when_dropped() {
    let (audio, shared) = fixture(false, None);
    drop(audio.call(&AtomicBool::new(false)).unwrap());
    let state = shared.lock().unwrap();
    assert!(state.input.is_none());
    assert!(state.output.is_none());
    assert!(state.order.contains(&"peer shut"));
    assert!(!state.order.contains(&"peer answer"));
}
