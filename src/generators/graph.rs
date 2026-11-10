use crate::ToOutput;
use crate::rng::Rng;
use std::borrow::Cow;
use std::collections::HashSet;
use std::ops::RangeBounds;

/// How a [`Graph`] is written. Nodes are numbered from 1 unless
/// [`Graph::zero_based`] is used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GraphFormat {
    /// `n m` on the first line, then one edge per line.
    #[default]
    EdgeList,
    /// `n` on the first line, then the `n - 1` edges, one per line. Only for
    /// trees, and what the tree constructors start out with.
    Tree,
    /// `n` on the first line and the parents of nodes `2..=n` on the second,
    /// with node `1` as the root. Only for trees.
    ParentList,
    /// One edge per line and nothing else, for when the counts are written
    /// somewhere else.
    EdgesOnly,
}

/// A simple graph: undirected, unless it comes from one of the directed
/// constructors.
///
/// Every constructor takes the generator's [`Rng`], even the deterministic ones,
/// because writing the graph out shuffles its edges.
///
/// ```
/// use ezcp::{Graph, GraphFormat, Rng, ToOutput};
///
/// let mut rng = Rng::from_seed(1);
/// let tree = Graph::new_random_tree(&mut rng, 4).with_format(GraphFormat::ParentList);
/// assert_eq!(tree.to_output().lines().count(), 2);
/// ```
pub struct Graph {
    /// The neighbours of every node; in a directed graph, the nodes its edges
    /// lead to.
    adjacency: Vec<Vec<usize>>,
    /// In insertion order, since iterating `edge_set` would differ between runs.
    /// An undirected edge has its larger end first.
    edges: Vec<(usize, usize)>,
    edge_set: HashSet<(usize, usize)>,
    directed: bool,
    /// One per edge, in the order of `edges`.
    weights: Option<Vec<i64>>,
    format: GraphFormat,
    zero_based: bool,
    output_seed: u64,
}

/// In 64 bits, since it overflows 32 from n = 65537 on.
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
            edge_set: HashSet::new(),
            directed,
            weights: None,
            format: GraphFormat::EdgeList,
            zero_based: false,
            output_seed: rng.next_seed(),
        }
    }

    /// Creates a graph with `n` nodes and no edges.
    #[must_use]
    pub fn new_empty(rng: &mut Rng, n: usize) -> Self {
        Self::new(rng, n, false)
    }

    /// Creates a directed graph with `n` nodes and no edges.
    #[must_use]
    pub fn new_empty_directed(rng: &mut Rng, n: usize) -> Self {
        Self::new(rng, n, true)
    }

    /// Creates a complete graph with `n` nodes.
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

    /// Creates a random graph with `n` nodes and `m` edges.
    ///
    /// # Panics
    /// Panics if `m` edges do not fit in a simple graph on `n` nodes.
    #[must_use]
    pub fn new_random(rng: &mut Rng, n: usize, m: usize) -> Self {
        assert_edges_fit(n, m, max_simple_edges(n), "graph");
        let mut result = Self::new_empty(rng, n);
        result.add_random_edges(rng, m, max_simple_edges(n), false, |u, v| (u, v));
        result
    }

    /// Creates a random directed graph with `n` nodes and `m` edges. Two nodes
    /// may be joined in both directions.
    ///
    /// # Panics
    /// Panics if `m` edges do not fit in a simple directed graph on `n` nodes.
    #[must_use]
    pub fn new_random_directed(rng: &mut Rng, n: usize, m: usize) -> Self {
        assert_edges_fit(n, m, 2 * max_simple_edges(n), "directed graph");
        let mut result = Self::new_empty_directed(rng, n);
        result.add_random_edges(rng, m, 2 * max_simple_edges(n), true, |u, v| (u, v));
        result
    }

    /// Creates a random directed acyclic graph with `n` nodes and `m` edges.
    /// The nodes are in no particular topological order.
    ///
    /// # Panics
    /// Panics if `m` edges do not fit in an acyclic graph on `n` nodes.
    #[must_use]
    pub fn new_random_dag(rng: &mut Rng, n: usize, m: usize) -> Self {
        assert_edges_fit(n, m, max_simple_edges(n), "directed acyclic graph");
        let mut result = Self::new_empty_directed(rng, n);
        // Every edge leads from an earlier node of a random order to a later one.
        let mut position: Vec<usize> = (0..n).collect();
        rng.shuffle(&mut position);
        result.add_random_edges(rng, m, max_simple_edges(n), false, |u, v| if position[u] < position[v] { (u, v) } else { (v, u) });
        result
    }

    /// Adds random edges until there are `m`. `orient` turns two different
    /// nodes into an edge, `both_ways` says whether the two orders of a pair
    /// give different edges, and `capacity` is how many edges there can be.
    fn add_random_edges<F: Fn(usize, usize) -> (usize, usize)>(&mut self, rng: &mut Rng, m: usize, capacity: u64, both_ways: bool, orient: F) {
        let n = self.num_nodes();

        // Guessing pairs is cheap while at most half of all edges are taken.
        // Beyond that the free pairs are enumerated instead, since guessing the
        // last few would take unboundedly long.
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

    /// Adds edges from `candidates` in random order until there are `m`.
    fn add_edges_from(&mut self, rng: &mut Rng, mut candidates: Vec<(usize, usize)>, m: usize) {
        rng.shuffle(&mut candidates);

        for (u, v) in candidates {
            if self.num_edges() >= m {
                break;
            }
            self.add_edge(u, v);
        }
    }

    /// A tree on the nodes in shuffled order, where each node is joined to the
    /// one `parent` picks from `(rng, shuffled nodes, its position)`.
    fn new_shuffled_tree<F: FnMut(&mut Rng, &[usize], usize) -> usize>(rng: &mut Rng, n: usize, mut parent: F) -> Self {
        assert!(n >= 1, "a tree needs at least one node, got {n}");
        let mut result = Self::new_empty(rng, n);
        result.format = GraphFormat::Tree;
        let mut nodes = (0..n).collect::<Vec<_>>();
        rng.shuffle(&mut nodes);
        for i in 1..n {
            let v = parent(rng, &nodes, i);
            result.add_edge(nodes[i], v);
        }
        result
    }

    /// Creates a random path, which counts as a tree.
    ///
    /// # Panics
    /// Panics if `n` is zero.
    #[must_use]
    pub fn new_random_path(rng: &mut Rng, n: usize) -> Self {
        Self::new_shuffled_tree(rng, n, |_rng, nodes, i| nodes[i - 1])
    }

    /// Creates a random tree with `n` nodes by joining each node to a random
    /// earlier one. Such a tree is shallow, about `ln n` deep; see
    /// [`Graph::new_random_uniform_tree`] and [`Graph::new_random_deep_tree`]
    /// for others.
    ///
    /// # Panics
    /// Panics if `n` is zero.
    #[must_use]
    pub fn new_random_tree(rng: &mut Rng, n: usize) -> Self {
        Self::new_shuffled_tree(rng, n, |rng, nodes, i| nodes[rng.random_range(0..i)])
    }

    /// Creates a random tree with depth in O(n).
    ///
    /// # Panics
    /// Panics if `n` is zero.
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

    /// Creates a tree picked uniformly among all trees on `n` labelled nodes.
    /// Such a tree is about `sqrt(n)` deep.
    ///
    /// # Panics
    /// Panics if `n` is zero.
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
        // The smallest leaf is joined to the next node of the code, and then
        // left out. Leaves below `pointer` have all been used, except for a
        // node that only just became one.
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

    /// Creates a star: a random centre joined to every other node.
    ///
    /// # Panics
    /// Panics if `n` is zero.
    #[must_use]
    pub fn new_star(rng: &mut Rng, n: usize) -> Self {
        Self::new_shuffled_tree(rng, n, |_rng, nodes, _i| nodes[0])
    }

    /// Creates a caterpillar: a path of `spine` nodes, with every other node
    /// joined to a random node of the path.
    ///
    /// # Panics
    /// Panics if `spine` is zero or more than `n`.
    #[must_use]
    pub fn new_caterpillar(rng: &mut Rng, n: usize, spine: usize) -> Self {
        assert!((1..=n).contains(&spine), "the spine of a caterpillar with {n} nodes cannot have {spine}");
        Self::new_shuffled_tree(rng, n, |rng, nodes, i| if i < spine { nodes[i - 1] } else { nodes[rng.random_range(0..spine)] })
    }

    /// Creates a complete binary tree with randomly numbered nodes, which is
    /// about `log2(n)` deep.
    ///
    /// # Panics
    /// Panics if `n` is zero.
    #[must_use]
    pub fn new_binary_tree(rng: &mut Rng, n: usize) -> Self {
        Self::new_shuffled_tree(rng, n, |_rng, nodes, i| nodes[(i - 1) / 2])
    }

    /// Creates a forest of exactly `trees` random trees on `n` nodes.
    ///
    /// # Panics
    /// Panics if `trees` is zero or more than `n`.
    #[must_use]
    pub fn new_random_forest(rng: &mut Rng, n: usize, trees: usize) -> Self {
        assert!((1..=n).contains(&trees), "a forest with {n} nodes cannot have {trees} trees");
        let mut result = Self::new_empty(rng, n);
        let mut nodes = (0..n).collect::<Vec<_>>();
        rng.shuffle(&mut nodes);
        // The first `trees` nodes are the roots.
        for i in trees..n {
            result.add_edge(nodes[i], nodes[rng.random_range(0..i)]);
        }
        result
    }

    /// Creates a cycle through all `n` nodes in random order.
    ///
    /// # Panics
    /// Panics if `n` is less than three.
    #[must_use]
    pub fn new_cycle(rng: &mut Rng, n: usize) -> Self {
        assert!(n >= 3, "a cycle needs at least three nodes, got {n}");
        let mut result = Self::new_empty(rng, n);
        let mut nodes = (0..n).collect::<Vec<_>>();
        rng.shuffle(&mut nodes);
        for i in 0..n {
            result.add_edge(nodes[i], nodes[(i + 1) % n]);
        }
        result
    }

    /// Creates a grid of `rows` by `columns` nodes, each joined to the ones
    /// next to it. The node in row `r` and column `c`, both from zero, is
    /// `r * columns + c`.
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

    /// Creates a random connected graph with `n` nodes and `max(m, n - 1)` edges.
    ///
    /// # Panics
    /// Panics if `n` is zero, or if `m` edges do not fit in a simple graph on `n`
    /// nodes.
    #[must_use]
    pub fn new_random_connected(rng: &mut Rng, n: usize, m: usize) -> Self {
        assert_edges_fit(n, m, max_simple_edges(n), "graph");
        let mut result = Self::new_random_tree(rng, n);
        result.format = GraphFormat::EdgeList;
        result.add_random_edges(rng, m, max_simple_edges(n), false, |u, v| (u, v));
        result
    }

    /// Creates a random bipartite graph with `n` nodes and `m` edges.
    ///
    /// # Panics
    /// Panics if `n` is less than two, or if `m` edges do not fit in any bipartition
    /// of `n` nodes.
    #[must_use]
    pub fn new_random_bipartite(rng: &mut Rng, n: usize, m: usize) -> Self {
        assert!(n >= 2, "a bipartite graph needs at least two nodes, got {n}");
        assert_edges_fit(n, m, max_bipartite_edges(n), "bipartite graph");

        let mut result = Self::new_empty(rng, n);
        let mut nodes = (0..n).collect::<Vec<_>>();
        rng.shuffle(&mut nodes);

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

    /// Sets how the graph is written.
    #[must_use]
    pub const fn with_format(mut self, format: GraphFormat) -> Self {
        self.format = format;
        self
    }

    /// Numbers the nodes from 0 instead of 1 when the graph is written.
    #[must_use]
    pub const fn zero_based(mut self) -> Self {
        self.zero_based = true;
        self
    }

    /// Gives every edge a random weight from `range`, written after the edge.
    /// Edges added afterwards have none, so add them first.
    #[must_use]
    pub fn with_random_weights<R: RangeBounds<i64> + Clone>(self, rng: &mut Rng, range: R) -> Self {
        let weights = rng.random_vec(self.edges.len(), range);
        self.with_weights(weights)
    }

    /// Gives the edges these weights, in the order of [`Graph::edges`].
    ///
    /// # Panics
    /// Panics if there is not one weight per edge.
    #[must_use]
    pub fn with_weights(mut self, weights: Vec<i64>) -> Self {
        assert_eq!(weights.len(), self.edges.len(), "a graph needs one weight per edge");
        self.weights = Some(weights);
        self
    }

    /// Renumbers the nodes at random, which hides the structure a constructor
    /// leaves in the numbers, such as the rows of a grid.
    #[must_use]
    pub fn with_shuffled_labels(mut self, rng: &mut Rng) -> Self {
        let mut labels: Vec<usize> = (0..self.num_nodes()).collect();
        rng.shuffle(&mut labels);
        self.edge_set.clear();
        self.adjacency.iter_mut().for_each(Vec::clear);
        for (u, v) in std::mem::take(&mut self.edges) {
            self.add_edge(labels[u], labels[v]);
        }
        self
    }

    /// An undirected edge is the same edge whichever end comes first.
    fn key(&self, u: usize, v: usize) -> (usize, usize) {
        if self.directed { (u, v) } else { (usize::max(u, v), usize::min(u, v)) }
    }

    /// Whether `u` and `v` are adjacent; in a directed graph, whether there is
    /// an edge from `u` to `v`.
    #[must_use]
    pub fn has_edge(&self, u: usize, v: usize) -> bool {
        self.edge_set.contains(&self.key(u, v))
    }

    /// The number of edges.
    #[must_use]
    pub const fn num_edges(&self) -> usize {
        self.edges.len()
    }

    /// The number of nodes.
    #[must_use]
    pub const fn num_nodes(&self) -> usize {
        self.adjacency.len()
    }

    /// Whether the edges have a direction.
    #[must_use]
    pub const fn is_directed(&self) -> bool {
        self.directed
    }

    /// Adds an edge, in a directed graph from `u` to `v`, unless it exists
    /// already or is a loop.
    ///
    /// # Panics
    /// Panics if `u` or `v` does not exist.
    pub fn add_edge(&mut self, u: usize, v: usize) {
        let n = self.num_nodes();
        assert!(u < n && v < n, "cannot join nodes {u} and {v} of a graph with {n} nodes");
        if u != v && !self.has_edge(u, v) {
            let edge = self.key(u, v);
            self.edge_set.insert(edge);
            self.edges.push(edge);
            self.adjacency[u].push(v);
            if !self.directed {
                self.adjacency[v].push(u);
            }
        }
    }

    /// The edges, in the order they were added. An undirected edge has its
    /// larger end first; a directed one leads from the first node to the second.
    #[must_use]
    pub fn edges(&self) -> &[(usize, usize)] {
        &self.edges
    }

    /// The weights of the edges, in the order of [`Graph::edges`], if they were
    /// given any.
    #[must_use]
    pub fn weights(&self) -> Option<&[i64]> {
        self.weights.as_deref()
    }

    /// The nodes adjacent to `node`; in a directed graph, the nodes its edges
    /// lead to.
    ///
    /// # Panics
    /// Panics if `node` does not exist.
    #[must_use]
    pub fn neighbours(&self, node: usize) -> &[usize] {
        &self.adjacency[node]
    }

    /// The number of edges at `node`; in a directed graph, of those leaving it.
    ///
    /// # Panics
    /// Panics if `node` does not exist.
    #[must_use]
    pub fn degree(&self, node: usize) -> usize {
        self.adjacency[node].len()
    }

    /// The neighbours of every node, whatever the direction of the edges.
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

    /// The connected components, as lists of nodes in no particular order. The
    /// direction of edges does not matter.
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

    /// Whether the graph is connected. The direction of edges does not matter.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.connected_components().len() == 1
    }

    /// Whether the graph is a tree. The direction of edges does not matter.
    #[must_use]
    pub fn is_tree(&self) -> bool {
        self.num_edges() + 1 == self.num_nodes() && self.is_connected()
    }

    /// Whether the graph has every edge it can have.
    #[must_use]
    pub const fn is_full(&self) -> bool {
        let capacity = max_simple_edges(self.num_nodes()) * if self.directed { 2 } else { 1 };
        self.num_edges() as u64 == capacity
    }

    /// Whether the graph is bipartite. The direction of edges does not matter.
    #[must_use]
    pub fn is_bipartite(&self) -> bool {
        let adjacency = self.undirected_adjacency();
        // 0 while unvisited, then 1 or -1 for the two sides.
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

    /// For every node other than node 0, its parent in the tree rooted there
    /// and the edge between the two.
    fn parents(&self) -> Vec<(usize, usize)> {
        let mut edges_at = vec![Vec::new(); self.num_nodes()];
        for (edge, &(u, v)) in self.edges.iter().enumerate() {
            edges_at[u].push((v, edge));
            edges_at[v].push((u, edge));
        }
        let mut parents = vec![(0, 0); self.num_nodes()];
        let mut stack = vec![0];
        while let Some(node) = stack.pop() {
            for &(neighbour, edge) in &edges_at[node] {
                // The root is its own parent, and so never taken for a child.
                if neighbour != parents[node].0 && neighbour != 0 {
                    parents[neighbour] = (node, edge);
                    stack.push(neighbour);
                }
            }
        }
        parents.split_off(1)
    }
}

impl ToOutput for Graph {
    /// As its [`GraphFormat`] says. Edges are written in random order, and the
    /// ends of an undirected edge in random order too.
    ///
    /// # Panics
    /// Panics if the format is one for trees but the graph is not a tree, or if
    /// edges were added after the weights.
    fn to_output(self) -> String {
        let mut out = String::new();
        self.write_output(&mut out);
        out
    }

    fn write_output(self, out: &mut String) {
        let is_tree_format = matches!(self.format, GraphFormat::Tree | GraphFormat::ParentList);
        assert!(!is_tree_format || self.is_tree(), "the graph is written as a tree but is not one");
        if let Some(weights) = &self.weights {
            assert_eq!(weights.len(), self.edges.len(), "edges were added to the graph after its weights");
        }

        let first_label = usize::from(!self.zero_based);
        let write_line = |out: &mut String, values: &[usize]| {
            for (i, &value) in values.iter().enumerate() {
                if i > 0 {
                    out.push(' ');
                }
                value.write_output(out);
            }
            out.push('\n');
        };
        match self.format {
            GraphFormat::EdgeList => write_line(out, &[self.num_nodes(), self.num_edges()]),
            GraphFormat::Tree | GraphFormat::ParentList => write_line(out, &[self.num_nodes()]),
            GraphFormat::EdgesOnly => {}
        }

        if self.format == GraphFormat::ParentList {
            let parents = self.parents();
            if !parents.is_empty() {
                write_line(out, &parents.iter().map(|&(parent, _)| parent + first_label).collect::<Vec<_>>());
            }
            if let Some(weights) = &self.weights {
                parents.iter().map(|&(_, edge)| weights[edge]).collect::<Vec<_>>().write_output(out);
            }
            return;
        }

        // Shuffling the positions moves them exactly as it would the edges.
        let mut order: Vec<usize> = (0..self.edges.len()).collect();
        let mut rng = Rng::from_seed(self.output_seed);
        rng.shuffle(&mut order);
        for edge in order {
            let (u, v) = self.edges[edge];
            let (first, second) = if self.directed || rng.random_bool(0.5) { (u, v) } else { (v, u) };
            (first + first_label).write_output(out);
            out.push(' ');
            (second + first_label).write_output(out);
            if let Some(weights) = &self.weights {
                out.push(' ');
                weights[edge].write_output(out);
            }
            out.push('\n');
        }
    }
}
