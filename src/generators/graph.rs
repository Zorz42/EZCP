use crate::ToOutput;
use crate::rng::Rng;
use std::collections::HashSet;
use std::fmt::Write;

/// This struct represents a combinatorial undirected graph.
/// It is used to generate the input for test cases and to check the output of solutions.
///
/// Building a graph needs the generator's [`Rng`], even for the shapes that hold
/// no randomness: writing a graph out shuffles its edges, and the shuffle has to
/// come from the same seeded stream as everything else, so the seed is drawn up
/// front and kept.
pub struct Graph {
    nodes: Vec<Vec<usize>>,
    /// Edges in insertion order.
    ///
    /// A `HashSet` alone would be enough to model the graph, but iterating one
    /// yields a different order in every process, which would make a graph built
    /// from a fixed seed write itself out differently on every run.
    edges: Vec<(usize, usize)>,
    /// The same edges, for membership tests that stay O(1).
    edge_set: HashSet<(usize, usize)>,
    /// Seeds the edge shuffle in [`ToOutput::to_output`].
    output_seed: u64,
    /// if the graph should be tree, is only used when generating output:
    /// it checks if the graph is a tree and does not add edge count to the output,
    /// since it is equal to n-1
    pub is_tree: bool,
}

/// The number of edges a simple undirected graph on `n` nodes can hold.
///
/// Computed in 64 bits: `n * (n - 1) / 2` overflows an `i32` from n = 65537 on.
const fn max_simple_edges(n: i32) -> i64 {
    let n = n as i64;
    n * (n - 1) / 2
}

/// The number of edges the best possible bipartition of `n` nodes can hold.
const fn max_bipartite_edges(n: i32) -> i64 {
    let n = n as i64;
    (n / 2) * ((n + 1) / 2)
}

impl Graph {
    /// This function creates a new empty graph with `n` nodes and no edges.
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

    /// This function creates a new full graph with `n` nodes and all possible edges.
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

    /// This function creates a new random graph with `n` nodes and `m` edges.
    /// The edges are chosen randomly.
    ///
    /// # Panics
    /// Panics if `m` edges do not fit in a simple graph on `n` nodes, which would
    /// otherwise leave the generator looking for an edge that cannot exist.
    #[must_use]
    pub fn new_random(rng: &mut Rng, n: i32, m: i32) -> Self {
        let mut result = Self::new_empty(rng, n);
        assert!(m >= 0, "a graph cannot have {m} edges");
        assert!(
            i64::from(m) <= max_simple_edges(n),
            "cannot fit {m} edges in a graph with {n} nodes (at most {} are possible)",
            max_simple_edges(n)
        );
        result.add_random_edges(rng, m);
        result
    }

    /// Adds random edges until the graph holds `m` of them.
    ///
    /// Guessing pairs is fine while the graph is sparse, but once most of the
    /// possible edges are already taken almost every guess is a repeat, and the
    /// last few edges would take unboundedly many attempts to find. Past that
    /// point the candidates are enumerated and shuffled instead, which costs a
    /// number of pairs comparable to `m` itself.
    fn add_random_edges(&mut self, rng: &mut Rng, m: i32) {
        let n = self.get_num_nodes();

        // Half of the possible edges is where guessing still succeeds every other
        // attempt on average; beyond it the expected number of guesses grows
        // without bound.
        if i64::from(m) * 2 <= max_simple_edges(n) {
            while self.get_num_edges() < m {
                let u = rng.random_range(0..n);
                let v = rng.random_range(0..n);
                self.add_edge(u as usize, v as usize);
            }
            return;
        }

        let mut candidates = Vec::new();
        for u in 0..n as usize {
            for v in 0..u {
                if !self.has_edge(u, v) {
                    candidates.push((u, v));
                }
            }
        }
        self.add_edges_from(rng, candidates, m);
    }

    /// Adds edges drawn from `candidates`, in random order, until the graph holds
    /// `m` of them.
    ///
    /// This is what the generators fall back to once so many of the possible
    /// edges are taken that guessing pairs mostly finds repeats.
    fn add_edges_from(&mut self, rng: &mut Rng, mut candidates: Vec<(usize, usize)>, m: i32) {
        rng.shuffle(&mut candidates);

        for (u, v) in candidates {
            if self.get_num_edges() >= m {
                break;
            }
            self.add_edge(u, v);
        }
    }

    /// This function create a new random path (that is also a tree)
    ///
    /// # Panics
    /// Panics if `n` is not positive; a tree needs at least one node.
    #[must_use]
    pub fn new_random_path(rng: &mut Rng, n: i32) -> Self {
        assert!(n >= 1, "a tree needs at least one node, got {n}");
        let mut result = Self::new_empty(rng, n);
        result.is_tree = true;
        let mut nodes = (0..n).collect::<Vec<_>>();
        rng.shuffle(&mut nodes);
        for i in 1..n {
            let u = nodes[(i - 1) as usize];
            let v = nodes[i as usize];
            result.add_edge(u as usize, v as usize);
        }
        result
    }

    /// This function creates a new random tree with `n` nodes and `n - 1` edges by the definition of a tree.
    /// The edges are chosen randomly.
    ///
    /// # Panics
    /// Panics if `n` is not positive; a tree needs at least one node.
    #[must_use]
    pub fn new_random_tree(rng: &mut Rng, n: i32) -> Self {
        assert!(n >= 1, "a tree needs at least one node, got {n}");
        let mut result = Self::new_empty(rng, n);
        result.is_tree = true;
        let mut nodes = (0..n).collect::<Vec<_>>();
        rng.shuffle(&mut nodes);
        for i in 1..n {
            let u = nodes[i as usize];
            let v = nodes[rng.random_range(0..i) as usize];
            result.add_edge(u as usize, v as usize);
        }
        result
    }

    /// this creates a random tree that has O(n) depth
    ///
    /// # Panics
    /// Panics if `n` is not positive; a tree needs at least one node.
    #[must_use]
    pub fn new_random_deep_tree(rng: &mut Rng, n: i32) -> Self {
        assert!(n >= 1, "a tree needs at least one node, got {n}");
        let mut result = Self::new_empty(rng, n);
        result.is_tree = true;
        let mut nodes = (0..n).collect::<Vec<_>>();
        rng.shuffle(&mut nodes);
        let mut last_on_chain = nodes[0];
        for i in 1..n {
            let u = nodes[i as usize];
            let v = if rng.random_bool(0.5) {
                let res = last_on_chain;
                last_on_chain = u;
                res
            } else {
                nodes[rng.random_range(0..i) as usize]
            };
            result.add_edge(u as usize, v as usize);
        }
        result
    }

    /// This function creates a new random connected graph with `n` nodes and `m` edges.
    /// The edges are chosen randomly and the graph is guaranteed to be connected.
    /// If m <= n - 1, the graph will be a tree.
    ///
    /// # Panics
    /// Panics if `n` is not positive, or if `m` edges do not fit in a simple graph
    /// on `n` nodes.
    #[must_use]
    pub fn new_random_connected(rng: &mut Rng, n: i32, m: i32) -> Self {
        assert!(
            i64::from(m) <= max_simple_edges(n),
            "cannot fit {m} edges in a graph with {n} nodes (at most {} are possible)",
            max_simple_edges(n)
        );
        let mut result = Self::new_random_tree(rng, n);
        result.is_tree = false;
        result.add_random_edges(rng, m);
        result
    }

    /// This function creates a new random bipartite graph with `n` nodes and `m` edges.
    /// The edges are chosen randomly and the graph is guaranteed to be bipartite.
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

        // `size1 * (n - size1)` is concave in `size1`, so the splits that are big
        // enough for `m` edges form one interval, symmetric around n / 2. Pick from
        // that interval directly instead of guessing until a guess fits.
        let mut smallest_side = 1_i64;
        while smallest_side * (i64::from(n) - smallest_side) < i64::from(m) {
            smallest_side += 1;
        }
        let size1 = rng.random_range(smallest_side..=i64::from(n) - smallest_side) as i32;

        // Guessing pairs is only cheap while a good half of the pairs across the
        // partition are still free; past that point the last few edges take
        // unboundedly many guesses to find, exactly as in `add_random_edges`.
        if i64::from(m) * 2 <= i64::from(size1) * i64::from(n - size1) {
            while result.get_num_edges() < m {
                let u = nodes[rng.random_range(0..size1) as usize];
                let v = nodes[rng.random_range(size1..n) as usize];
                result.add_edge(u as usize, v as usize);
            }
            return result;
        }

        let mut candidates = Vec::new();
        for &u in &nodes[..size1 as usize] {
            for &v in &nodes[size1 as usize..] {
                candidates.push((u as usize, v as usize));
            }
        }
        result.add_edges_from(rng, candidates, m);
        result
    }

    /// This function returns true if there is an edge between nodes u and v.
    /// If u == v, this function will return false.
    /// Also for every pair of nodes `u`, `v`, the following holds: `has_edge(u, v) == has_edge(v, u)`
    #[must_use]
    pub fn has_edge(&self, u: usize, v: usize) -> bool {
        self.edge_set.contains(&(usize::max(u, v), usize::min(u, v)))
    }

    /// This function returns the count of edges between nodes u and v.
    #[must_use]
    pub const fn get_num_edges(&self) -> i32 {
        self.edges.len() as i32
    }

    /// This function returns the count of nodes in the graph.
    #[must_use]
    pub const fn get_num_nodes(&self) -> i32 {
        self.nodes.len() as i32
    }

    /// This function adds an edge between nodes u and v.
    pub fn add_edge(&mut self, u: usize, v: usize) {
        if !self.has_edge(u, v) && u != v {
            let edge = (usize::max(u, v), usize::min(u, v));
            self.edge_set.insert(edge);
            self.edges.push(edge);
            self.nodes[u].push(v);
            self.nodes[v].push(u);
        }
    }

    /// This function returns an iterator over the edges in the graph.
    ///
    /// The edges come back in the order they were added, which is what keeps a
    /// graph built from a given seed identical from run to run.
    pub fn edges_iter(&self) -> impl Iterator<Item = &(usize, usize)> {
        self.edges.iter()
    }

    /// This function returns the array of connected components in the graph.
    /// Each connected component is represented by an array of node indices.
    /// The nodes are 0-indexed and the arrays are in undefined order.
    #[must_use]
    pub fn get_connected_components(&self) -> Vec<Vec<usize>> {
        let mut result = Vec::new();
        let mut visited = vec![false; self.get_num_nodes() as usize];
        for i in 0..self.get_num_nodes() {
            if !visited[i as usize] {
                let mut component = Vec::new();
                let mut queue = vec![i as usize];
                visited[i as usize] = true;
                while let Some(u) = queue.pop() {
                    component.push(u);
                    for &v in &self.nodes[u] {
                        if !visited[v] {
                            visited[v] = true;
                            queue.push(v);
                        }
                    }
                }
                result.push(component);
            }
        }
        result
    }

    /// This function returns true if the graph is connected.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.get_connected_components().len() == 1
    }

    /// This function returns true if the graph is a tree.
    #[must_use]
    pub fn is_tree(&self) -> bool {
        self.get_num_edges() == self.get_num_nodes() - 1 && self.is_connected()
    }

    /// This function returns true if the graph has every possible edge.
    #[must_use]
    pub fn is_full(&self) -> bool {
        // In 64 bits: the node count squared overflows an i32 from n = 65537 on.
        i64::from(self.get_num_edges()) == max_simple_edges(self.get_num_nodes())
    }

    /// This function returns true if the graph is bipartite.
    #[must_use]
    pub fn is_bipartite(&self) -> bool {
        let mut visited = vec![false; self.get_num_nodes() as usize];
        let mut colors = vec![0; self.get_num_nodes() as usize];
        let mut queue = Vec::new();
        for i in 0..self.get_num_nodes() {
            if !visited[i as usize] {
                queue.push(i as usize);
                visited[i as usize] = true;
                colors[i as usize] = 1;
                while let Some(u) = queue.pop() {
                    for &v in &self.nodes[u] {
                        if !visited[v] {
                            visited[v] = true;
                            colors[v] = -colors[u];
                            queue.push(v);
                        } else if colors[v] == colors[u] {
                            return false;
                        }
                    }
                }
            }
        }
        true
    }

    /// Returns the nodes adjacent to `node`.
    ///
    /// # Panics
    ///
    /// Panics if `node` is not a node of this graph.
    #[must_use]
    pub fn get_neighbours(&self, node: usize) -> Vec<usize> {
        self.nodes[node].clone()
    }
}

impl ToOutput for Graph {
    /// This function converts the graph to an input string.
    /// The input string will be formatted as follows:
    /// The first line will contain two integers n and m, the number of nodes and edges respectively.
    /// The next m lines will contain two integers u and v, representing an edge between nodes u and v.
    /// The nodes are 1-indexed.
    /// The edges will be randomly shuffled and pair may be swapped.
    ///
    /// The shuffle runs off the seed the graph was built with, so writing the same
    /// graph out twice gives the same text.
    ///
    /// # Panics
    /// Panics if the graph was built as a tree but is not one, which would make
    /// the omitted edge count wrong.
    fn to_output(self) -> String {
        if self.is_tree {
            assert!(self.is_tree());
        }
        let mut result = String::new();
        if self.is_tree {
            writeln!(result, "{}", self.get_num_nodes()).ok();
        } else {
            writeln!(result, "{} {}", self.get_num_nodes(), self.get_num_edges()).ok();
        }
        let mut edges = self.edges_iter().collect::<Vec<_>>();
        let mut rng = Rng::from_seed(self.output_seed);
        rng.shuffle(&mut edges);
        for (u, v) in edges {
            if rng.random_bool(0.5) {
                writeln!(result, "{} {}", u + 1, v + 1).ok();
            } else {
                writeln!(result, "{} {}", v + 1, u + 1).ok();
            }
        }
        result
    }
}
