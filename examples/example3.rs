//! A task on trees: the graph generators, a validator, limits, and partial
//! solutions that fail in different ways.
//!
//! The slow partial solution spends the whole time limit on every large test.
//! Run with `--fail-fast` to have it stopped at the first one of each subtask.

use ezcp::{Graph, Result, Rng, Subtask, Task};
use std::path::PathBuf;

/// Reads a tree and can find the node farthest from another one.
const COMMON: &str = "
#include <cstdio>
#include <queue>
#include <utility>
#include <vector>
using namespace std;

int n;
vector<vector<int>> adjacent;

void read_tree() {
    scanf(\"%d\", &n);
    adjacent.resize(n + 1);
    for (int i = 1; i < n; i++) {
        int u, v;
        scanf(\"%d %d\", &u, &v);
        adjacent[u].push_back(v);
        adjacent[v].push_back(u);
    }
}

// The node farthest from `from`, and how far away it is.
pair<int, int> farthest(int from) {
    vector<int> distance(n + 1, -1);
    queue<int> reached;
    reached.push(from);
    distance[from] = 0;
    int last = from;
    while (!reached.empty()) {
        last = reached.front();
        reached.pop();
        for (int next : adjacent[last]) {
            if (distance[next] < 0) {
                distance[next] = distance[last] + 1;
                reached.push(next);
            }
        }
    }
    return {last, distance[last]};
}
";

fn program(answer: &str) -> String {
    format!("{COMMON}\nint main() {{\n    read_tree();\n    {answer}\n}}\n")
}

/// A tree of one of several shapes, since a solution may be slow or wrong on
/// only some of them.
fn tree(rng: &mut Rng, n: usize) -> Graph {
    match rng.weighted_index(&[3, 2, 2, 1, 1, 1]) {
        0 => Graph::new_random_uniform_tree(rng, n),
        1 => Graph::new_random_tree(rng, n),
        2 => Graph::new_random_deep_tree(rng, n),
        3 => Graph::new_caterpillar(rng, n, n.div_ceil(2)),
        4 => Graph::new_binary_tree(rng, n),
        _ => Graph::new_star(rng, n),
    }
}

/// A generator of trees with up to `max_n` nodes, more often many than few.
fn trees(max_n: usize) -> impl Fn(&mut Rng) -> Graph + Send + Sync {
    move |rng| {
        let n = rng.random_range_skewed(2..=max_n, 2);
        tree(rng, n)
    }
}

fn main() -> Result<()> {
    // Given a tree, print the number of edges on its longest path.
    let task = Task::new("Diameter", &PathBuf::from("task3"))
        .with_solution_source(&program("printf(\"%d\\n\", farthest(farthest(1).first).second);"))
        .with_time_limit(1000)
        .with_memory_limit(256);

    let paths = Subtask::new(20, "the tree is a path")
        .with_test(4, |rng| {
            let n = rng.random_range(2..=200_000);
            Graph::new_random_path(rng, n)
        })
        .with_test(1, |rng| Graph::new_random_path(rng, 200_000))
        .with_validator(|path: &Graph| assert!((0..path.num_nodes()).all(|node| path.degree(node) <= 2)));

    let small = Subtask::new(30, "n <= 2000")
        .with_test(8, trees(2000))
        .with_test(2, |rng| tree(rng, 2000))
        .with_validator(|tree: &Graph| assert!(tree.num_nodes() <= 2000 && tree.is_tree()));

    let large = Subtask::new(50, "n <= 200000")
        .with_test(8, trees(200_000))
        .with_test(2, |rng| tree(rng, 200_000))
        .with_validator(|tree: &Graph| assert!(tree.num_nodes() <= 200_000 && tree.is_tree()));

    task.with_subtask(paths)
        .with_subtask(small)
        .with_subtask(large)
        // Right, but quadratic.
        .with_partial_solution(
            "from every node",
            &program("int best = 0;\n    for (int from = 1; from <= n; from++) best = max(best, farthest(from).second);\n    printf(\"%d\\n\", best);"),
            &[1],
        )
        // Only right if node 1 happens to be an end of a longest path.
        .with_partial_solution("from node 1", &program("printf(\"%d\\n\", farthest(1).second);"), &[])
        .run()
}
