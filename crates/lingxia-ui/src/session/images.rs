use super::ViewSession;
use crate::ImageSource;
use crate::protocol::{field, tag};
use std::collections::HashSet;

impl ViewSession {
    pub fn image_requests(&self) -> Vec<ImageSource> {
        let mut seen = HashSet::new();
        self.input_records
            .as_deref()
            .unwrap_or_default()
            .iter()
            .filter(|record| record.tag == tag::IMAGE)
            .flat_map(|record| {
                let image = record.string(field::IMAGE_SRC).map(|source| ImageSource {
                    source: source.into(),
                    retry_key: record.string(field::IMAGE_RETRY_KEY).unwrap_or("").into(),
                });
                // The placeholder loads alongside; it never reports load events of its own.
                let placeholder =
                    record
                        .string(field::IMAGE_PLACEHOLDER)
                        .map(|source| ImageSource {
                            source: source.into(),
                            retry_key: String::new(),
                        });
                [placeholder, image].into_iter().flatten()
            })
            .filter(|resource| {
                self.image_sources.contains(&resource.source) && seen.insert(resource.clone())
            })
            .collect()
    }

    pub(super) fn retain_image_lifecycle(&mut self) {
        let wanted: HashSet<_> = self.image_requests().into_iter().collect();
        self.image_status.retain(|key, _| wanted.contains(key));
        let records = self.input_records.as_deref().unwrap_or_default();
        let identities = crate::protocol::identities(records);
        let alive: HashSet<_> = records
            .iter()
            .zip(identities)
            .filter(|(r, _)| r.tag == tag::IMAGE)
            .map(|(record, identity)| {
                (
                    identity,
                    ImageSource {
                        source: record.string(field::IMAGE_SRC).unwrap_or_default().into(),
                        retry_key: record.string(field::IMAGE_RETRY_KEY).unwrap_or("").into(),
                    },
                )
            })
            .collect();
        self.image_notified.retain(|key| alive.contains(key));
    }

    /// Record only the result of a still-wanted source/retry pair.
    pub fn image_completed(&mut self, resource: ImageSource, success: bool) {
        if self.image_requests().contains(&resource) {
            self.image_status.insert(resource, success);
        }
    }

    /// Called after presentation; newly mounted consumers also receive cached outcomes.
    pub fn dispatch_image_events(
        &mut self,
        generation: crate::session::lifecycle::CommitTicket,
    ) -> Result<bool, String> {
        if generation != self.generation {
            return Ok(false);
        }
        let records = self.input_records.as_deref().unwrap_or_default();
        let identities = crate::protocol::identities(records);
        let mut handles = Vec::new();
        for (record, identity) in records
            .iter()
            .zip(identities)
            .filter(|(r, _)| r.tag == tag::IMAGE)
        {
            let resource = ImageSource {
                source: record.string(field::IMAGE_SRC).unwrap_or_default().into(),
                retry_key: record.string(field::IMAGE_RETRY_KEY).unwrap_or("").into(),
            };
            let Some(success) = self.image_status.get(&resource) else {
                continue;
            };
            if self
                .image_notified
                .contains(&(identity.clone(), resource.clone()))
            {
                continue;
            }
            if let Some(handle) = record
                .fields
                .iter()
                .find(|(id, _)| {
                    *id == if *success {
                        field::ON_IMAGE_LOAD
                    } else {
                        field::ON_IMAGE_ERROR
                    }
                })
                .and_then(|(_, value)| value.as_u64())
            {
                handles.push((identity, resource, handle));
            }
        }
        let mut changed = false;
        for (identity, resource, handle) in handles {
            self.image_notified.insert((identity, resource));
            changed |= self.click(generation, handle)?;
        }
        Ok(changed)
    }

    pub fn image_sources(&self) -> Vec<String> {
        self.image_sources.iter().cloned().collect()
    }
    pub fn load_image(&mut self, src: &str, bytes: &[u8]) -> Result<bool, String> {
        if !self.image_sources.contains(src) {
            return Ok(false);
        }
        let bitmap = crate::scene::image::Bitmap::decode(bytes)?;
        self.bind_image(src, bitmap)
    }
    pub(crate) fn bind_image(
        &mut self,
        src: &str,
        bitmap: crate::scene::image::Bitmap,
    ) -> Result<bool, String> {
        if !self.image_sources.contains(src) {
            return Ok(false);
        }
        // Only painted sources are bound, each decoded to its box. Past the visible budget
        // the source fails (`onError`, placeholder); the process cache only evicts.
        let retained: usize = self
            .images
            .iter()
            .filter(|(key, _)| key.as_str() != src)
            .map(|(_, image)| image.byte_len())
            .sum();
        if retained + bitmap.byte_len() > crate::protocol::limits::VISIBLE_IMAGE_BYTES {
            return Err("UI_IMAGE: visible image budget exceeded".into());
        }
        self.images.insert(src.to_owned(), bitmap);
        Ok(true)
    }
    pub(super) fn commit_images(&mut self, wanted: std::collections::HashSet<String>) {
        self.images.retain(|src, _| wanted.contains(src));
        self.image_sources = wanted;
        self.retain_image_lifecycle();
    }
}
