use crate::ToOutput;
use crate::rng::Rng;
use std::borrow::Cow;
use std::collections::HashSet;
use std::hash::{BuildHasherDefault, Hasher};
use std::ops::RangeBounds;

/// Nodes are written from 1 unless [`Graph::zero_based`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GraphFormat {
    /// `n m`, then one edge per line.
    #[default]
    EdgeList,
    /// `n`, then one edge per line. What the tree constructors start with.
    Tree,
    /// `n`, then the parents of nodes `2..=n` on one line, rooted at node 1.
    ParentList,
    /// One edge per line.
    EdgesOnly,
}

/// `SipHash` took longer than everything else done to an edge, and guards against crafted collisions,
/// which cannot happen here.
#[derive(Default)]
struct EdgeHasher(u64);

impl Hasher for EdgeHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.write_usize(usize::from(byte));
        }
    }

    fn write_usize(&mut self, value: usize) {
        self.0 = (self.0.rotate_left(5) ^ value as u64).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

/// A simple graph, undirected unless built directed; the structural checks ignore directions. Every
/// constructor takes the `Rng`, as writing the graph shuffles its edges.
pub struct Graph {
    /// In a directed graph, the nodes the edges lead to.
    adjacency: Vec<Vec<usize>>,
    /// In the order they were added; an undirected edge has its larger end first.
    edges: Vec<(usize, usize)>,
    edge_set: HashSet<(usize, usize), BuildHasherDefault<EdgeHasher>>,
    directed: bool,
    weights: Option<Vec<i64>>,
    format: GraphFormat,
    zero_based: bool,
    output_seed: u64,
}

/// 64 bits, as 32 overflow from n = 65537 on.
const fn max_simple_edges(n: usize) -> u64 {
    let n = n as u64;
    n * n.saturating_sub(1) / 2
}

const fn max_bipartite_edges(n: usize) -> u64 {
    let n = n as u64;
    (n / 2) * n.div_ceil(2)
}

fn assert_edges_fit(n: usize, m: usize, capacity: u64, kind: &str) {
    assert!(m as u64 <= capacity, "cannot fit {m} edges in a {kind} with {n} nodes (at most {capacity} are possible)");
}

impl Graph {
    fn new(rng: &mut Rng, n: usize, directed: bool) -> Self {
        Self {
            adjacency: vec![Vec::new(); n],
            edges: Vec::new(),
            edge_set: HashSet::default(),
            directed,
            weights: None,
            format: GraphFormat::EdgeList,
            zero_based: false,
            output_seed: rng.next_u64(),
        }
    }

    #[must_use]
    pub fn new_empty(rng: &mut Rng, n: usize) -> Self {
        Self::new(rng, n, false)
    }

    #[must_use]
    pub fn new_empty_directed(rng: &mut Rng, n: usize) -> Self {
        Self::new(rng, n, true)
    }

    #[must_use]
    pub fn new_full(rng: &mut Rng, n: usize) -> Self {
        let mut result = Self::new_empty(rng, n);
        for u in 0..n {
            for v in 0..u {
                result.add_edge(u, v);
            }
        }
        result
    }

    #[must_use]
    pub fn new_random(rng: &mut Rng, n: usize, m: usize) -> Self {
        assert_edges_fit(n, m, max_simple_edges(n), "graph");
        let mut result = Self::new_empty(rng, n);
        result.add_random_edges(rng, m, max_simple_edges(n), false, |u, v| (u, v));
        result
    }

    /// Two nodes may be joined in both directions.
    #[must_use]
    pub fn new_random_directed(rng: &mut Rng, n: usize, m: usize) -> Self {
        assert_edges_fit(n, m, 2 * max_simple_edges(n), "directed graph");
        let mut result = Self::new_empty_directed(rng, n);
        result.add_random_edges(rng, m, 2 * max_simple_edges(n), true, |u, v| (u, v));
        result
    }

    /// The nodes are in no particular topological order.
    #[must_use]
    pub fn new_random_dag(rng: &mut Rng, n: usize, m: usize) -> Self {
        assert_edges_fit(n, m, max_simple_edges(n), "directed acyclic graph");
        let mut result = Self::new_empty_directed(rng, n);
        // Edges lead forward in a random order of the nodes.
        let position = rng.shuffled(0..n);
        result.add_random_edges(rng, m, max_simple_edges(n), false, |u, v| if position[u] < position[v] { (u, v) } else { (v, u) });
        result
    }

    /// Adds random edges until there are `m`. `orient` turns two nodes into an edge, `both_ways` says
    /// whether the two orders of a pair are different edges, `capacity` is how many there can be.
    fn add_random_edges<F: Fn(usize, usize) -> (usize, usize)>(&mut self, rng: &mut Rng, m: usize, capacity: u64, both_ways: bool, orient: F) {
        let n = self.num_nodes();

        // Guessing pairs is cheap while at most half are taken; beyond that the free pairs are listed,
        // as guessing the last few takes unboundedly long.
        if m as u64 * 2 <= capacity {
            while self.num_edges() < m {
                let (u, v) = (rng.random_range(0..n), rng.random_range(0..n));
                if u != v {
                    let (from, to) = orient(u, v);
                    self.add_edge(from, to);
                }
            }
            return;
        }

        let candidates = (0..n)
            .flat_map(|u| (0..if both_ways { n } else { u }).map(move |v| (u, v)))
            .filter(|&(u, v)| u != v)
            .map(|(u, v)| orient(u, v))
            .filter(|&(from, to)| !self.has_edge(from, to))
            .collect();
        self.add_edges_from(rng, candidates, m);
    }

    /// `candidates` are edges the graph does not have.
    fn add_edges_from(&mut self, rng: &mut Rng, candidates: Vec<(usize, usize)>, m: usize) {
        let missing = m.saturating_sub(self.num_edges());
        for (u, v) in rng.shuffled(candidates).into_iter().take(missing) {
            self.add_edge(u, v);
        }
    }

    /// Joins each node of a shuffled order to the one `parent` picks from `(rng, order, position)`.
    fn new_shuffled_tree<F: FnMut(&mut Rng, &[usize], usize) -> usize>(rng: &mut Rng, n: usize, mut parent: F) -> Self {
        assert!(n >= 1, "a tree needs at least one node, got {n}");
        let mut result = Self::new_empty(rng, n);
        result.format = GraphFormat::Tree;
        let nodes = rng.shuffled(0..n);
        for i in 1..n {
            let v = parent(rng, &nodes, i);
            result.add_edge(nodes[i], v);
        }
        result
    }

    #[must_use]
    pub fn new_random_path(rng: &mut Rng, n: usize) -> Self {
        Self::new_shuffled_tree(rng, n, |_rng, nodes, i| nodes[i - 1])
    }

    /// Each node joined to a random earlier one: about `ln n` deep.
    #[must_use]
    pub fn new_random_tree(rng: &mut Rng, n: usize) -> Self {
        Self::new_shuffled_tree(rng, n, |rng, nodes, i| nodes[rng.random_range(0..i)])
    }

    /// About `n / 2` deep.
    #[must_use]
    pub fn new_random_deep_tree(rng: &mut Rng, n: usize) -> Self {
        // Half the nodes extend one chain.
        let mut chain_end = None;
        Self::new_shuffled_tree(rng, n, |rng, nodes, i| {
            if rng.random_bool(0.5) {
                chain_end.replace(nodes[i]).unwrap_or(nodes[0])
            } else {
                nodes[rng.random_range(0..i)]
            }
        })
    }

    /// Uniform among all labelled trees: about `sqrt(n)` deep.
    #[must_use]
    pub fn new_random_uniform_tree(rng: &mut Rng, n: usize) -> Self {
        assert!(n >= 1, "a tree needs at least one node, got {n}");
        let mut result = Self::new_empty(rng, n);
        result.format = GraphFormat::Tree;
        if n == 1 {
            return result;
        }

        // Every sequence of n - 2 nodes is the Prüfer code of exactly one tree.
        let code: Vec<usize> = rng.random_vec(n - 2, 0..n);
        let mut degree = vec![1; n];
        for &node in &code {
            degree[node] += 1;
        }
        // The smallest leaf is joined to the next node of the code and dropped. Leaves below `pointer`
        // are all used, except a node that only just became one.
        let mut pointer = 0;
        while degree[pointer] != 1 {
            pointer += 1;
        }
        let mut leaf = pointer;
        for &node in &code {
            result.add_edge(leaf, node);
            degree[node] -= 1;
            if degree[node] == 1 && node < pointer {
                leaf = node;
            } else {
                pointer += 1;
                while degree[pointer] != 1 {
                    pointer += 1;
                }
                leaf = pointer;
            }
        }
        // Two nodes are left, and the last node is always one of them.
        result.add_edge(leaf, n - 1);
        result
    }

    #[must_use]
    pub fn new_star(rng: &mut Rng, n: usize) -> Self {
        Self::new_shuffled_tree(rng, n, |_rng, nodes, _i| nodes[0])
    }

    /// A path of `spine` nodes, with every other node joined to one of them.
    #[must_use]
    pub fn new_caterpillar(rng: &mut Rng, n: usize, spine: usize) -> Self {
        assert!((1..=n).contains(&spine), "the spine of a caterpillar with {n} nodes cannot have {spine}");
        Self::new_shuffled_tree(rng, n, |rng, nodes, i| if i < spine { nodes[i - 1] } else { nodes[rng.random_range(0..spine)] })
    }

    /// Complete, with randomly numbered nodes.
    #[must_use]
    pub fn new_binary_tree(rng: &mut Rng, n: usize) -> Self {
        Self::new_shuffled_tree(rng, n, |_rng, nodes, i| nodes[(i - 1) / 2])
    }

    #[must_use]
    pub fn new_random_forest(rng: &mut Rng, n: usize, trees: usize) -> Self {
        assert!((1..=n).contains(&trees), "a forest with {n} nodes cannot have {trees} trees");
        let mut result = Self::new_empty(rng, n);
        let nodes = rng.shuffled(0..n);
        for i in trees..n {
            result.add_edge(nodes[i], nodes[rng.random_range(0..i)]);
        }
        result
    }

    #[must_use]
    pub fn new_cycle(rng: &mut Rng, n: usize) -> Self {
        assert!(n >= 3, "a cycle needs at least three nodes, got {n}");
        let mut result = Self::new_empty(rng, n);
        let nodes = rng.shuffled(0..n);
        for i in 0..n {
            result.add_edge(nodes[i], nodes[(i + 1) % n]);
        }
        result
    }

    /// Node `r * columns + c` is in row `r` and column `c`.
    #[must_use]
    pub fn new_grid(rng: &mut Rng, rows: usize, columns: usize) -> Self {
        let mut result = Self::new_empty(rng, rows * columns);
        for row in 0..rows {
            for column in 0..columns {
                let node = row * columns + column;
                if column + 1 < columns {
                    result.add_edge(node, node + 1);
                }
                if row + 1 < rows {
                    result.add_edge(node, node + columns);
                }
            }
        }
        result
    }

    /// `max(m, n - 1)` edges.
    #[must_use]
    pub fn new_random_connected(rng: &mut Rng, n: usize, m: usize) -> Self {
        assert_edges_fit(n, m, max_simple_edges(n), "graph");
        let mut result = Self::new_random_tree(rng, n);
        result.format = GraphFormat::EdgeList;
        result.add_random_edges(rng, m, max_simple_edges(n), false, |u, v| (u, v));
        result
    }

    #[must_use]
    pub fn new_random_bipartite(rng: &mut Rng, n: usize, m: usize) -> Self {
        assert!(n >= 2, "a bipartite graph needs at least two nodes, got {n}");
        assert_edges_fit(n, m, max_bipartite_edges(n), "bipartite graph");

        let mut result = Self::new_empty(rng, n);
        let nodes = rng.shuffled(0..n);

        // The splits with room for `m` edges form an interval symmetric around n / 2.
        let mut smallest_side = 1;
        while smallest_side as u64 * ((n - smallest_side) as u64) < m as u64 {
            smallest_side += 1;
        }
        let size1 = rng.random_range(smallest_side..=n - smallest_side);

        // As in `add_random_edges`.
        if m as u64 * 2 <= size1 as u64 * (n - size1) as u64 {
            while result.num_edges() < m {
                let u = nodes[rng.random_range(0..size1)];
                let v = nodes[rng.random_range(size1..n)];
                result.add_edge(u, v);
            }
            return result;
        }

        let (left, right) = nodes.split_at(size1);
        let candidates = left.iter().flat_map(|&u| right.iter().map(move |&v| (u, v))).collect();
        result.add_edges_from(rng, candidates, m);
        result
    }

    #[must_use]
    pub const fn with_format(mut self, format: GraphFormat) -> Self {
        self.format = format;
        self
    }

    #[must_use]
    pub const fn zero_based(mut self) -> Self {
        self.zero_based = true;
        self
    }

    /// Written after each edge. Edges added afterwards have none.
    #[must_use]
    pub fn with_random_weights<R: RangeBounds<i64> + Clone>(self, rng: &mut Rng, range: R) -> Self {
        let weights = rng.random_vec(self.edges.len(), range);
        self.with_weights(weights)
    }

    /// In the order of [`Graph::edges`].
    #[must_use]
    pub fn with_weights(mut self, weights: Vec<i64>) -> Self {
        assert_eq!(weights.len(), self.edges.len(), "a graph needs one weight per edge");
        self.weights = Some(weights);
        self
    }

    /// Hides the structure a constructor leaves in the numbers, such as a grid's rows.
    #[must_use]
    pub fn with_shuffled_labels(mut self, rng: &mut Rng) -> Self {
        let labels = rng.shuffled(0..self.num_nodes());
        self.edge_set.clear();
        self.adjacency.iter_mut().for_each(Vec::clear);
        for (u, v) in std::mem::take(&mut self.edges) {
            self.add_edge(labels[u], labels[v]);
        }
        self
    }

    fn key(&self, u: usize, v: usize) -> (usize, usize) {
        if self.directed { (u, v) } else { (usize::max(u, v), usize::min(u, v)) }
    }

    #[must_use]
    pub fn has_edge(&self, u: usize, v: usize) -> bool {
        self.edge_set.contains(&self.key(u, v))
    }

    #[must_use]
    pub const fn num_edges(&self) -> usize {
        self.edges.len()
    }

    #[must_use]
    pub const fn num_nodes(&self) -> usize {
        self.adjacency.len()
    }

    #[must_use]
    pub const fn is_directed(&self) -> bool {
        self.directed
    }

    /// Ignores loops and edges it has.
    pub fn add_edge(&mut self, u: usize, v: usize) {
        let n = self.num_nodes();
        assert!(u < n && v < n, "cannot join nodes {u} and {v} of a graph with {n} nodes");
        let edge = self.key(u, v);
        if u != v && self.edge_set.insert(edge) {
            self.edges.push(edge);
            self.adjacency[u].push(v);
            if !self.directed {
                self.adjacency[v].push(u);
            }
        }
    }

    /// In the order they were added; an undirected edge has its larger end first.
    #[must_use]
    pub fn edges(&self) -> &[(usize, usize)] {
        &self.edges
    }

    #[must_use]
    pub fn weights(&self) -> Option<&[i64]> {
        self.weights.as_deref()
    }

    /// In a directed graph, the nodes the edges lead to.
    #[must_use]
    pub fn neighbours(&self, node: usize) -> &[usize] {
        &self.adjacency[node]
    }

    /// In a directed graph, of the edges leaving `node`.
    #[must_use]
    pub fn degree(&self, node: usize) -> usize {
        self.adjacency[node].len()
    }

    fn undirected_adjacency(&self) -> Cow<'_, [Vec<usize>]> {
        if !self.directed {
            return Cow::Borrowed(&self.adjacency);
        }
        let mut adjacency = self.adjacency.clone();
        for &(from, to) in &self.edges {
            adjacency[to].push(from);
        }
        Cow::Owned(adjacency)
    }

    #[must_use]
    pub fn connected_components(&self) -> Vec<Vec<usize>> {
        let adjacency = self.undirected_adjacency();
        let mut visited = vec![false; adjacency.len()];
        let mut result = Vec::new();
        for start in 0..adjacency.len() {
            if visited[start] {
                continue;
            }
            visited[start] = true;
            let (mut component, mut stack) = (Vec::new(), vec![start]);
            while let Some(u) = stack.pop() {
                component.push(u);
                for &v in &adjacency[u] {
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

    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.connected_components().len() == 1
    }

    #[must_use]
    pub fn is_tree(&self) -> bool {
        self.num_edges() + 1 == self.num_nodes() && self.is_connected()
    }

    #[must_use]
    pub const fn is_full(&self) -> bool {
        let capacity = max_simple_edges(self.num_nodes()) * if self.directed { 2 } else { 1 };
        self.num_edges() as u64 == capacity
    }

    #[must_use]
    pub fn is_bipartite(&self) -> bool {
        let adjacency = self.undirected_adjacency();
        // 0 unvisited, else 1 or -1.
        let mut colors = vec![0_i8; adjacency.len()];
        for start in 0..adjacency.len() {
            if colors[start] != 0 {
                continue;
            }
            colors[start] = 1;
            let mut stack = vec![start];
            while let Some(u) = stack.pop() {
                for &v in &adjacency[u] {
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

    /// In the tree rooted at node 0, which is its own parent.
    fn parents(&self) -> Vec<usize> {
        let adjacency = self.undirected_adjacency();
        let mut parents = vec![0; adjacency.len()];
        let mut stack = vec![0];
        while let Some(node) = stack.pop() {
            for &neighbour in &adjacency[node] {
                if neighbour != parents[node] && neighbour != 0 {
                    parents[neighbour] = node;
                    stack.push(neighbour);
                }
            }
        }
        parents
    }
}

impl ToOutput for Graph {
    /// Edges, and the ends of undirected ones, in random order.
    fn write_output(self, out: &mut String) {
        let is_tree_format = matches!(self.format, GraphFormat::Tree | GraphFormat::ParentList);
        assert!(!is_tree_format || self.is_tree(), "the graph is written as a tree but is not one");
        if let Some(weights) = &self.weights {
            assert_eq!(weights.len(), self.edges.len(), "edges were added to the graph after its weights");
        }

        let first_label = usize::from(!self.zero_based);
        match self.format {
            GraphFormat::EdgeList => (self.num_nodes(), self.num_edges()).write_output(out),
            GraphFormat::Tree | GraphFormat::ParentList => (self.num_nodes(),).write_output(out),
            GraphFormat::EdgesOnly => {}
        }

        if self.format == GraphFormat::ParentList {
            let parents = self.parents();
            parents[1..].iter().map(|parent| parent + first_label).collect::<Vec<_>>().write_output(out);
            if let Some(weights) = &self.weights {
                // An edge's weight goes to its child end.
                let mut above = vec![0; parents.len()];
                for (&(u, v), &weight) in self.edges.iter().zip(weights) {
                    above[if parents[u] == v { u } else { v }] = weight;
                }
                above[1..].write_output(out);
            }
            return;
        }

        let mut order: Vec<usize> = (0..self.edges.len()).collect();
        let mut rng = Rng::from_seed(self.output_seed);
        rng.shuffle(&mut order);
        for edge in order {
            let (u, v) = self.edges[edge];
            let (first, second) = if self.directed || rng.random_bool(0.5) { (u, v) } else { (v, u) };
            let (first, second) = (first + first_label, second + first_label);
            match &self.weights {
                Some(weights) => (first, second, weights[edge]).write_output(out),
                None => (first, second).write_output(out),
            }
        }
    }
}
