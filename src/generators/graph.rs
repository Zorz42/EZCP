use std::collections::HashSet;
use std::fmt::Write;
use rand::prelude::SliceRandom;
use rand::RngExt;
use crate::ToOutput;

/// This struct represents a combinatorial undirected graph.
/// It is used to generate the input for test cases and to check the output of solutions.
pub struct Graph {
    nodes: Vec<Vec<usize>>,
    edges: HashSet<(usize, usize)>,
    /// if the graph should be tree, is only used when generating output:
    /// it checks if the graph is a tree and does not add edge count to the output,
    /// since it is equal to n-1
    pub is_tree: bool,
}

impl Graph {
    /// This function creates a new empty graph with `n` nodes and no edges.
    #[must_use]
    pub fn new_empty(n: i32) -> Self {
        Self {
            nodes: vec![Vec::new(); n as usize],
            edges: HashSet::new(),
            is_tree: false,
        }
    }

    /// This function creates a new full graph with `n` nodes and all possible edges.
    #[must_use]
    pub fn new_full(n: i32) -> Self {
        let mut result = Self::new_empty(n);
        for u in 0..n {
            for v in 0..u {
                result.add_edge(u as usize, v as usize);
            }
        }
        result
    }

    /// This function creates a new random graph with `n` nodes and `m` edges.
    /// The edges are chosen randomly.
    #[must_use]
    pub fn new_random(n: i32, m: i32) -> Self {
        let mut result = Self::new_empty(n);
        let mut rng = rand::rng();
        while result.get_num_edges() < m {
            let u = rng.random_range(0..n);
            let v = rng.random_range(0..n);
            result.add_edge(u as usize, v as usize);
        }
        result
    }

    /// This function creates a new random tree with `n` nodes and `n - 1` edges by the definition of a tree.
    /// The edges are chosen randomly.
    #[must_use]
    pub fn new_random_tree(n: i32) -> Self {
        let mut result = Self::new_empty(n);
        result.is_tree = true;
        let mut rng = rand::rng();
        let mut nodes = (0..n).collect::<Vec<_>>();
        nodes.shuffle(&mut rng);
        for i in 1..n {
            let u = nodes[i as usize];
            let v = nodes[rng.random_range(0..i) as usize];
            result.add_edge(u as usize, v as usize);
        }
        result
    }

    /// this creates a random tree that has O(n) depth
    #[must_use]
    pub fn new_random_deep_tree(n: i32) -> Self {
        let mut result = Self::new_empty(n);
        result.is_tree = true;
        let mut rng = rand::rng();
        let mut nodes = (0..n).collect::<Vec<_>>();
        nodes.shuffle(&mut rng);
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
    #[must_use]
    pub fn new_random_connected(n: i32, m: i32) -> Self {
        let mut result = Self::new_random_tree(n);
        result.is_tree = false;
        let mut rng = rand::rng();
        while result.get_num_edges() < m {
            let u = rng.random_range(0..n);
            let v = rng.random_range(0..n);
            result.add_edge(u as usize, v as usize);
        }
        result
    }

    /// This function creates a new random bipartite graph with `n` nodes and `m` edges.
    /// The edges are chosen randomly and the graph is guaranteed to be bipartite.
    #[must_use]
    pub fn new_random_bipartite(n: i32, m: i32) -> Self {
        let mut result = Self::new_empty(n);
        let mut rng = rand::rng();
        let mut nodes = (0..n).collect::<Vec<_>>();
        nodes.shuffle(&mut rng);
        let size1 = loop {
            let size1 = rng.random_range(1..n);
            if size1 * (n - size1) >= m {
                break size1;
            }
        };
        while result.get_num_edges() < m {
            let u = nodes[rng.random_range(0..size1) as usize];
            let v = nodes[rng.random_range(size1..n) as usize];
            result.add_edge(u as usize, v as usize);
        }
        result
    }

    /// This function returns true if there is an edge between nodes u and v.
    /// If u == v, this function will return false.
    /// Also for every pair of nodes `u`, `v`, the following holds: `has_edge(u, v) == has_edge(v, u)`
    #[must_use]
    pub fn has_edge(&self, u: usize, v: usize) -> bool {
        self.edges.contains(&(usize::max(u, v), usize::min(u, v)))
    }

    /// This function returns the count of edges between nodes u and v.
    #[must_use]
    pub fn get_num_edges(&self) -> i32 {
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
            self.edges.insert((usize::max(u, v), usize::min(u, v)));
            self.nodes[u].push(v);
            self.nodes[v].push(u);
        }
    }

    /// This function returns an iterator over the edges in the graph.
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
        self.get_num_edges() == self.get_num_nodes() * (self.get_num_nodes() - 1) / 2
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
}

impl ToOutput for Graph {
    /// This function converts the graph to an input string.
    /// The input string will be formatted as follows:
    /// The first line will contain two integers n and m, the number of nodes and edges respectively.
    /// The next m lines will contain two integers u and v, representing an edge between nodes u and v.
    /// The nodes are 1-indexed.
    /// The edges will be randomly shuffled and pair may be swapped.
    fn to_output(self) -> String {
        if self.is_tree {
            assert!(self.is_tree());
        }
        let mut result = String::new();
        if self.is_tree {
            result += &format!("{}\n", self.get_num_nodes());
        } else {
            result += &format!("{} {}\n", self.get_num_nodes(), self.get_num_edges());
        }
        let mut edges = self.edges_iter().collect::<Vec<_>>();
        let mut rng = rand::rng();
        edges.shuffle(&mut rng);
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
