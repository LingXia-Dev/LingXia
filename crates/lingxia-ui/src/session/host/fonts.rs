//! `@font-face` files: fetched by lxapp path once the module loads, then registered in the
//! page's collection. Text shows in the fallback until its face arrives (CSS `swap`).
use super::{Guard, Inner};
use crate::platform::PlatformAdapter;
use std::sync::Arc;

/// Font fetches share the adapter's image transport in their own token space.
pub(super) const FONT_TOKEN: u64 = 1 << 62;

impl<A: PlatformAdapter> Inner<A> {
    pub(super) fn load_fonts(self: &Arc<Self>, page: super::PageTicket) {
        self.work(
            Guard::Page(page),
            |_, session| session.declared_fonts(),
            move |inner, result| match result {
                Some(Ok(faces)) if inner.lifecycle.is_current_page(page) => {
                    let requests = {
                        let mut main = inner.main();
                        faces
                            .into_iter()
                            .map(|face| {
                                main.next_font += 1;
                                let token = FONT_TOKEN | main.next_font;
                                main.fonts.insert(token, face.clone());
                                (token, face.src)
                            })
                            .collect::<Vec<_>>()
                    };
                    for (token, src) in requests {
                        inner.adapter.fetch(token, &src);
                    }
                }
                Some(Err(error)) => inner.adapter.warn(&format!("UI_FONT: {error}")),
                _ => {}
            },
        );
    }

    pub(super) fn font_loaded(self: &Arc<Self>, token: u64, bytes: Result<Vec<u8>, String>) {
        let Some(face) = ({
            let mut main = self.main();
            if main.disposed {
                None
            } else {
                main.fonts.remove(&token)
            }
        }) else {
            return;
        };
        let bytes = match bytes {
            Ok(bytes) => bytes,
            Err(error) => {
                self.adapter
                    .warn(&format!("UI_FONT: {} failed to load: {error}", face.src));
                return;
            }
        };
        // A parked page keeps its module and so its faces; the current ticket reaches it.
        self.work(
            Guard::Page(self.lifecycle.page()),
            move |_, session| {
                session
                    .register_font(&face.family, bytes)
                    .map_err(|error| format!("UI_FONT: {} {error}", face.src))
            },
            |inner, result| match result {
                Some(Ok(())) => {
                    inner.main().dirty = true;
                    inner.schedule();
                }
                Some(Err(error)) => inner.adapter.warn(&error),
                None => {}
            },
        );
    }

    /// Fetches still in flight; a new module or disposal cancels them.
    pub(super) fn take_font_fetches(&self) -> Vec<u64> {
        self.main().fonts.drain().map(|(token, _)| token).collect()
    }
}
