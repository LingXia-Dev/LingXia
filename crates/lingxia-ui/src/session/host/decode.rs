//! Image decoding off the View worker, sized to the laid-out box, behind one
//! process-wide LRU. A large photo decodes for seconds in debug builds; on the
//! shared worker it would hold up every page's frames.
use crate::scene::image::{Bitmap, Bucket};
use std::collections::HashMap;
use std::hash::BuildHasher;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

type Job = Box<dyn FnOnce() + Send>;
type Done = Box<dyn FnOnce(Result<Bitmap, String>) + Send>;

/// At most this many decodes run at once, process-wide.
const THREADS: usize = 2;
/// Decoded pixels the cache retains across every page; pages hold what they show.
const BUDGET: usize = crate::protocol::limits::DECODED_IMAGE_BYTES;

static POOL: LazyLock<Mutex<Sender<Job>>> = LazyLock::new(|| {
    let (jobs, queue) = channel::<Job>();
    let queue: Arc<Mutex<Receiver<Job>>> = Arc::new(Mutex::new(queue));
    for index in 0..THREADS {
        let queue = queue.clone();
        let _ = std::thread::Builder::new()
            .name(format!("lingxia-ui-decode-{index}"))
            .spawn(move || {
                loop {
                    let job = queue.lock().unwrap_or_else(|p| p.into_inner()).recv();
                    match job {
                        Ok(job) => job(),
                        Err(_) => return,
                    }
                }
            });
    }
    Mutex::new(jobs)
});

static PENDING: AtomicUsize = AtomicUsize::new(0);
static CACHE: LazyLock<Mutex<Cache>> = LazyLock::new(|| Mutex::new(Cache::new(BUDGET)));
/// Keyed per process, so colliding encodings cannot be crafted ahead of time.
static HASH: LazyLock<std::collections::hash_map::RandomState> =
    LazyLock::new(std::collections::hash_map::RandomState::new);

/// The encoded bytes' identity: equal bytes from any page or app share one decode.
/// Process-local: never persisted or sent over the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Key {
    hash: u64,
    len: usize,
}

impl Key {
    pub(crate) fn of(bytes: &[u8]) -> Self {
        Self {
            hash: HASH.hash_one(bytes),
            len: bytes.len(),
        }
    }
}

struct Entry {
    bitmap: Bitmap,
    used: u64,
}

/// Decoded bitmaps by content and size, least recently used evicted first.
pub(crate) struct Cache {
    entries: HashMap<Key, Vec<Entry>>,
    inflight: HashMap<Key, Vec<(Bucket, Option<Done>)>>,
    bytes: usize,
    budget: usize,
    tick: u64,
    #[cfg(test)]
    decodes: HashMap<Key, usize>,
}

impl Cache {
    pub(crate) fn new(budget: usize) -> Self {
        Self {
            entries: HashMap::new(),
            inflight: HashMap::new(),
            bytes: 0,
            budget,
            tick: 0,
            #[cfg(test)]
            decodes: HashMap::new(),
        }
    }

    /// The smallest cached bitmap that serves `bucket`, now most recently used.
    pub(crate) fn get(&mut self, key: Key, bucket: Bucket) -> Option<Bitmap> {
        self.tick += 1;
        let entry = self
            .entries
            .get_mut(&key)?
            .iter_mut()
            .filter(|entry| entry.bitmap.serves(bucket))
            .min_by_key(|entry| entry.bitmap.byte_len())?;
        entry.used = self.tick;
        Some(entry.bitmap.clone())
    }

    pub(crate) fn insert(&mut self, key: Key, bitmap: Bitmap) {
        let len = bitmap.byte_len();
        if len > self.budget {
            return;
        }
        self.tick += 1;
        let entries = self.entries.entry(key).or_default();
        // The new pixels serve every bucket the content's older decodes served.
        let mut freed = 0;
        entries.retain(|entry| {
            let keep = !entry.bitmap.superseded_by(&bitmap);
            if !keep {
                freed += entry.bitmap.byte_len();
            }
            keep
        });
        self.bytes -= freed;
        entries.push(Entry {
            bitmap,
            used: self.tick,
        });
        self.bytes += len;
        while self.bytes > self.budget {
            let Some((key, index)) = self
                .entries
                .iter()
                .flat_map(|(key, entries)| {
                    entries
                        .iter()
                        .enumerate()
                        .map(move |(index, entry)| (entry.used, *key, index))
                })
                .min_by_key(|(used, ..)| *used)
                .map(|(_, key, index)| (key, index))
            else {
                break;
            };
            let entries = self.entries.get_mut(&key).expect("evicted key");
            self.bytes -= entries.swap_remove(index).bitmap.byte_len();
            if entries.is_empty() {
                self.entries.remove(&key);
            }
        }
    }

    pub(crate) fn bytes(&self) -> usize {
        self.bytes
    }
}

fn cache() -> MutexGuard<'static, Cache> {
    CACHE.lock().unwrap_or_else(|p| p.into_inner())
}

/// Decode `bytes` for a box in `bucket` on the pool, or reuse pixels already decoded
/// for that content. `done` runs once, on the pool or on the caller's thread.
pub(super) fn decode(
    bytes: Vec<u8>,
    bucket: Bucket,
    done: impl FnOnce(Result<Bitmap, String>) + Send + 'static,
) {
    let key = Key::of(&bytes);
    let done: Done = Box::new(done);
    let start = {
        let mut cache = cache();
        if let Some(bitmap) = cache.get(key, bucket) {
            drop(cache);
            done(Ok(bitmap));
            return;
        }
        let waiters = cache.inflight.entry(key).or_default();
        // A running decode at this size or larger serves this request as well.
        let start = !waiters.iter().any(|(size, _)| size.covers(bucket));
        waiters.push((bucket, Some(done)));
        start
    };
    if !start {
        return;
    }
    PENDING.fetch_add(1, Ordering::AcqRel);
    let job: Job = Box::new(move || {
        let result = Bitmap::decode_to(&bytes, bucket);
        let ready = {
            let mut cache = cache();
            #[cfg(test)]
            {
                *cache.decodes.entry(key).or_default() += 1;
            }
            if let Ok(bitmap) = &result {
                cache.insert(key, bitmap.clone());
            }
            let waiters = cache.inflight.entry(key).or_default();
            let mut ready = Vec::new();
            // Bad bytes fail every size; waiters for a larger decode keep waiting for it.
            for (size, done) in waiters.iter_mut() {
                if (result.is_err() || result.as_ref().is_ok_and(|b| b.serves(*size)))
                    && let Some(done) = done.take()
                {
                    ready.push(done);
                }
            }
            waiters.retain(|(_, done)| done.is_some());
            if waiters.is_empty() {
                cache.inflight.remove(&key);
            }
            ready
        };
        for done in ready {
            done(result.clone());
        }
        PENDING.fetch_sub(1, Ordering::AcqRel);
    });
    let _ = POOL.lock().unwrap_or_else(|p| p.into_inner()).send(job);
}

/// Decoded bytes the process cache retains.
pub fn retained_bytes() -> usize {
    cache().bytes()
}

/// Decoded bytes and bitmaps the process cache retains.
pub(super) fn retained() -> (usize, usize) {
    let cache = cache();
    (cache.bytes(), cache.entries.values().map(Vec::len).sum())
}

/// How many times `bytes` were decoded in this process.
#[cfg(test)]
pub(crate) fn decodes_of(bytes: &[u8]) -> usize {
    cache().decodes.get(&Key::of(bytes)).copied().unwrap_or(0)
}

/// Tests settle only after every queued decode has handed its result on.
#[cfg(test)]
pub(crate) fn wait_idle() {
    while PENDING.load(Ordering::Acquire) != 0 {
        std::thread::yield_now();
    }
}

/// Occupies every decode thread until `release` fires or drops.
#[cfg(test)]
pub(crate) fn occupy(release: std::sync::mpsc::Receiver<()>) {
    let release = Arc::new(Mutex::new(release));
    for _ in 0..THREADS {
        let release = release.clone();
        PENDING.fetch_add(1, Ordering::AcqRel);
        let job: Job = Box::new(move || {
            let _ = release.lock().unwrap_or_else(|p| p.into_inner()).recv();
            PENDING.fetch_sub(1, Ordering::AcqRel);
        });
        let _ = POOL.lock().unwrap_or_else(|p| p.into_inner()).send(job);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vello_cpu::Pixmap;

    fn bitmap(side: u16) -> Bitmap {
        Bitmap::from_pixels(Pixmap::new(side, side))
    }

    fn key(n: u8) -> Key {
        Key::of(&[n])
    }

    const B64: Bucket = Bucket {
        width: 64,
        height: 64,
    };

    #[test]
    fn the_least_recently_used_bitmap_is_evicted_first_within_one_budget() {
        // 16x16 = 1 KiB each; room for three.
        let mut cache = Cache::new(3 * 1024);
        cache.insert(key(1), bitmap(16));
        cache.insert(key(2), bitmap(16));
        cache.insert(key(3), bitmap(16));
        assert!(cache.get(key(1), B64).is_some());
        cache.insert(key(4), bitmap(16));
        assert!(cache.get(key(2), B64).is_none(), "oldest unused entry goes");
        for kept in [1, 3, 4] {
            assert!(cache.get(key(kept), B64).is_some());
        }
        assert_eq!(cache.bytes(), 3 * 1024);
        // Larger than the whole budget: served, never retained.
        cache.insert(key(5), bitmap(64));
        assert!(cache.get(key(5), B64).is_none());
        assert_eq!(cache.bytes(), 3 * 1024);
    }

    #[test]
    fn a_larger_decode_replaces_the_smaller_ones_it_serves() {
        let mut cache = Cache::new(1 << 20);
        let small = Bitmap::decode_to(&png(512, 256), B64).unwrap();
        let large = Bitmap::decode_to(&png(512, 256), Bucket::of(256., 256.)).unwrap();
        assert!(small.byte_len() < large.byte_len());
        cache.insert(key(1), small);
        assert!(
            cache.get(key(1), Bucket::of(256., 256.)).is_none(),
            "a small decode never upsamples"
        );
        cache.insert(key(1), large.clone());
        assert_eq!(cache.get(key(1), B64), Some(large));
        assert_eq!(cache.bytes(), 512 * 256 * 4);
    }

    #[test]
    fn decodes_cover_their_box_under_any_fit_and_never_exceed_the_source() {
        // A 2:1 source in a 64x64 box: cover needs 128x64.
        let wide = Bitmap::decode_to(&png(512, 256), B64).unwrap();
        assert_eq!((wide.pixels().width(), wide.pixels().height()), (128, 64));
        assert!(wide.serves(B64) && !wide.serves(Bucket::of(181., 64.)));
        // A wide box needs only its width from a wide source.
        let banner = Bitmap::decode_to(&png(512, 256), Bucket::of(256., 64.)).unwrap();
        assert_eq!(banner.pixels().width(), 256);
        let full = Bitmap::decode_to(&png(512, 256), Bucket::of(1024., 1024.)).unwrap();
        assert_eq!((full.pixels().width(), full.pixels().height()), (512, 256));
        assert!(full.serves(Bucket::FULL));
        assert_eq!(
            Bucket::of(0., 65.),
            Bucket {
                width: 64,
                height: 91
            }
        );
        assert_eq!(
            Bucket::of(1080., 9000.),
            Bucket {
                width: 1448,
                height: 4096
            }
        );
    }

    pub(crate) fn png(width: u32, height: u32) -> Vec<u8> {
        use ::image::ImageEncoder;
        let mut bytes = Vec::new();
        ::image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(
                &vec![200; (width * height * 4) as usize],
                width,
                height,
                ::image::ExtendedColorType::Rgba8,
            )
            .unwrap();
        bytes
    }
}
