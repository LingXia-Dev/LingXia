use super::*;
use crate::audio::protocol::SessionOutput;
use crate::audio::types::RouteOutput;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

struct Harness {
    coordinator: Coordinator,
    commands: Arc<Mutex<Vec<Command>>>,
    next_req: u64,
}

struct TestPlayer {
    id: u64,
    shared: Arc<Mutex<PlayerSnapshot>>,
    events: UnboundedReceiver<PlayerEvent>,
}

impl TestPlayer {
    fn drain(&mut self) -> Vec<PlayerEvent> {
        let mut out = Vec::new();
        while let Ok(event) = self.events.try_recv() {
            out.push(event);
        }
        out
    }

    fn status(&self) -> PlayerStatus {
        self.shared.lock().unwrap().status
    }
}

fn entry(name: &str) -> QueueEntry {
    QueueEntry {
        uri: format!("/media/{name}.mp3"),
        headers: BTreeMap::new(),
        mime_type: None,
        start_ms: 0,
    }
}

fn statuses(events: &[PlayerEvent]) -> Vec<PlayerStatus> {
    events
        .iter()
        .filter_map(|e| match e {
            PlayerEvent::State(s) => Some(s.status),
            _ => None,
        })
        .collect()
}

fn replies(events: &[PlayerEvent]) -> Vec<(u64, Result<Reply, AudioErrorCode>)> {
    events
        .iter()
        .filter_map(|e| match e {
            PlayerEvent::Reply { req, result } => {
                Some((*req, result.clone().map_err(|error| error.code)))
            }
            _ => None,
        })
        .collect()
}

fn interruptions(events: &[PlayerEvent]) -> Vec<Interruption> {
    events
        .iter()
        .filter_map(|e| match e {
            PlayerEvent::Interruption(i) => Some(*i),
            _ => None,
        })
        .collect()
}

impl Harness {
    fn new(capabilities: AudioDeviceCapabilities) -> Self {
        let commands = Arc::new(Mutex::new(Vec::new()));
        let sink_commands = commands.clone();
        let coordinator = Coordinator::new(
            Box::new(move |command| sink_commands.lock().unwrap().push(command)),
            capabilities,
            Arc::new(Mutex::new(Route::default())),
        );
        Self {
            coordinator,
            commands,
            next_req: 1000,
        }
    }

    fn take(&self) -> Vec<Command> {
        std::mem::take(&mut *self.commands.lock().unwrap())
    }

    fn player(&mut self, id: u64, owner: OwnerId, options: PlayerOptions) -> TestPlayer {
        let shared = Arc::new(Mutex::new(PlayerSnapshot::new(&options, Route::default())));
        let (tx, events) = unbounded_channel();
        self.coordinator.handle(Input::CreatePlayer {
            id,
            owner,
            options,
            shared: shared.clone(),
            tx,
        });
        self.take();
        TestPlayer { id, shared, events }
    }

    fn command(&mut self, player: &TestPlayer, command: PlayerCommand) -> u64 {
        self.next_req += 1;
        let req = self.next_req;
        self.coordinator.handle(Input::Player {
            id: player.id,
            req,
            command,
        });
        req
    }

    fn native(&mut self, event: NativeEvent) {
        self.coordinator.handle(Input::Native(event));
    }

    fn last_load_req(&self, id: u64) -> u64 {
        self.commands
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find_map(|c| match c {
                Command::PlayerLoad { id: i, req, .. } if *i == id => Some(*req),
                _ => None,
            })
            .expect("a load was issued")
    }

    /// Load `items`, confirm natively, and leave the player paused.
    fn loaded(&mut self, player: &mut TestPlayer, items: Vec<QueueEntry>) {
        self.command(
            player,
            PlayerCommand::SetQueue {
                items,
                start_index: 0,
                autoplay: false,
            },
        );
        let req = self.last_load_req(player.id);
        self.native(NativeEvent::PlayerLoaded {
            id: player.id,
            req,
            duration_ms: Some(10_000),
        });
        self.take();
        player.drain();
    }

    fn playing(&mut self, player: &mut TestPlayer) {
        self.command(player, PlayerCommand::Play);
        self.native(NativeEvent::PlayerPlaying {
            id: player.id,
            position_ms: Some(0),
        });
        player.drain();
    }
}

fn phone() -> AudioDeviceCapabilities {
    AudioDeviceCapabilities {
        earpiece: true,
        proximity: true,
    }
}

#[test]
fn load_then_play_walks_the_lifecycle_and_resolves_after_each_transition() {
    let mut h = Harness::new(phone());
    let mut p = h.player(1, 7, PlayerOptions::default());
    let load = h.command(
        &p,
        PlayerCommand::SetQueue {
            items: vec![entry("a")],
            start_index: 0,
            autoplay: false,
        },
    );
    let commands = h.take();
    assert!(matches!(commands[0], Command::PlayerLoad { id: 1, .. }));
    let events = p.drain();
    assert_eq!(statuses(&events), vec![PlayerStatus::Loading]);
    assert!(events.iter().any(|e| matches!(
        e,
        PlayerEvent::Item {
            index: 0,
            reason: ItemReason::Load
        }
    )));
    assert!(replies(&events).is_empty());

    let req = match commands[0] {
        Command::PlayerLoad { req, .. } => req,
        _ => unreachable!(),
    };
    h.native(NativeEvent::PlayerLoaded {
        id: 1,
        req,
        duration_ms: Some(4_000),
    });
    let events = p.drain();
    assert_eq!(statuses(&events), vec![PlayerStatus::Paused]);
    assert_eq!(
        replies(&events),
        vec![(
            load,
            Ok(Reply::Loaded {
                duration_ms: Some(4_000),
                index: 0
            })
        )]
    );

    let play = h.command(&p, PlayerCommand::Play);
    let commands = h.take();
    assert_eq!(
        commands,
        vec![
            Command::Session {
                active: true,
                focus: Some(Focus::Exclusive),
                obey_silent_switch: false,
                output: SessionOutput::Speaker,
                proximity: false,
            },
            Command::PlayerPlay { id: 1 },
        ]
    );
    assert!(replies(&p.drain()).is_empty(), "play waits for audio");
    h.native(NativeEvent::PlayerPlaying {
        id: 1,
        position_ms: Some(0),
    });
    let events = p.drain();
    assert_eq!(statuses(&events), vec![PlayerStatus::Playing]);
    assert_eq!(replies(&events), vec![(play, Ok(Reply::Done))]);
    // The state event precedes the reply that it caused.
    assert!(matches!(events.last(), Some(PlayerEvent::Reply { .. })));

    let pause = h.command(&p, PlayerCommand::Pause);
    let events = p.drain();
    assert_eq!(statuses(&events), vec![PlayerStatus::Paused]);
    assert_eq!(replies(&events), vec![(pause, Ok(Reply::Done))]);
    assert_eq!(
        h.take(),
        vec![
            Command::PlayerPause { id: 1 },
            SessionSpec::INACTIVE.command()
        ]
    );
}

#[test]
fn play_before_load_rejects_and_failed_load_reports_error() {
    let mut h = Harness::new(phone());
    let mut p = h.player(1, 7, PlayerOptions::default());
    let play = h.command(&p, PlayerCommand::Play);
    assert_eq!(
        replies(&p.drain()),
        vec![(play, Err(AudioErrorCode::InvalidSource))]
    );

    let load = h.command(
        &p,
        PlayerCommand::SetQueue {
            items: vec![entry("missing")],
            start_index: 0,
            autoplay: true,
        },
    );
    let req = h.last_load_req(1);
    h.native(NativeEvent::PlayerError {
        id: 1,
        req: Some(req),
        code: "source_not_found".into(),
        message: "no such file".into(),
    });
    let events = p.drain();
    assert_eq!(
        statuses(&events),
        vec![PlayerStatus::Loading, PlayerStatus::Failed]
    );
    assert_eq!(
        replies(&events),
        vec![(load, Err(AudioErrorCode::SourceNotFound))]
    );
    assert!(events.iter().any(|e| matches!(
        e,
        PlayerEvent::Error { error, index: Some(0) } if error.code == AudioErrorCode::SourceNotFound
    )));
    assert_eq!(
        p.shared.lock().unwrap().error.as_ref().map(|e| e.code),
        Some(AudioErrorCode::SourceNotFound)
    );
}

#[test]
fn a_superseded_load_answer_is_ignored() {
    let mut h = Harness::new(phone());
    let mut p = h.player(1, 7, PlayerOptions::default());
    let first = h.command(
        &p,
        PlayerCommand::SetQueue {
            items: vec![entry("a")],
            start_index: 0,
            autoplay: false,
        },
    );
    let stale = h.last_load_req(1);
    let second = h.command(
        &p,
        PlayerCommand::SetQueue {
            items: vec![entry("b")],
            start_index: 0,
            autoplay: false,
        },
    );
    let current = h.last_load_req(1);
    h.native(NativeEvent::PlayerError {
        id: 1,
        req: Some(stale),
        code: "network".into(),
        message: String::new(),
    });
    assert_eq!(p.status(), PlayerStatus::Loading);
    h.native(NativeEvent::PlayerLoaded {
        id: 1,
        req: current,
        duration_ms: None,
    });
    let events = p.drain();
    assert_eq!(
        replies(&events),
        vec![
            (first, Err(AudioErrorCode::Interrupted)),
            (
                second,
                Ok(Reply::Loaded {
                    duration_ms: None,
                    index: 0
                })
            )
        ]
    );
    assert_eq!(p.status(), PlayerStatus::Paused);
}

#[test]
fn one_exclusive_player_at_a_time_but_mix_plays_alongside() {
    let mut h = Harness::new(phone());
    let mut a = h.player(1, 7, PlayerOptions::default());
    let mut b = h.player(2, 8, PlayerOptions::default());
    let mut m = h.player(
        3,
        8,
        PlayerOptions {
            focus: Focus::Mix,
            obey_silent_switch: true,
            ..PlayerOptions::default()
        },
    );
    for p in [&mut a, &mut b, &mut m] {
        h.loaded(p, vec![entry("x")]);
    }
    h.playing(&mut a);
    h.playing(&mut m);
    h.take();

    h.command(&b, PlayerCommand::Play);
    let commands = h.take();
    assert!(commands.contains(&Command::PlayerPause { id: 1 }));
    assert!(!commands.contains(&Command::PlayerPause { id: 3 }));
    let events = a.drain();
    assert_eq!(statuses(&events), vec![PlayerStatus::Paused]);
    assert_eq!(
        interruptions(&events),
        vec![Interruption {
            phase: InterruptionPhase::Began,
            reason: InterruptionReason::FocusTaken,
            should_resume: false,
            resumed: false,
        }]
    );
    assert!(m.drain().is_empty());
}

#[test]
fn queue_advances_repeats_and_ends() {
    let mut h = Harness::new(phone());
    let mut p = h.player(1, 7, PlayerOptions::default());
    h.loaded(&mut p, vec![entry("a"), entry("b")]);
    h.playing(&mut p);
    h.take();

    h.native(NativeEvent::PlayerEnded { id: 1 });
    let events = p.drain();
    assert!(matches!(events[0], PlayerEvent::Ended { index: 0 }));
    assert!(events.iter().any(|e| matches!(
        e,
        PlayerEvent::Item {
            index: 1,
            reason: ItemReason::Auto
        }
    )));
    let req = h.last_load_req(1);
    h.native(NativeEvent::PlayerLoaded {
        id: 1,
        req,
        duration_ms: Some(1_000),
    });
    // Focus is still held across the item change: play, no new session.
    assert_eq!(h.take().last(), Some(&Command::PlayerPlay { id: 1 }));
    h.native(NativeEvent::PlayerPlaying {
        id: 1,
        position_ms: Some(0),
    });
    p.drain();

    h.native(NativeEvent::PlayerEnded { id: 1 });
    let events = p.drain();
    assert_eq!(statuses(&events), vec![PlayerStatus::Ended]);
    assert_eq!(h.take(), vec![SessionSpec::INACTIVE.command()]);

    // play() after the end restarts the queue from the first item.
    let play = h.command(&p, PlayerCommand::Play);
    let events = p.drain();
    assert!(events.iter().any(|e| matches!(
        e,
        PlayerEvent::Item {
            index: 0,
            reason: ItemReason::Load
        }
    )));
    let req = h.last_load_req(1);
    h.native(NativeEvent::PlayerLoaded {
        id: 1,
        req,
        duration_ms: Some(1_000),
    });
    h.native(NativeEvent::PlayerPlaying {
        id: 1,
        position_ms: Some(0),
    });
    assert_eq!(replies(&p.drain()), vec![(play, Ok(Reply::Done))]);

    p.shared.lock().unwrap().repeat = Repeat::One;
    h.command(&p, PlayerCommand::ApplySettings);
    h.take();
    h.native(NativeEvent::PlayerEnded { id: 1 });
    let commands = h.take();
    assert!(matches!(
        commands[0],
        Command::PlayerSeek { position_ms: 0, .. }
    ));
    assert_eq!(commands[1], Command::PlayerPlay { id: 1 });
}

#[test]
fn next_previous_and_skip_validate_the_queue() {
    let mut h = Harness::new(phone());
    let mut p = h.player(1, 7, PlayerOptions::default());
    h.loaded(&mut p, vec![entry("a"), entry("b")]);
    let previous = h.command(&p, PlayerCommand::Previous);
    // At index 0 near the start, previous rewinds instead.
    assert!(matches!(
        h.take()[0],
        Command::PlayerSeek { position_ms: 0, .. }
    ));
    let seek_req = match p.shared.lock().unwrap().seeking {
        true => h.coordinator.players[&1].seek.unwrap().0,
        false => panic!("seeking"),
    };
    h.native(NativeEvent::PlayerSeeked {
        id: 1,
        req: seek_req,
        position_ms: 0,
    });
    assert_eq!(replies(&p.drain()), vec![(previous, Ok(Reply::Done))]);

    let skip = h.command(&p, PlayerCommand::SkipTo(5));
    assert_eq!(
        replies(&p.drain()),
        vec![(skip, Err(AudioErrorCode::InvalidOptions))]
    );
    let next = h.command(&p, PlayerCommand::Next);
    let events = p.drain();
    assert!(events.iter().any(|e| matches!(
        e,
        PlayerEvent::Item {
            index: 1,
            reason: ItemReason::Next
        }
    )));
    let req = h.last_load_req(1);
    h.native(NativeEvent::PlayerLoaded {
        id: 1,
        req,
        duration_ms: None,
    });
    assert_eq!(
        replies(&p.drain()),
        vec![(
            next,
            Ok(Reply::Loaded {
                duration_ms: None,
                index: 1
            })
        )]
    );
    let beyond = h.command(&p, PlayerCommand::Next);
    assert_eq!(
        replies(&p.drain()),
        vec![(beyond, Err(AudioErrorCode::InvalidOptions))]
    );
}

#[test]
fn external_interruption_resumes_only_when_asked_and_hinted() {
    let mut h = Harness::new(phone());
    let mut keep = h.player(
        1,
        7,
        PlayerOptions {
            resume_after_interruption: true,
            ..PlayerOptions::default()
        },
    );
    h.loaded(&mut keep, vec![entry("a")]);
    h.playing(&mut keep);
    let (host_tx, mut host) = unbounded_channel();
    h.coordinator.handle(Input::RegisterOwner {
        owner: 7,
        tx: host_tx,
    });
    h.take();

    h.native(NativeEvent::Interruption {
        phase: InterruptionPhase::Began,
        reason: InterruptionReason::Call,
        should_resume: false,
    });
    assert_eq!(keep.status(), PlayerStatus::Paused);
    assert_eq!(
        interruptions(&keep.drain())[0].reason,
        InterruptionReason::Call
    );
    h.native(NativeEvent::Interruption {
        phase: InterruptionPhase::Ended,
        reason: InterruptionReason::Call,
        should_resume: true,
    });
    let ended = interruptions(&keep.drain());
    assert_eq!(ended.len(), 1);
    assert!(ended[0].resumed);
    assert!(h.take().contains(&Command::PlayerPlay { id: 1 }));
    let mut host_events = Vec::new();
    while let Ok(event) = host.try_recv() {
        host_events.push(event);
    }
    assert!(matches!(
        host_events.as_slice(),
        [HostEvent::Interruption(began), HostEvent::Interruption(ended)]
            if began.phase == InterruptionPhase::Began && ended.resumed
    ));
}

#[test]
fn route_loss_pauses_focus_holders_but_not_mix() {
    let mut h = Harness::new(phone());
    let mut music = h.player(1, 7, PlayerOptions::default());
    let mut ambient = h.player(
        2,
        7,
        PlayerOptions {
            focus: Focus::Mix,
            ..PlayerOptions::default()
        },
    );
    h.loaded(&mut music, vec![entry("a")]);
    h.loaded(&mut ambient, vec![entry("b")]);
    h.playing(&mut music);
    h.playing(&mut ambient);
    h.native(NativeEvent::Route {
        output: RouteOutput::Headphones,
        name: Some("Wired".into()),
        reason: RouteReason::DeviceAdded,
        lost: false,
    });
    music.drain();
    ambient.drain();
    h.native(NativeEvent::Route {
        output: RouteOutput::Speaker,
        name: None,
        reason: RouteReason::Other,
        lost: true,
    });
    assert_eq!(music.status(), PlayerStatus::Paused);
    assert_eq!(ambient.status(), PlayerStatus::Playing);
    let events = ambient.drain();
    assert!(events.iter().any(|e| matches!(
        e,
        PlayerEvent::Route(RouteChange {
            reason: RouteReason::DeviceRemoved,
            ..
        })
    )));
    assert_eq!(
        ambient.shared.lock().unwrap().route.output,
        RouteOutput::Speaker
    );
}

#[test]
fn earpiece_needs_a_receiver_and_owns_the_output() {
    let mut desktop = Harness::new(AudioDeviceCapabilities::default());
    let mut p = desktop.player(
        1,
        7,
        PlayerOptions {
            output: OutputRequest::Earpiece,
            ..PlayerOptions::default()
        },
    );
    desktop.loaded(&mut p, vec![entry("a")]);
    let play = desktop.command(&p, PlayerCommand::Play);
    assert_eq!(
        replies(&p.drain()),
        vec![(play, Err(AudioErrorCode::OutputUnsupported))]
    );
    let set = desktop.command(&p, PlayerCommand::SetOutput(OutputRequest::Earpiece));
    assert_eq!(
        replies(&p.drain()),
        vec![(set, Err(AudioErrorCode::OutputUnsupported))]
    );

    let mut h = Harness::new(phone());
    let mut voice = h.player(
        1,
        7,
        PlayerOptions {
            focus: Focus::Duck,
            output: OutputRequest::Earpiece,
            ..PlayerOptions::default()
        },
    );
    h.loaded(&mut voice, vec![entry("v")]);
    h.command(&voice, PlayerCommand::Play);
    assert!(matches!(
        h.take()[0],
        Command::Session {
            output: SessionOutput::Earpiece,
            focus: Some(Focus::Duck),
            ..
        }
    ));
    h.command(&voice, PlayerCommand::SetOutput(OutputRequest::Speaker));
    assert!(matches!(
        h.take()[0],
        Command::Session {
            output: SessionOutput::Speaker,
            ..
        }
    ));
}

#[test]
fn proximity_switches_the_built_in_route() {
    let mut h = Harness::new(phone());
    let mut voice = h.player(
        1,
        7,
        PlayerOptions {
            focus: Focus::Duck,
            proximity: true,
            ..PlayerOptions::default()
        },
    );
    h.loaded(&mut voice, vec![entry("v")]);
    h.playing(&mut voice);
    h.take();
    h.native(NativeEvent::Proximity { near: true });
    assert!(matches!(
        h.take()[0],
        Command::Session {
            output: SessionOutput::Earpiece,
            proximity: true,
            ..
        }
    ));
    assert!(voice.drain().iter().any(|e| matches!(
        e,
        PlayerEvent::Route(RouteChange {
            reason: RouteReason::Proximity,
            route: Route {
                output: RouteOutput::Earpiece,
                ..
            }
        })
    )));
    h.native(NativeEvent::Proximity { near: false });
    assert!(matches!(
        h.take()[0],
        Command::Session {
            output: SessionOutput::Speaker,
            ..
        }
    ));
}

#[test]
fn hiding_the_lxapp_pauses_its_players_without_resuming_on_show() {
    let mut h = Harness::new(phone());
    let mut mine = h.player(1, 7, PlayerOptions::default());
    let mut other = h.player(
        2,
        8,
        PlayerOptions {
            focus: Focus::Mix,
            ..PlayerOptions::default()
        },
    );
    h.loaded(&mut mine, vec![entry("a")]);
    h.loaded(&mut other, vec![entry("b")]);
    h.playing(&mut mine);
    h.playing(&mut other);
    h.coordinator.handle(Input::OwnerVisible {
        owner: 7,
        visible: false,
    });
    assert_eq!(mine.status(), PlayerStatus::Paused);
    assert_eq!(other.status(), PlayerStatus::Playing);
    assert_eq!(
        interruptions(&mine.drain())[0].reason,
        InterruptionReason::AppHidden
    );
    h.coordinator.handle(Input::OwnerVisible {
        owner: 7,
        visible: true,
    });
    let shown = interruptions(&mine.drain());
    assert_eq!(shown[0].phase, InterruptionPhase::Ended);
    assert!(shown[0].should_resume && !shown[0].resumed);
    assert_eq!(mine.status(), PlayerStatus::Paused);
}

#[test]
fn owner_shutdown_and_dispose_end_every_command() {
    let mut h = Harness::new(phone());
    let mut p = h.player(1, 7, PlayerOptions::default());
    h.loaded(&mut p, vec![entry("a")]);
    let dispose = h.command(&p, PlayerCommand::Dispose);
    let events = p.drain();
    assert_eq!(statuses(&events), vec![PlayerStatus::Disposed]);
    assert_eq!(replies(&events), vec![(dispose, Ok(Reply::Done))]);
    assert!(h.take().contains(&Command::PlayerDispose { id: 1 }));
    // The channel closes with the player.
    assert!(p.events.try_recv().is_err());

    let mut q = h.player(2, 9, PlayerOptions::default());
    h.loaded(&mut q, vec![entry("a")]);
    h.coordinator.handle(Input::OwnerShutdown { owner: 9 });
    assert_eq!(q.status(), PlayerStatus::Disposed);
    assert!(h.coordinator.players.is_empty());
}

#[test]
fn video_and_players_share_exclusive_focus() {
    let mut h = Harness::new(phone());
    let mut p = h.player(1, 7, PlayerOptions::default());
    h.loaded(&mut p, vec![entry("a")]);
    h.playing(&mut p);
    h.take();
    h.native(NativeEvent::Video {
        component_id: "v1".into(),
        playing: true,
    });
    assert_eq!(p.status(), PlayerStatus::Paused);
    assert_eq!(
        interruptions(&p.drain())[0].reason,
        InterruptionReason::FocusTaken
    );
    h.take();
    h.command(&p, PlayerCommand::Play);
    assert!(h.take().contains(&Command::VideoPause {
        component_id: "v1".into()
    }));
    h.native(NativeEvent::Video {
        component_id: "v1".into(),
        playing: false,
    });
    assert!(h.coordinator.videos.is_empty());
}

#[test]
fn sounds_mix_steal_voices_and_report_ends() {
    let mut h = Harness::new(phone());
    let (tx, mut events) = unbounded_channel();
    let (reply, mut loaded) = oneshot::channel();
    h.coordinator.handle(Input::LoadSound {
        id: 5,
        owner: 7,
        options: SoundOptions {
            focus: Focus::Mix,
            obey_silent_switch: true,
            max_voices: 2,
        },
        path: "/tap.wav".into(),
        tx,
        reply,
    });
    let req = match h.take()[0] {
        Command::SoundLoad { req, .. } => req,
        _ => panic!("load"),
    };
    // A voice before the load completes ends at once.
    h.coordinator.handle(Input::PlayVoice {
        id: 5,
        voice: 1,
        params: VoiceParams {
            volume: 1.0,
            rate: 1.0,
            pan: 0.0,
            looping: false,
        },
    });
    assert_eq!(events.try_recv(), Ok(SoundEvent::VoiceEnded(1)));
    h.native(NativeEvent::SoundLoaded {
        id: 5,
        req,
        duration_ms: 120,
    });
    assert_eq!(loaded.try_recv(), Ok(Ok(120)));
    let params = VoiceParams {
        volume: 0.5,
        rate: 1.0,
        pan: -1.0,
        looping: false,
    };
    for voice in [2, 3, 4] {
        h.coordinator.handle(Input::PlayVoice {
            id: 5,
            voice,
            params,
        });
    }
    let commands = h.take();
    assert!(matches!(
        commands[0],
        Command::Session {
            focus: Some(Focus::Mix),
            obey_silent_switch: true,
            ..
        }
    ));
    assert!(commands.contains(&Command::SoundStop { id: 5, voice: 2 }));
    assert_eq!(events.try_recv(), Ok(SoundEvent::VoiceEnded(2)));
    h.native(NativeEvent::SoundEnded { id: 5, voice: 3 });
    assert_eq!(events.try_recv(), Ok(SoundEvent::VoiceEnded(3)));
    h.coordinator.handle(Input::StopAllVoices { id: 5 });
    assert_eq!(events.try_recv(), Ok(SoundEvent::VoiceEnded(4)));
    assert_eq!(h.take().last(), Some(&SessionSpec::INACTIVE.command()));
}

#[test]
fn a_hidden_lxapp_cannot_start_a_player_until_it_shows_again() {
    let mut h = Harness::new(phone());
    let (tx, _rx) = unbounded_channel();
    h.coordinator.handle(Input::RegisterOwner { owner: 7, tx });
    let mut p = h.player(1, 7, PlayerOptions::default());
    h.loaded(&mut p, vec![entry("a")]);
    h.coordinator.handle(Input::OwnerVisible {
        owner: 7,
        visible: false,
    });
    p.drain();
    let play = h.command(&p, PlayerCommand::Play);
    let events = p.drain();
    assert_eq!(
        replies(&events),
        vec![(play, Err(AudioErrorCode::Interrupted))]
    );
    assert_eq!(
        interruptions(&events)[0].reason,
        InterruptionReason::AppHidden
    );
    assert_eq!(p.status(), PlayerStatus::Paused);
    assert!(!h.take().contains(&Command::PlayerPlay { id: 1 }));
    h.coordinator.handle(Input::OwnerVisible {
        owner: 7,
        visible: true,
    });
    let shown = interruptions(&p.drain());
    assert_eq!(shown[0].phase, InterruptionPhase::Ended);
    h.command(&p, PlayerCommand::Play);
    assert!(h.take().contains(&Command::PlayerPlay { id: 1 }));
}

#[test]
fn a_load_about_to_autoplay_yields_focus_like_a_live_player() {
    let mut h = Harness::new(phone());
    let mut first = h.player(1, 7, PlayerOptions::default());
    let mut second = h.player(2, 7, PlayerOptions::default());
    h.loaded(&mut second, vec![entry("b")]);
    let load = h.command(
        &first,
        PlayerCommand::SetQueue {
            items: vec![entry("a")],
            start_index: 0,
            autoplay: true,
        },
    );
    let first_req = h.last_load_req(1);
    first.drain();
    h.playing(&mut second);
    // The second player took exclusive focus while the first was still loading.
    assert_eq!(
        interruptions(&first.drain())[0].reason,
        InterruptionReason::FocusTaken
    );
    h.take();
    h.native(NativeEvent::PlayerLoaded {
        id: 1,
        req: first_req,
        duration_ms: Some(1_000),
    });
    let events = first.drain();
    assert_eq!(
        replies(&events),
        vec![(
            load,
            Ok(Reply::Loaded {
                duration_ms: Some(1_000),
                index: 0
            })
        )]
    );
    assert_eq!(first.status(), PlayerStatus::Paused);
    assert!(!h.take().contains(&Command::PlayerPlay { id: 1 }));
    assert_eq!(second.status(), PlayerStatus::Playing);
}

#[test]
fn a_track_change_while_playing_goes_from_loading_straight_to_playing() {
    let mut h = Harness::new(phone());
    let mut p = h.player(1, 7, PlayerOptions::default());
    h.loaded(&mut p, vec![entry("a"), entry("b")]);
    h.playing(&mut p);
    h.take();

    let next = h.command(&p, PlayerCommand::Next);
    let req = h.last_load_req(1);
    h.native(NativeEvent::PlayerLoaded {
        id: 1,
        req,
        duration_ms: Some(3_000),
    });
    assert_eq!(p.status(), PlayerStatus::Loading);
    assert!(h.take().contains(&Command::PlayerPlay { id: 1 }));
    h.native(NativeEvent::PlayerPlaying {
        id: 1,
        position_ms: Some(0),
    });
    let events = p.drain();
    assert_eq!(
        statuses(&events),
        vec![
            PlayerStatus::Loading,
            PlayerStatus::Loading,
            PlayerStatus::Playing
        ]
    );
    assert!(replies(&events).contains(&(
        next,
        Ok(Reply::Loaded {
            duration_ms: Some(3_000),
            index: 1
        })
    )));
}

#[test]
fn a_pause_before_the_loaded_item_starts_settles_paused() {
    let mut h = Harness::new(phone());
    let p = h.player(1, 7, PlayerOptions::default());
    h.command(
        &p,
        PlayerCommand::SetQueue {
            items: vec![entry("a")],
            start_index: 0,
            autoplay: true,
        },
    );
    let req = h.last_load_req(1);
    h.native(NativeEvent::PlayerLoaded {
        id: 1,
        req,
        duration_ms: Some(3_000),
    });
    assert_eq!(p.status(), PlayerStatus::Loading);
    h.take();

    h.command(&p, PlayerCommand::Pause);
    assert_eq!(p.status(), PlayerStatus::Paused);
    assert!(h.take().contains(&Command::PlayerPause { id: 1 }));
    // The native start that was already on its way is ignored.
    h.native(NativeEvent::PlayerPlaying {
        id: 1,
        position_ms: Some(0),
    });
    assert_eq!(p.status(), PlayerStatus::Paused);
}

#[test]
fn an_autoplay_load_refused_for_a_hidden_lxapp_settles_paused() {
    let mut h = Harness::new(phone());
    let (tx, _rx) = unbounded_channel();
    h.coordinator.handle(Input::RegisterOwner { owner: 7, tx });
    let p = h.player(1, 7, PlayerOptions::default());
    h.coordinator.handle(Input::OwnerVisible {
        owner: 7,
        visible: false,
    });
    h.command(
        &p,
        PlayerCommand::SetQueue {
            items: vec![entry("a")],
            start_index: 0,
            autoplay: true,
        },
    );
    let req = h.last_load_req(1);
    h.native(NativeEvent::PlayerLoaded {
        id: 1,
        req,
        duration_ms: Some(3_000),
    });
    assert_eq!(p.status(), PlayerStatus::Paused);
    assert!(!h.take().contains(&Command::PlayerPlay { id: 1 }));
}

#[test]
fn an_interruption_reported_after_its_native_pause_still_resumes() {
    let mut h = Harness::new(phone());
    let mut p = h.player(
        1,
        7,
        PlayerOptions {
            resume_after_interruption: true,
            ..PlayerOptions::default()
        },
    );
    h.loaded(&mut p, vec![entry("a")]);
    h.playing(&mut p);
    h.take();

    h.native(NativeEvent::PlayerPaused {
        id: 1,
        position_ms: Some(1_000),
    });
    assert_eq!(p.status(), PlayerStatus::Paused);
    h.native(NativeEvent::Interruption {
        phase: InterruptionPhase::Began,
        reason: InterruptionReason::OtherApp,
        should_resume: false,
    });
    assert_eq!(interruptions(&p.drain()).len(), 1);
    h.take();
    h.native(NativeEvent::Interruption {
        phase: InterruptionPhase::Ended,
        reason: InterruptionReason::OtherApp,
        should_resume: true,
    });
    let ended = interruptions(&p.drain());
    assert_eq!(ended.len(), 1);
    assert!(ended[0].resumed);
    assert!(h.take().contains(&Command::PlayerPlay { id: 1 }));
}

#[test]
fn a_command_after_a_native_pause_keeps_a_later_interruption_away() {
    let mut h = Harness::new(phone());
    let mut p = h.player(1, 7, PlayerOptions::default());
    h.loaded(&mut p, vec![entry("a")]);
    h.playing(&mut p);
    h.native(NativeEvent::PlayerPaused {
        id: 1,
        position_ms: Some(1_000),
    });
    h.command(&p, PlayerCommand::Pause);
    p.drain();
    h.native(NativeEvent::Interruption {
        phase: InterruptionPhase::Began,
        reason: InterruptionReason::OtherApp,
        should_resume: false,
    });
    assert!(interruptions(&p.drain()).is_empty());
}
