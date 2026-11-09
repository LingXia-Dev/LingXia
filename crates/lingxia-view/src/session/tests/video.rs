use crate::ViewSession;
use crate::renderer::gpu::video::{Player, install};
use crate::scene::video::Status;

struct Fake;

impl Player for Fake {
    fn has_new(&self) -> bool {
        false
    }

    fn next_frame(
        &mut self,
        _device: &wgpu::Device,
    ) -> Result<Option<(wgpu::Texture, [u16; 2])>, String> {
        Ok(None)
    }

    fn status(&self) -> Status {
        Status {
            duration: Some(6.0),
            current_time: 1.5,
            paused: false,
            ..Status::default()
        }
    }
}

#[test]
fn video_events_reach_a_page_that_committed_since_it_last_presented() {
    install(|_| Ok(Box::new(Fake)));
    let module = crate::session::tests::source(
        "function render(){const clip=useVideo('clip');return h('column',{},h('video',{player:clip,src:'clip.mp4',style:{width:160,height:90}}),h('text',{'data-testid':'duration'},clip.duration===null?'none':String(clip.duration)))}",
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
