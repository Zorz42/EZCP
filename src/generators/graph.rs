use crate::ToOutput;
use crate::rng::Rng;
use std::collections::HashSet;
use std::fmt::Write;

/// A simple undirected graph.
///
/// Every constructor takes the generator's [`Rng`], even the deterministic ones,
/// because writing the graph out shuffles its edges.
pub struct Graph {
    nodes: Vec<Vec<usize>>,
    /// In insertion order, since iterating `edge_set` would differ between runs.
    edges: Vec<(usize, usize)>,
    edge_set: HashSet<(usize, usize)>,
    output_seed: u64,
    /// If set, the graph is written without its edge count and must be a tree.
    pub is_tree: bool,
}

/// In 64 bits, since it overflows `i32` from n = 65537 on.
const fn max_simple_edges(n: i32) -> i64 {
    let n = n as i64;
    n * (n - 1) / 2
}

const fn max_bipartite_edges(n: i32) -> i64 {
    let n = n as i64;
    (n / 2) * ((n + 1) / 2)
}

fn assert_edges_fit(n: i32, m: i32) {
    assert!(
        i64::from(m) <= max_simple_edges(n),
        "cannot fit {m} edges in a graph with {n} nodes (at most {} are possible)",
        max_simple_edges(n)
    );
}

impl Graph {
    /// Creates a graph with `n` nodes and no edges.
    ///
    /// # Panics
    /// Panics if `n` is negative.
    #[must_use]
    pub fn new_empty(rng: &mut Rng, n: i32) -> Self {
        assert!(n >= 0, "a graph cannot have {n} nodes");
        Self {
            nodes: vec![Vec::new(); n as usize],
            edges: Vec::new(),
            edge_set: HashSet::new(),
            output_seed: rng.next_seed(),
            is_tree: false,
        }
    }

    /// Creates a complete graph with `n` nodes.
    ///
    /// # Panics
    /// Panics if `n` is negative.
    #[must_use]
    pub fn new_full(rng: &mut Rng, n: i32) -> Self {
        let mut result = Self::new_empty(rng, n);
        for u in 0..n {
            for v in 0..u {
                result.add_edge(u as usize, v as usize);
            }
        }
        result
    }

    /// Creates a random graph with `n` nodes and `m` edges.
    ///
    /// # Panics
    /// Panics if `m` edges do not fit in a simple graph on `n` nodes.
    #[must_use]
    pub fn new_random(rng: &mut Rng, n: i32, m: i32) -> Self {
        let mut result = Self::new_empty(rng, n);
        assert!(m >= 0, "a graph cannot have {m} edges");
        assert_edges_fit(n, m);
        result.add_random_edges(rng, m);
        result
    }

    /// Adds random edges until there are `m`.
    fn add_random_edges(&mut self, rng: &mut Rng, m: i32) {
        let n = self.get_num_nodes();

        // Guessing pairs is cheap while at most half of all edges are taken.
        // Beyond that the free pairs are enumerated instead, since guessing the
        // last few would take unboundedly long.
        if i64::from(m) * 2 <= max_simple_edges(n) {
            while self.get_num_edges() < m {
                let u = rng.random_range(0..n);
                let v = rng.random_range(0..n);
                self.add_edge(u as usize, v as usize);
            }
            return;
        }

        let candidates = (0..n as usize).flat_map(|u| (0..u).map(move |v| (u, v))).filter(|&(u, v)| !self.has_edge(u, v)).collect();
        self.add_edges_from(rng, candidates, m);
    }

    /// Adds edges from `candidates` in random order until there are `m`.
    fn add_edges_from(&mut self, rng: &mut Rng, mut candidates: Vec<(usize, usize)>, m: i32) {
        rng.shuffle(&mut candidates);

        for (u, v) in candidates {
            if self.get_num_edges() >= m {
                break;
            }
            self.add_edge(u, v);
        }
    }

    /// A tree on the nodes in shuffled order, where each node is joined to the
    /// one `parent` picks from `(rng, shuffled nodes, its position)`.
    fn new_shuffled_tree(rng: &mut Rng, n: i32, mut parent: impl FnMut(&mut Rng, &[i32], i32) -> i32) -> Self {
        assert!(n >= 1, "a tree needs at least one node, got {n}");
        let mut result = Self::new_empty(rng, n);
        result.is_tree = true;
        let mut nodes = (0..n).collect::<Vec<_>>();
        rng.shuffle(&mut nodes);
        for i in 1..n {
            let v = parent(rng, &nodes, i);
            result.add_edge(nodes[i as usize] as usize, v as usize);
        }
        result
    }

    /// Creates a random path, which counts as a tree.
    ///
    /// # Panics
    /// Panics if `n` is not positive.
    #[must_use]
    pub fn new_random_path(rng: &mut Rng, n: i32) -> Self {
        Self::new_shuffled_tree(rng, n, |_rng, nodes, i| nodes[i as usize - 1])
    }

    /// Creates a random tree with `n` nodes.
    ///
    /// # Panics
    /// Panics if `n` is not positive.
    #[must_use]
    pub fn new_random_tree(rng: &mut Rng, n: i32) -> Self {
        Self::new_shuffled_tree(rng, n, |rng, nodes, i| nodes[rng.random_range(0..i) as usize])
    }

    /// Creates a random tree with depth in O(n).
    ///
    /// # Panics
    /// Panics if `n` is not positive.
    #[must_use]
    pub fn new_random_deep_tree(rng: &mut Rng, n: i32) -> Self {
        // Half the nodes extend one chain.
        let mut chain_end = None;
        Self::new_shuffled_tree(rng, n, |rng, nodes, i| {
            if rng.random_bool(0.5) {
                chain_end.replace(nodes[i as usize]).unwrap_or(nodes[0])
            } else {
                nodes[rng.random_range(0..i) as usize]
            }
        })
    }

    /// Creates a random connected graph with `n` nodes and `max(m, n - 1)` edges.
    ///
    /// # Panics
    /// Panics if `n` is not positive, or if `m` edges do not fit in a simple graph
    /// on `n` nodes.
    #[must_use]
    pub fn new_random_connected(rng: &mut Rng, n: i32, m: i32) -> Self {
        assert_edges_fit(n, m);
        let mut result = Self::new_random_tree(rng, n);
        result.is_tree = false;
        result.add_random_edges(rng, m);
        result
    }

    /// Creates a random bipartite graph with `n` nodes and `m` edges.
    ///
    /// # Panics
    /// Panics if `n` is less than two, or if `m` edges do not fit in any bipartition
    /// of `n` nodes.
    #[must_use]
    pub fn new_random_bipartite(rng: &mut Rng, n: i32, m: i32) -> Self {
        assert!(n >= 2, "a bipartite graph needs at least two nodes, got {n}");
        assert!(m >= 0, "a graph cannot have {m} edges");
        assert!(
            i64::from(m) <= max_bipartite_edges(n),
            "cannot fit {m} edges in a bipartite graph with {n} nodes (at most {} are possible)",
            max_bipartite_edges(n)
        );

        let mut result = Self::new_empty(rng, n);
        let mut nodes = (0..n).collect::<Vec<_>>();
        rng.shuffle(&mut nodes);

        // The splits with room for `m` edges form an interval symmetric around n / 2.
        let mut smallest_side = 1_i64;
        while smallest_side * (i64::from(n) - smallest_side) < i64::from(m) {
            smallest_side += 1;
        }
        let size1 = rng.random_range(smallest_side..=i64::from(n) - smallest_side) as i32;

        // As in `add_random_edges`.
        if i64::from(m) * 2 <= i64::from(size1) * i64::from(n - size1) {
            while result.get_num_edges() < m {
                let u = nodes[rng.random_range(0..size1) as usize];
                let v = nodes[rng.random_range(size1..n) as usize];
                result.add_edge(u as usize, v as usize);
            }
            return result;
        }

        let (left, right) = nodes.split_at(size1 as usize);
        let candidates = left.iter().flat_map(|&u| right.iter().map(move |&v| (u as usize, v as usize))).collect();
        result.add_edges_from(rng, candidates, m);
        result
    }

    /// Whether `u` and `v` are adjacent.
    #[must_use]
    pub fn has_edge(&self, u: usize, v: usize) -> bool {
        self.edge_set.contains(&(usize::max(u, v), usize::min(u, v)))
    }

    /// The number of edges.
    #[must_use]
    pub const fn get_num_edges(&self) -> i32 {
        self.edges.len() as i32
    }

    /// The number of nodes.
    #[must_use]
    pub const fn get_num_nodes(&self) -> i32 {
        self.nodes.len() as i32
    }

    /// Adds an edge, unless it exists already or is a loop.
    pub fn add_edge(&mut self, u: usize, v: usize) {
        if !self.has_edge(u, v) && u != v {
            let edge = (usize::max(u, v), usize::min(u, v));
            self.edge_set.insert(edge);
            self.edges.push(edge);
            self.nodes[u].push(v);
            self.nodes[v].push(u);
        }
    }

    /// The edges, in the order they were added.
    pub fn edges_iter(&self) -> impl Iterator<Item = &(usize, usize)> {
        self.edges.iter()
    }

    /// The connected components, as lists of nodes in no particular order.
    #[must_use]
    pub fn get_connected_components(&self) -> Vec<Vec<usize>> {
        let mut visited = vec![false; self.nodes.len()];
        let mut result = Vec::new();
        for start in 0..self.nodes.len() {
            if visited[start] {
                continue;
            }
            visited[start] = true;
            let (mut component, mut stack) = (Vec::new(), vec![start]);
            while let Some(u) = stack.pop() {
                component.push(u);
                for &v in &self.nodes[u] {
                    if !visited[v] {
                        visited[v] = true;
                        stack.push(v);
                    }
                }
            }
            result.push(component);
        }
        result
    }

    /// Whether the graph is connected.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.get_connected_components().len() == 1
    }

    /// Whether the graph is a tree.
    #[must_use]
    pub fn is_tree(&self) -> bool {
        self.get_num_edges() == self.get_num_nodes() - 1 && self.is_connected()
    }

    /// Whether the graph is complete.
    #[must_use]
    pub fn is_full(&self) -> bool {
        i64::from(self.get_num_edges()) == max_simple_edges(self.get_num_nodes())
    }

    /// Whether the graph is bipartite.
    #[must_use]
    pub fn is_bipartite(&self) -> bool {
        // 0 while unvisited, then 1 or -1 for the two sides.
        let mut colors = vec![0_i8; self.nodes.len()];
        for start in 0..self.nodes.len() {
            if colors[start] != 0 {
                continue;
            }
            colors[start] = 1;
            let mut stack = vec![start];
            while let Some(u) = stack.pop() {
                for &v in &self.nodes[u] {
                    if colors[v] == 0 {
                        colors[v] = -colors[u];
                        stack.push(v);
                    } else if colors[v] == colors[u] {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// The nodes adjacent to `node`.
    ///
    /// # Panics
    /// Panics if `node` does not exist.
    #[must_use]
    pub fn get_neighbours(&self, node: usize) -> Vec<usize> {
        self.nodes[node].clone()
    }
}

impl ToOutput for Graph {
    /// `n m` (just `n` for a tree), then one 1-based edge per line, with the
    /// edges shuffled and their ends randomly swapped.
    ///
    /// # Panics
    /// Panics if `is_tree` is set but the graph is not a tree.
    fn to_output(self) -> String {
        assert!(!self.is_tree || self.is_tree(), "the graph is marked as a tree but is not one");
        let mut result = if self.is_tree {
            format!("{}\n", self.get_num_nodes())
        } else {
            format!("{} {}\n", self.get_num_nodes(), self.get_num_edges())
        };
        let mut edges = self.edges;
        let mut rng = Rng::from_seed(self.output_seed);
        rng.shuffle(&mut edges);
        for (u, v) in edges {
            let (first, second) = if rng.random_bool(0.5) { (u, v) } else { (v, u) };
            writeln!(result, "{} {}", first + 1, second + 1).ok();
        }
        result
    }
}
