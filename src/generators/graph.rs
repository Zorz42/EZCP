use rand::prelude::SliceRandom;
use rand::Rng;
use std::collections::HashSet;
use std::fmt::Write;

pub struct Graph {
    nodes: Vec<Vec<usize>>,
    edges: HashSet<(usize, usize)>,
}

impl Graph {
    pub fn new_empty(n: i32) -> Self {
        Self {
            nodes: vec![Vec::new(); n as usize],
            edges: HashSet::new(),
        }
    }

    pub fn new_full(n: i32) -> Self {
        let mut result = Self::new_empty(n);
        for u in 0..n {
            for v in 0..u {
                result.add_edge(u as usize, v as usize);
            }
        }
        result
    }

    pub fn has_edge(&self, u: usize, v: usize) -> bool {
        self.edges.contains(&(u, v))
    }

    pub fn get_num_edges(&self) -> i32 {
        self.edges.len() as i32 / 2
    }

    pub fn get_num_nodes(&self) -> i32 {
        self.nodes.len() as i32
    }

    pub fn add_edge(&mut self, u: usize, v: usize) {
        if !self.has_edge(u, v) {
            self.edges.insert((u, v));
            self.edges.insert((v, u));
            self.nodes[u].push(v);
            self.nodes[v].push(u);
        }
    }

    pub fn edges_iter(&self) -> impl Iterator<Item = &(usize, usize)> {
        self.edges.iter()
    }

    pub fn create_output(&self) -> String {
        let mut result = String::new();
        writeln!(result, "{} {}", self.get_num_nodes(), self.get_num_edges()).ok();
        let mut edges = self.edges_iter().collect::<Vec<_>>();
        let mut rng = rand::thread_rng();
        edges.shuffle(&mut rng);
        for (u, v) in edges {
            if rng.gen_bool(0.5) {
                writeln!(result, "{} {}", u + 1, v + 1).ok();
            } else {
                writeln!(result, "{} {}", v + 1, u + 1).ok();
            }
        }
        result
    }
}
