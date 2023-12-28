use crate::Input;
use anyhow::Result;
use rand::prelude::SliceRandom;
use rand::Rng;
use std::collections::HashSet;
use std::fmt::Write;

pub struct Graph {
    nodes: Vec<Vec<usize>>,
    edges: HashSet<(usize, usize)>,
}

impl Graph {
    #[must_use]
    pub fn new_empty(n: i32) -> Self {
        Self {
            nodes: vec![Vec::new(); n as usize],
            edges: HashSet::new(),
        }
    }

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

    #[must_use]
    pub fn new_random(n: i32, m: i32) -> Self {
        let mut result = Self::new_empty(n);
        let mut rng = rand::thread_rng();
        while result.get_num_edges() < m {
            let u = rng.gen_range(0..n);
            let v = rng.gen_range(0..n);
            result.add_edge(u as usize, v as usize);
        }
        result
    }

    #[must_use]
    pub fn new_random_tree(n: i32) -> Self {
        let mut result = Self::new_empty(n);
        let mut rng = rand::thread_rng();
        let mut nodes = (0..n).collect::<Vec<_>>();
        nodes.shuffle(&mut rng);
        for i in 1..n {
            let u = nodes[i as usize];
            let v = nodes[rng.gen_range(0..i) as usize];
            result.add_edge(u as usize, v as usize);
        }
        result
    }

    #[must_use]
    pub fn new_random_connected(n: i32, m: i32) -> Self {
        let mut result = Self::new_random_tree(n);
        let mut rng = rand::thread_rng();
        while result.get_num_edges() < m {
            let u = rng.gen_range(0..n);
            let v = rng.gen_range(0..n);
            result.add_edge(u as usize, v as usize);
        }
        result
    }

    #[must_use]
    pub fn new_random_bipartite(n: i32, m: i32) -> Self {
        let mut result = Self::new_empty(n);
        let mut rng = rand::thread_rng();
        let mut nodes = (0..n).collect::<Vec<_>>();
        nodes.shuffle(&mut rng);
        let size1 = loop {
            let size1 = rng.gen_range(1..n);
            if size1 * (n - size1) >= m {
                break size1;
            }
        };
        while result.get_num_edges() < m {
            let u = nodes[rng.gen_range(0..size1) as usize];
            let v = nodes[rng.gen_range(size1..n) as usize];
            result.add_edge(u as usize, v as usize);
        }
        result
    }

    pub fn new_from_input(input: &str) -> Result<Self> {
        let mut input = Input::new(input);
        let n = input.get_int()?;
        let m = input.get_int()?;
        let mut result = Self::new_empty(n);
        for _ in 0..m {
            let u = input.get_int()? - 1;
            let v = input.get_int()? - 1;
            result.add_edge(u as usize, v as usize);
        }
        Ok(result)
    }

    #[must_use]
    pub fn has_edge(&self, u: usize, v: usize) -> bool {
        self.edges.contains(&(u, v))
    }

    #[must_use]
    pub fn get_num_edges(&self) -> i32 {
        self.edges.len() as i32 / 2
    }

    #[must_use]
    pub fn get_num_nodes(&self) -> i32 {
        self.nodes.len() as i32
    }

    pub fn add_edge(&mut self, u: usize, v: usize) {
        if !self.has_edge(u, v) && u != v {
            self.edges.insert((u, v));
            self.edges.insert((v, u));
            self.nodes[u].push(v);
            self.nodes[v].push(u);
        }
    }

    pub fn edges_iter(&self) -> impl Iterator<Item = &(usize, usize)> {
        self.edges.iter()
    }

    #[must_use]
    pub fn create_output(&self) -> String {
        let mut result = String::new();
        writeln!(result, "{} {}", self.get_num_nodes(), self.get_num_edges()).ok();
        let mut edges = self.edges_iter().collect::<Vec<_>>();
        let mut rng = rand::thread_rng();
        edges.shuffle(&mut rng);
        for (u, v) in edges {
            if u > v {
                continue;
            }
            if rng.gen_bool(0.5) {
                writeln!(result, "{} {}", u + 1, v + 1).ok();
            } else {
                writeln!(result, "{} {}", v + 1, u + 1).ok();
            }
        }
        result
    }

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

    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.get_connected_components().len() == 1
    }

    #[must_use]
    pub fn is_tree(&self) -> bool {
        self.get_num_edges() == self.get_num_nodes() - 1 && self.is_connected()
    }

    #[must_use]
    pub fn is_full(&self) -> bool {
        self.get_num_edges() == self.get_num_nodes() * (self.get_num_nodes() - 1) / 2
    }

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
