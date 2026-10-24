//! Experimental UI language core. No page-stack or host authority dependencies.
pub mod animation;
pub mod engine;
pub mod interaction;
pub mod layout;
pub mod platform;
pub mod protocol;
pub mod renderer;
pub mod scene;
pub(crate) mod semantics;
pub mod session;
pub mod text;

pub use engine::evaluate;
pub use interaction::input_state::InputReply;
pub use layout::Renderer;
pub use scene::display::DisplayList;
pub use scene::node::Frame;
pub use scene::plot::{PieLayout, PlotSelection, PlotSnapshot};
pub use scene::slider::SliderSnapshot;
pub use session::ViewSession;
pub use session::app::{ViewApp, ViewPageId};
pub use session::image_requests::{ImageRequest, ImageRequests, ImageSource};
pub use session::metrics::FrameMetrics;
