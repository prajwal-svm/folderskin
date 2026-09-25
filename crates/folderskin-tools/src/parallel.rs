//! Work on every core at once: libwebp's smallest lossless file takes most of a second a picture,
//! and a pack has up to fifty.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// `f` of every one of `items`, on every core at once, in the order of `items`.
pub(crate) fn map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let done: Mutex<Vec<(usize, R)>> = Mutex::new(Vec::with_capacity(items.len()));
    let workers = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(items.len().max(1));
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(item) = items.get(i) else { break };
                let out = f(item);
                done.lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push((i, out));
            });
        }
    });
    let mut done = done
        .into_inner()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    done.sort_by_key(|(i, _)| *i);
    done.into_iter().map(|(_, out)| out).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_item_comes_back_in_its_place() {
        let items: Vec<u32> = (0..100).collect();
        assert_eq!(
            super::map(&items, |n| n * 2),
            (0..200).step_by(2).collect::<Vec<_>>()
        );
        assert!(super::map(&[] as &[u32], |n| *n).is_empty());
    }
}
