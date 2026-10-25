use super::*;
use std::sync::{Arc, Mutex};

/// Shared between "processes" in a test, like a file on disk.
#[derive(Clone, Default)]
struct Disk {
    record: Arc<Mutex<Option<String>>>,
    fail_writes: bool,
}

impl SentinelStore for Disk {
    fn read(&self) -> Option<String> {
        self.record.lock().unwrap().clone()
    }
    fn write(&self, record: &str) -> Result<(), String> {
        if self.fail_writes {
            return Err("read-only".into());
        }
        *self.record.lock().unwrap() = Some(record.into());
        Ok(())
    }
    fn clear(&self) {
        *self.record.lock().unwrap() = None;
    }
}

const VULKAN: &str = "Vulkan|13b5:92020010|Mali-G78|v1.r32p1";
const MALEOON: &str =
    "Vulkan|19e5:10001000|Maleoon 910|Maleoon 910|V100R001C10B221|lingxia-ui 0.28.0";
const GLES: &str = "Gl|13b5:0000|Mali-G78|OpenGL ES 3.2";
const ADRENO: &str = "Vulkan|5143:6040001|Adreno 640|512.502";

#[test]
fn a_presented_frame_clears_probation_and_later_launches_use_the_gpu() {
    let disk = Disk::default();
    let first = Sentinel::new(Box::new(disk.clone()));
    assert!(first.arm(ADRENO));
    assert!(
        first.arm(ADRENO),
        "a second page on the same adapter shares probation"
    );
    first.presented(ADRENO);
    assert_eq!(disk.read(), None);
    let next = Sentinel::new(Box::new(disk.clone()));
    assert!(next.arm(ADRENO));
    assert_eq!(next.fallback(), None);
}

#[test]
fn a_crash_on_probation_blocks_that_adapter_and_its_fallback_keeps_the_block() {
    let disk = Disk::default();
    let crashed = Sentinel::new(Box::new(disk.clone()));
    assert!(crashed.arm(VULKAN));
    drop(crashed);
    let relaunch = Sentinel::new(Box::new(disk.clone()));
    assert_eq!(relaunch.fallback(), Some(Fallback::Driver));
    assert!(!relaunch.arm(VULKAN));
    assert!(
        relaunch.arm(GLES),
        "the fallback backend gets its own probation"
    );
    relaunch.presented(GLES);
    let later = Sentinel::new(Box::new(disk.clone()));
    assert!(
        !later.arm(VULKAN),
        "a proven fallback never unblocks the crashed adapter"
    );
    assert!(later.arm(GLES));
    assert_eq!(later.fallback(), Some(Fallback::Driver));
    later.presented(GLES);
    let updated = Sentinel::new(Box::new(disk.clone()));
    assert!(
        updated.arm("Vulkan|13b5:92020010|Mali-G78|v1.r33p0"),
        "an updated driver is a different record and is tried again"
    );
}

#[test]
fn a_crashing_fallback_is_blocked_too() {
    let disk = Disk::default();
    let first = Sentinel::new(Box::new(disk.clone()));
    assert!(first.arm(VULKAN));
    drop(first);
    let second = Sentinel::new(Box::new(disk.clone()));
    assert!(second.arm(GLES));
    drop(second);
    let third = Sentinel::new(Box::new(disk.clone()));
    assert!(!third.arm(VULKAN));
    assert!(
        !third.arm(GLES),
        "every crashed backend stays blocked; pages render on the CPU"
    );
}

#[test]
fn failed_attach_releases_probation_without_blocking() {
    let disk = Disk::default();
    let sentinel = Sentinel::new(Box::new(disk.clone()));
    assert!(sentinel.arm(ADRENO));
    sentinel.released(ADRENO);
    assert_eq!(disk.read(), None);
    assert!(sentinel.arm(ADRENO));
    assert_eq!(sentinel.fallback(), None);
}

#[test]
fn an_unwritable_store_never_risks_a_crash_loop() {
    let disk = Disk {
        fail_writes: true,
        ..Default::default()
    };
    let sentinel = Sentinel::new(Box::new(disk));
    assert!(!sentinel.arm(ADRENO));
}

#[test]
fn file_store_round_trips_a_record() {
    let path = std::env::temp_dir().join(format!("lingxia-sentinel-{}", std::process::id()));
    let store = FileStore(path.clone());
    assert_eq!(store.read(), None);
    store.write(VULKAN).unwrap();
    assert_eq!(store.read().as_deref(), Some(VULKAN));
    store.clear();
    assert!(!path.exists());
}

#[test]
fn known_crashing_drivers_never_get_a_first_crash() {
    let disk = Disk::default();
    let sentinel = Sentinel::new(Box::new(disk.clone()));
    assert!(!sentinel.arm(MALEOON));
    assert_eq!(disk.read(), None, "nothing was put on probation");
    assert_eq!(sentinel.fallback(), Some(Fallback::Driver));
    assert!(sentinel.arm("Gl|19e5:0000|Maleoon 910|OpenGL ES 3.2"));
}
