use std::cell::RefCell;

type FrameCallback = Box<dyn Fn() + Send + 'static>;

#[derive(Default)]
pub(crate) struct FrameCallbacks(RefCell<Vec<FrameCallback>>);

impl FrameCallbacks {
    /// Returns whether the host needs to request a new frame.
    pub(crate) fn observe(&self, callback: FrameCallback) -> bool {
        let mut pending = self.0.borrow_mut();
        let first = pending.is_empty();
        pending.push(callback);
        first
    }

    pub(crate) fn notify(&self) {
        // A callback may register work for the next frame. Do not hold the
        // borrow or run that new work during this frame.
        let callbacks = std::mem::take(&mut *self.0.borrow_mut());
        for callback in callbacks {
            callback();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn one_frame_notifies_every_observer_once() {
        let callbacks = FrameCallbacks::default();
        let seen = Arc::new(Mutex::new(Vec::new()));
        for index in 0..3 {
            let seen = seen.clone();
            assert_eq!(
                callbacks.observe(Box::new(move || seen.lock().unwrap().push(index))),
                index == 0
            );
        }
        callbacks.notify();
        callbacks.notify();
        assert_eq!(*seen.lock().unwrap(), vec![0, 1, 2]);
        assert!(callbacks.observe(Box::new(|| {})));
    }

    #[test]
    fn next_frame_can_be_registered_from_a_callback() {
        thread_local! {
            static CALLBACKS: FrameCallbacks = FrameCallbacks::default();
        }
        let seen = Arc::new(Mutex::new(Vec::new()));
        let callback_seen = seen.clone();
        CALLBACKS.with(|callbacks| {
            callbacks.observe(Box::new(move || {
                callback_seen.lock().unwrap().push(1);
                let seen = callback_seen.clone();
                CALLBACKS.with(|callbacks| {
                    assert!(callbacks.observe(Box::new(move || seen.lock().unwrap().push(2))));
                });
            }));
            callbacks.notify();
            assert_eq!(*seen.lock().unwrap(), vec![1]);
            callbacks.notify();
            assert_eq!(*seen.lock().unwrap(), vec![1, 2]);
        });
    }
}
