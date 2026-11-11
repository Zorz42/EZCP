use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicUsize, Ordering};

/// The results are in the order of the items. A panic is returned in its item's place, so the caller
/// decides whether that item mattered.
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
            // `apply` catches panics.
            for (idx, result) in handle.join().unwrap_or_default() {
                results[idx] = Some(result);
            }
        }
    });
    results.into_iter().flatten().collect()
}
