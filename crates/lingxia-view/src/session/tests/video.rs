use crate::ViewSession;
use crate::renderer::gpu::video::{Player, install};
use crate::scene::video::{Status, VideoError, VideoOp};

#[derive(Default)]
struct Fake {
    paused: bool,
}

impl Player for Fake {
    fn has_new(&self) -> bool {
        false
    }

    fn next_frame(
        &mut self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) -> Result<Option<(wgpu::Texture, [u16; 2])>, String> {
        Ok(None)
    }

    fn command(&mut self, op: &VideoOp) -> Result<(), VideoError> {
        match op {
            VideoOp::Play => self.paused = false,
            VideoOp::Pause => self.paused = true,
            _ => {}
        }
        Ok(())
    }

    fn status(&self) -> Status {
        Status {
            duration: Some(6.0),
            current_time: 1.5,
            paused: self.paused,
            ..Status::default()
        }
    }
}

#[test]
fn video_events_reach_a_page_that_committed_since_it_last_presented() {
    install(|_| Ok(Box::new(Fake::default())));
    let module = crate::session::tests::source(
        "function render(){const clip=useVideo('clip');return h('view',{},h('video',{player:clip,src:'clip.mp4',style:{width:160,height:90}}),h('text',{'data-testid':'duration'},clip.duration===null?'none':String(clip.duration)))}",
    );
    let mut page = ViewSession::new(&module).unwrap();
    page.layout(200, 200, 1.0).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut delivered = false;
    while !delivered && std::time::Instant::now() < deadline {
        // A page that commits every frame (a spinner) is always a commit past its last present.
        page.layout(200, 200, 1.0).unwrap();
        delivered = page.dispatch_video_events(false).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(delivered, "the player's status never reached the page");
    let frame = page.layout(200, 200, 1.0).unwrap();
    let text = &frame
        .nodes
        .iter()
        .find(|n| n.test_id == "duration")
        .unwrap()
        .text;
    assert_eq!(text, "6");
}

#[test]
fn a_hidden_page_holds_its_video_and_keeps_the_page_choice_of_play_or_pause() {
    use crate::renderer::gpu::video::{command, status};
    install(|_| Ok(Box::new(Fake::default())));
    let module = crate::session::tests::source(
        "function render(){const held=useVideo('held');return h('video',{player:held,src:'held.mp4',style:{width:160,height:90}})}",
    );
    let mut page = ViewSession::new(&module).unwrap();
    page.layout(200, 200, 1.0).unwrap();
    let scope = page.video_scope();
    let key = crate::scene::video::key(scope, "name:held");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while status(&key).and_then(|s| s.duration).is_none() {
        assert!(
            std::time::Instant::now() < deadline,
            "the player never opened"
        );
        page.layout(200, 200, 1.0).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let paused = || status(&key).unwrap().paused;
    // The hold is not the page's pause: the player still reads as playing.
    crate::renderer::video_hidden(scope, true);
    assert!(!paused());
    // A pause asked while hidden holds on show; a play asked while hidden plays on show.
    command(&key, VideoOp::Pause).unwrap();
    assert!(paused());
    crate::renderer::video_hidden(scope, false);
    assert!(paused());
    crate::renderer::video_hidden(scope, true);
    command(&key, VideoOp::Play).unwrap();
    assert!(!paused());
    crate::renderer::video_hidden(scope, false);
    assert!(!paused());
}
