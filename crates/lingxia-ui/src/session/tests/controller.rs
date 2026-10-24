//! The shared core driven through a fake platform adapter.
use crate::DisplayList;
use crate::interaction::frame::PresentedFrame;
use crate::interaction::view::{AccessibilityEffect, Dispatch, Effect, Pointer, PointerPhase};
use crate::platform::*;
use crate::semantics::projection::{Projection, Role};
use crate::session::controller::ViewController;
use crate::session::lifecycle::{FailureKind, Retry, SurfaceTicket};
use crate::session::tests::source;

#[derive(Default)]
struct Fake {
    presented: Vec<SurfaceTicket>,
    texts: Vec<String>,
    fetched: Vec<(u64, String)>,
    cancelled: Vec<u64>,
    roles: Vec<Vec<Role>>,
    events: Vec<AccessibilityEffect>,
    effects: Vec<Effect>,
    editors: Vec<Dispatch>,
}

impl Presenter for Fake {
    fn present(&mut self, display: DisplayList, surface: SurfaceTicket) -> Result<bool, String> {
        self.presented.push(surface);
        self.texts = display
            .nodes
            .iter()
            .map(|node| node.text.clone())
            .filter(|t| !t.is_empty())
            .collect();
        Ok(true)
    }
}
impl ImageProvider for Fake {
    fn fetch(&mut self, id: u64, source: &str) {
        self.fetched.push((id, source.into()));
    }
    fn cancel(&mut self, id: u64) {
        self.cancelled.push(id);
    }
}
impl AccessibilitySink for Fake {
    fn publish(&mut self, projection: &Projection<'_>, events: &[AccessibilityEffect]) {
        self.roles.push(
            projection
                .roots()
                .iter()
                .filter_map(|id| projection.node(*id))
                .map(|node| node.role)
                .collect(),
        );
        self.events.extend_from_slice(events);
    }
}
impl InputMethod for Fake {
    fn presented(&mut self, _: &PresentedFrame) {}
    fn focus(&mut self, editor: &Dispatch) {
        self.editors.push(editor.clone());
    }
}
impl Feedback for Fake {
    fn apply(&mut self, effects: &[Effect]) {
        self.effects.extend_from_slice(effects);
    }
}
impl Clock for Fake {
    fn now(&self) -> std::time::Duration {
        std::time::Duration::ZERO
    }
}

fn tap(controller: &mut ViewController<Fake>, x: f32, y: f32) {
    for phase in [PointerPhase::Down, PointerPhase::Up] {
        controller
            .pointer(Pointer {
                phase,
                x,
                y,
                slop: 8.0,
                velocity: [0.0; 2],
                min_fling: 50.0,
            })
            .unwrap();
    }
}

#[test]
fn a_fake_platform_hosts_frames_input_semantics_and_images() {
    let page = source(
        "function render(){const n=useLocal('n',0);return h('column',{},h('button',{testId:'add',onClick:()=>n.set(n.value+1),style:{height:60}},`Count ${n.value}`),n.value<2?h('image',{src:'lx://pic.png',alt:'Picture',style:{width:20,height:20}}):h('text',{},'done'))}",
    );
    let mut controller = ViewController::new(Fake::default(), &page, (200, 400, 2.0)).unwrap();
    assert!(!controller.frame().unwrap(), "hidden pages do not present");
    controller.set_visible(true);
    assert!(controller.frame().unwrap());
    assert_eq!(controller.adapter.texts, ["Count 0"]);
    assert_eq!(
        controller.adapter.roles.last().unwrap(),
        &[Role::Button, Role::Image]
    );
    let (image, source) = controller.adapter.fetched[0].clone();
    assert_eq!(source, "lx://pic.png");
    tap(&mut controller, 40.0, 40.0);
    assert!(controller.adapter.effects.contains(&Effect::Clicked));
    controller.frame().unwrap();
    assert_eq!(controller.adapter.texts, ["Count 1"]);
    let png = {
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(&[255, 0, 0, 255], 1, 1, image::ExtendedColorType::Rgba8)
            .unwrap();
        bytes
    };
    use image::ImageEncoder;
    assert!(controller.image_loaded(image, Ok(png.clone())).unwrap());
    assert!(
        !controller.image_loaded(image, Ok(png)).unwrap(),
        "a settled request cannot bind twice"
    );
    tap(&mut controller, 40.0, 40.0);
    controller.frame().unwrap();
    assert_eq!(controller.adapter.texts, ["Count 2", "done"]);
    assert!(
        controller
            .adapter
            .events
            .iter()
            .any(|e| matches!(e, AccessibilityEffect::Change(_)))
    );
}

#[test]
fn removed_images_cancel_their_transport_and_failures_stop_input() {
    let page = source(
        "function render(){const show=useLocal('show',true);return h('column',{},h('button',{onClick:()=>show.set(false),style:{height:60}},'hide'),show.value?h('image',{src:'lx://a.png',style:{width:20,height:20}}):h('text',{},'gone'))}",
    );
    let mut controller = ViewController::new(Fake::default(), &page, (200, 400, 1.0)).unwrap();
    controller.set_visible(true);
    controller.frame().unwrap();
    let (image, _) = controller.adapter.fetched[0].clone();
    tap(&mut controller, 20.0, 20.0);
    controller.frame().unwrap();
    assert_eq!(controller.adapter.cancelled, [image]);
    assert!(!controller.image_loaded(image, Err("late".into())).unwrap());
    let recovery = controller.fail("UI_TIMEOUT: View JavaScript exceeded its execution budget");
    assert_eq!(
        (recovery.kind, recovery.retry),
        (FailureKind::Session, Retry::Reload)
    );
    assert!(!controller.frame().unwrap());
    assert!(
        !controller
            .pointer(Pointer {
                phase: PointerPhase::Down,
                x: 20.0,
                y: 20.0,
                slop: 8.0,
                velocity: [0.0; 2],
                min_fling: 0.0
            })
            .unwrap()
    );
}
