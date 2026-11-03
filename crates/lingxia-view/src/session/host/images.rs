//! Image requests: the presented frame's sources start loads, obsolete loads
//! are cancelled, and results bind on the worker before `onLoad`/`onError`.
use super::{Guard, Inner};
use crate::ImageSource;
use crate::platform::PlatformAdapter;
use std::sync::Arc;

impl<A: PlatformAdapter> Inner<A> {
    pub(super) fn update_images(
        self: &Arc<Self>,
        sources: Vec<ImageSource>,
        buckets: std::collections::HashMap<String, crate::scene::image::Bucket>,
    ) {
        let result = {
            let mut main = self.main();
            let before = main.images.inflight_tokens();
            main.images.update_sized(&sources, &buckets).map(|()| {
                before
                    .into_iter()
                    .filter(|token| !main.images.current(*token))
                    .collect::<Vec<_>>()
            })
        };
        match result {
            Ok(obsolete) => {
                for token in obsolete {
                    self.main().images.complete(token);
                    self.adapter.cancel(token);
                }
                self.pump_images();
            }
            Err(error) => self.fail(error),
        }
    }

    pub(super) fn pump_images(self: &Arc<Self>) {
        let requests = {
            let mut main = self.main();
            if main.disposed || !main.visible {
                return;
            }
            main.images.start()
        };
        for request in requests {
            self.adapter.fetch(request.token, &request.source);
        }
    }

    pub(super) fn image_loaded(self: &Arc<Self>, token: u64, bytes: Result<Vec<u8>, String>) {
        if token & super::fonts::FONT_TOKEN != 0 {
            return self.font_loaded(token, bytes);
        }
        let request = {
            let mut main = self.main();
            match main.images.request(token) {
                Some(request) if !main.disposed => Some(request.clone()),
                _ => {
                    main.images.complete(token);
                    None
                }
            }
        };
        let Some(request) = request else {
            self.pump_images();
            return;
        };
        match bytes {
            Ok(bytes) => {
                let inner = self.clone();
                let bucket = request.bucket;
                super::decode::decode(bytes, bucket, move |bitmap| {
                    let host = inner.clone();
                    inner
                        .adapter
                        .post(Box::new(move || host.decoded(token, request, bitmap)));
                });
            }
            Err(_) => self.decoded(token, request, Err(String::new())),
        }
    }

    /// Bind a decoded bitmap on the worker if its request is still current.
    fn decoded(
        self: &Arc<Self>,
        token: u64,
        request: super::super::image_requests::ImageRequest,
        bitmap: Result<crate::scene::image::Bitmap, String>,
    ) {
        if self.main().images.request(token).is_none() {
            self.pump_images();
            return;
        }
        self.work(
            Guard::Page(self.lifecycle.page()),
            move |inner, session| {
                let bound = match bitmap {
                    Ok(bitmap) => {
                        session
                            .bind_image(&request.source, bitmap)
                            .unwrap_or_else(|error| {
                                inner
                                    .adapter
                                    .warn(&format!("UI_IMAGE: {} failed: {error}", request.source));
                                false
                            })
                    }
                    Err(error) => {
                        if !error.is_empty() {
                            inner
                                .adapter
                                .warn(&format!("UI_IMAGE: {} failed: {error}", request.source));
                        }
                        false
                    }
                };
                session.image_completed(
                    ImageSource {
                        source: request.source,
                        retry_key: request.retry_key,
                    },
                    bound,
                );
                Ok(bound)
            },
            move |inner, result| {
                let live = inner.main().images.complete(token);
                if live {
                    if matches!(result, Some(Ok(true))) {
                        inner.schedule();
                    }
                    inner.flush_image_events();
                }
                inner.pump_images();
            },
        );
    }

    /// Deliver settled outcomes once the frame that mounted their consumers presented.
    pub(super) fn flush_image_events(self: &Arc<Self>) {
        let Some(generation) = ({
            let main = self.main();
            if main.disposed {
                None
            } else {
                main.snapshot.as_ref().map(|s| s.generation)
            }
        }) else {
            return;
        };
        let page = self.lifecycle.page();
        self.work(
            Guard::Page(page),
            move |inner, session| {
                if !session.dispatch_image_events(generation)? {
                    return Ok(None);
                }
                inner.drained(session).map(Some)
            },
            move |inner, result| match result {
                Some(Ok(Some(update))) => {
                    if let Some((connection, update)) = update {
                        inner.publish(connection, update);
                    }
                    if !inner.main().disposed && inner.lifecycle.is_current_page(page) {
                        inner.main().dirty = true;
                        inner.schedule();
                    }
                }
                Some(Err(error)) if inner.lifecycle.is_current_page(page) => inner.fail(error),
                _ => {}
            },
        );
    }
}
