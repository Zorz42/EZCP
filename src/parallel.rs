//! Running the pure parts of test generation on several threads.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Applies `f` to every item on up to `workers` threads and returns the results
/// in the order of the items.
///
/// A panic in `f` is returned in the place of its item rather than passed on,
/// so the caller decides whether that item mattered.
pub fn parallel_map<I: Sync, O: Send, F: Fn(&I) -> O + Sync>(items: &[I], workers: usize, f: F) -> Vec<std::thread::Result<O>> {
    let apply = |item| catch_unwind(AssertUnwindSafe(|| f(item)));
    let workers = workers.min(items.len());
    if workers <= 1 {
        return items.iter().map(apply).collect();
    }

    let next_item = AtomicUsize::new(0);
    let mut results: Vec<_> = items.iter().map(|_| None).collect();
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut done = Vec::new();
                    loop {
                        let idx = next_item.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(idx) else { break };
                        done.push((idx, apply(item)));
                    }
                    done
                })
            })
            .collect();
        for handle in handles {
            // `apply` catches panics, so a worker never ends with one.
            for (idx, result) in handle.join().unwrap_or_default() {
                results[idx] = Some(result);
            }
        }
    });
    // Every index was taken by exactly one worker.
    results.into_iter().flatten().collect()
}
