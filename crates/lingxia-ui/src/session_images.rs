use super::ViewSession;
use crate::ImageSource;
use std::collections::HashSet;

impl ViewSession {
    pub fn image_requests(&self) -> Vec<ImageSource> {
        let mut seen = HashSet::new();
        self.input_records
            .as_deref()
            .unwrap_or_default()
            .iter()
            .filter(|record| record.tag == 7)
            .filter_map(|record| {
                let resource = ImageSource {
                    source: record.string(19)?.into(),
                    retry_key: record.string(106).unwrap_or("").into(),
                };
                (self.image_sources.contains(&resource.source) && seen.insert(resource.clone()))
                    .then_some(resource)
            })
            .collect()
    }

    pub(super) fn retain_image_lifecycle(&mut self) {
        let wanted: HashSet<_> = self.image_requests().into_iter().collect();
        self.image_status.retain(|key, _| wanted.contains(key));
        let records = self.input_records.as_deref().unwrap_or_default();
        let identities = crate::schema::identities(records);
        let alive: HashSet<_> = records
            .iter()
            .zip(identities)
            .filter(|(r, _)| r.tag == 7)
            .map(|(record, identity)| {
                (
                    identity,
                    ImageSource {
                        source: record.string(19).unwrap_or_default().into(),
                        retry_key: record.string(106).unwrap_or("").into(),
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
    pub fn dispatch_image_events(&mut self, generation: u64) -> Result<bool, String> {
        if generation != self.generation {
            return Ok(false);
        }
        let records = self.input_records.as_deref().unwrap_or_default();
        let identities = crate::schema::identities(records);
        let mut handles = Vec::new();
        for (record, identity) in records.iter().zip(identities).filter(|(r, _)| r.tag == 7) {
            let resource = ImageSource {
                source: record.string(19).unwrap_or_default().into(),
                retry_key: record.string(106).unwrap_or("").into(),
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
                .find(|(id, _)| *id == if *success { 107 } else { 108 })
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
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_callback_errors_do_not_consume_other_consumers_or_their_updates() {
        let module = crate::session::tests::source(
            "function render(){const status=useLocal('status','initial');return h('column',{},h('image',{src:'photo',onLoad:()=>{status.set('bad');throw Error('first')},style:{width:20,height:20}}),h('image',{src:'photo',onLoad:()=>status.set('ready'),style:{width:20,height:20}}),h('image',{src:'photo',onLoad:()=>{status.set('bad');throw Error('last')},style:{width:20,height:20}}),h('text',{testId:'status'},status.value))}",
        );
        let mut page = ViewSession::new(&module).unwrap();
        page.layout(200, 200, 1.0).unwrap();
        page.image_completed(page.image_requests()[0].clone(), true);
        assert!(page.dispatch_image_events(page.generation()).unwrap());
        assert_eq!(page.take_action_errors().unwrap().len(), 2);
        let frame = page.layout(200, 200, 1.0).unwrap();
        assert_eq!(
            frame
                .nodes
                .iter()
                .find(|n| n.test_id == "status")
                .unwrap()
                .text,
            "ready"
        );
        assert!(!page.dispatch_image_events(page.generation()).unwrap());
    }

    #[test]
    fn regenerated_image_callbacks_reuse_layout() {
        let module = crate::session::tests::source(
            "function render(){return h('image',{src:'photo',onLoad(){},onError(){},style:{width:20,height:20}})}",
        );
        let mut page = ViewSession::new(&module).unwrap();
        page.layout(200, 200, 1.0).unwrap();
        page.layout(200, 200, 1.0).unwrap();
        assert_eq!(page.frame_metrics().layout_passes, 0);
    }

    #[test]
    fn image_outcomes_follow_committed_identity_and_retry_without_idle_loops() {
        let module = crate::session::tests::source(
            "globalThis.retry='';globalThis.extra=false;function render(){const status=useLocal('status','loading');return h('column',{},h('image',{key:'photo',src:'photo',retryKey:globalThis.retry,onLoad:()=>status.set('ready'),onError:()=>status.set('failed'),style:{width:20,height:20}}),globalThis.extra?h('image',{key:'extra',src:'photo',retryKey:globalThis.retry,onLoad:()=>status.set('cached'),style:{width:20,height:20}}):null,h('text',{testId:'status'},status.value))}",
        );
        let mut page = ViewSession::new(&module).unwrap();
        page.layout(200, 200, 1.0).unwrap();
        let original = page.image_requests()[0].clone();
        page.image_completed(original.clone(), false);
        assert!(!page.dispatch_image_events(page.generation() - 1).unwrap());
        assert!(page.dispatch_image_events(page.generation()).unwrap());
        let frame = page.layout(200, 200, 1.0).unwrap();
        assert_eq!(
            frame
                .nodes
                .iter()
                .find(|n| n.test_id == "status")
                .unwrap()
                .text,
            "failed"
        );
        assert!(!page.dispatch_image_events(page.generation()).unwrap());
        page.eval("globalThis.retry='1'").unwrap();
        page.layout(200, 200, 1.0).unwrap();
        page.image_completed(original, true);
        assert!(!page.dispatch_image_events(page.generation()).unwrap());
        page.image_completed(page.image_requests()[0].clone(), true);
        assert!(page.dispatch_image_events(page.generation()).unwrap());
        page.layout(200, 200, 1.0).unwrap();
        page.eval("globalThis.extra=true").unwrap();
        page.layout(200, 200, 1.0).unwrap();
        assert!(page.dispatch_image_events(page.generation()).unwrap());
        let frame = page.layout(200, 200, 1.0).unwrap();
        assert_eq!(
            frame
                .nodes
                .iter()
                .find(|n| n.test_id == "status")
                .unwrap()
                .text,
            "cached"
        );
        assert!(!page.dispatch_image_events(page.generation()).unwrap());
    }
}
