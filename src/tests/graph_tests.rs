#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod graph_tests {
    use crate::ToOutput;
    use crate::generators::Graph;
    use crate::rng::Rng;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// A different seed on every call.
    fn rng() -> Rng {
        static NEXT_SEED: AtomicU64 = AtomicU64::new(0);
        Rng::from_seed(NEXT_SEED.fetch_add(1, Ordering::Relaxed))
    }

    /// Adds `edges` to an empty graph one at a time, recording `check` after each.
    fn after_each_edge<T>(n: i32, edges: &[(usize, usize)], check: impl Fn(&Graph) -> T) -> Vec<T> {
        let mut graph = Graph::new_empty(&mut rng(), n);
        edges
            .iter()
            .map(|&(u, v)| {
                graph.add_edge(u, v);
                check(&graph)
            })
            .collect()
    }

    /// The edges after the header line, as sorted 0-based `(min, max)` pairs.
    fn parse_edges(output: &str) -> Vec<(usize, usize)> {
        let mut edges: Vec<_> = output
            .lines()
            .skip(1)
            .map(|line| {
                let parts: Vec<usize> = line.split_whitespace().map(|s| s.parse().unwrap()).collect();
                assert_eq!(parts.len(), 2, "each edge line must have exactly 2 values");
                (parts[0].min(parts[1]) - 1, parts[0].max(parts[1]) - 1)
            })
            .collect();
        edges.sort_unstable();
        edges
    }

    fn cycle(n: usize) -> Vec<(usize, usize)> {
        (0..n).map(|u| (u, (u + 1) % n)).collect()
    }

    #[test]
    fn test_empty_and_full() {
        for n in 1..100 {
            let (empty, full) = (Graph::new_empty(&mut rng(), n), Graph::new_full(&mut rng(), n));
            assert_eq!((empty.get_num_nodes(), empty.get_num_edges()), (n, 0));
            assert_eq!((full.get_num_nodes(), full.get_num_edges()), (n, n * (n - 1) / 2));
            for u in 0..n as usize {
                for v in 0..n as usize {
                    assert!(!empty.has_edge(u, v));
                    assert_eq!(full.has_edge(u, v), u != v);
                }
            }
            assert!(full.is_full());
            assert_eq!(empty.is_full(), n == 1);
        }
    }

    #[test]
    fn test_add_edge() {
        let edges = [(0, 1), (1, 2), (0, 3)];
        assert_eq!(after_each_edge(5, &edges, Graph::get_num_edges), vec![1, 2, 3]);

        let mut graph = Graph::new_empty(&mut rng(), 5);
        for (u, v) in edges {
            graph.add_edge(u, v);
        }
        graph.add_edge(1, 0);
        graph.add_edge(4, 4);
        assert_eq!(graph.get_num_edges(), 3, "duplicates and loops are not added");
        for u in 0..5 {
            for v in 0..5 {
                assert_eq!(graph.has_edge(u, v), edges.contains(&(u, v)) || edges.contains(&(v, u)), "{u} {v}");
            }
        }
    }

    #[test]
    fn test_random() {
        for n in 5..100 {
            for m in [n, 2 * n] {
                let graph = Graph::new_random(&mut rng(), n, m);
                assert_eq!((graph.get_num_nodes(), graph.get_num_edges()), (n, m));
                let adjacent = (0..n as usize).flat_map(|u| (0..n as usize).map(move |v| (u, v))).filter(|&(u, v)| graph.has_edge(u, v));
                assert_eq!(adjacent.count(), 2 * m as usize);
            }
        }
    }

    #[test]
    fn test_random_dense() {
        for n in 2..80 {
            let max_edges = n * (n - 1) / 2;
            for m in [max_edges, max_edges - 1, max_edges * 3 / 4] {
                assert_eq!(Graph::new_random(&mut rng(), n, m).get_num_edges(), m, "for {n} nodes and {m} edges");
            }
            let connected = Graph::new_random_connected(&mut rng(), n, max_edges);
            assert!(connected.is_connected() && connected.is_full());
        }
        assert!(Graph::new_random(&mut rng(), 200, 200 * 199 / 2).is_full());
    }

    #[test]
    fn test_random_trees_and_connected_graphs() {
        for n in 1..100 {
            for tree in [
                Graph::new_random_tree(&mut rng(), n),
                Graph::new_random_path(&mut rng(), n),
                Graph::new_random_deep_tree(&mut rng(), n),
                Graph::new_random_connected(&mut rng(), n, n - 1),
            ] {
                assert_eq!((tree.get_num_nodes(), tree.get_num_edges()), (n, n - 1));
                assert!(tree.is_tree());
            }
        }
        for n in 2..100 {
            assert!(Graph::new_random_connected(&mut rng(), n, n - 2).is_tree());
            assert!(!Graph::new_random(&mut rng(), n, n - 2).is_tree());
        }
        for n in 3..100 {
            assert!(!Graph::new_full(&mut rng(), n).is_tree());
            assert!(!Graph::new_random(&mut rng(), n, n).is_tree());
        }
        let connected = Graph::new_random_connected(&mut rng(), 10, 20);
        assert_eq!((connected.get_num_nodes(), connected.get_num_edges()), (10, 20));
        assert!(connected.is_connected());
    }

    #[test]
    fn test_is_tree() {
        let mut path_then_cycle = vec![false; 8];
        path_then_cycle.extend([true, false]);
        assert_eq!(after_each_edge(10, &cycle(10), Graph::is_tree), path_then_cycle);
        assert_eq!(after_each_edge(5, &[(0, 1), (0, 2), (2, 3), (2, 4), (4, 0)], Graph::is_tree), vec![false, false, false, true, false]);
        assert_eq!(after_each_edge(4, &cycle(3), Graph::is_tree), vec![false; 3]);
    }

    #[test]
    fn test_random_bipartite() {
        for n in 10..100 {
            for m in [n / 2, n, 2 * n, 3 * n].into_iter().filter(|&m| i64::from(m) <= i64::from(n / 2) * i64::from((n + 1) / 2)) {
                let graph = Graph::new_random_bipartite(&mut rng(), n, m);
                assert_eq!((graph.get_num_nodes(), graph.get_num_edges()), (n, m));
                assert!(graph.is_bipartite());
            }
        }
    }

    #[test]
    fn test_is_bipartite() {
        let mut even_cycle_then_chord = cycle(10);
        even_cycle_then_chord.push((0, 2));
        let mut expected = vec![true; 10];
        expected.push(false);
        assert_eq!(after_each_edge(10, &even_cycle_then_chord, Graph::is_bipartite), expected);
        assert_eq!(after_each_edge(5, &[(0, 1), (0, 2), (2, 3), (2, 4), (4, 0)], Graph::is_bipartite), vec![true, true, true, true, false]);
        assert_eq!(after_each_edge(4, &cycle(3), Graph::is_bipartite), vec![true, true, false]);
        assert!(Graph::new_empty(&mut rng(), 3).is_bipartite());
    }

    #[test]
    fn test_connected_components() {
        assert_eq!(Graph::new_empty(&mut rng(), 3).get_connected_components(), vec![vec![0], vec![1], vec![2]]);
        assert!(Graph::new_empty(&mut rng(), 1).is_connected());
        assert!(!Graph::new_empty(&mut rng(), 2).is_connected());

        let edges = [(0, 1), (1, 2), (2, 3), (3, 4), (0, 5), (3, 0), (6, 7), (7, 8), (8, 9), (0, 6), (9, 0)];
        let counts = after_each_edge(10, &edges, |graph| graph.get_connected_components().len());
        assert_eq!(counts, vec![9, 8, 7, 6, 5, 5, 4, 3, 2, 1, 1]);
        let mut component = after_each_edge(10, &edges, Graph::get_connected_components).pop().unwrap().remove(0);
        component.sort_unstable();
        assert_eq!(component, (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn test_to_output() {
        assert_eq!(Graph::new_empty(&mut rng(), 5).to_output(), "5 0\n");
        assert_eq!(Graph::new_empty(&mut rng(), 1).to_output(), "1 0\n");

        let mut path = Graph::new_empty(&mut rng(), 4);
        for u in 0..3 {
            path.add_edge(u, u + 1);
        }
        path.is_tree = true;
        let output = path.to_output();
        assert_eq!(output.lines().next(), Some("4"), "a tree is written without its edge count");
        assert_eq!(parse_edges(&output), vec![(0, 1), (1, 2), (2, 3)]);

        let output = Graph::new_full(&mut rng(), 4).to_output();
        assert_eq!(output.lines().next(), Some("4 6"));
        assert_eq!(parse_edges(&output), vec![(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)]);
    }

    #[test]
    fn test_to_output_of_random_graphs() {
        for n in 5..30 {
            let output = Graph::new_random(&mut rng(), n, 2 * n).to_output();
            assert_eq!(output.lines().next().unwrap(), format!("{n} {}", 2 * n));
            let edges = parse_edges(&output);
            assert_eq!(edges.len(), 2 * n as usize);
            assert!(edges.iter().all(|&(u, v)| u < v && v < n as usize), "{edges:?}");

            let tree = Graph::new_random_tree(&mut rng(), n).to_output();
            assert_eq!(tree.lines().next().unwrap(), n.to_string());
            assert_eq!(tree.lines().count(), n as usize);
        }
    }

    #[test]
    #[should_panic(expected = "at most 3 are possible")]
    fn test_random_rejects_too_many_edges() {
        let _ = Graph::new_random(&mut rng(), 3, 10);
    }

    #[test]
    #[should_panic(expected = "at most 0 are possible")]
    fn test_random_rejects_edge_on_single_node() {
        let _ = Graph::new_random(&mut rng(), 1, 1);
    }

    #[test]
    #[should_panic(expected = "at most 6 are possible")]
    fn test_random_connected_rejects_too_many_edges() {
        let _ = Graph::new_random_connected(&mut rng(), 4, 100);
    }

    #[test]
    #[should_panic(expected = "at most 4 are possible")]
    fn test_random_bipartite_rejects_too_many_edges() {
        let _ = Graph::new_random_bipartite(&mut rng(), 4, 100);
    }

    #[test]
    #[should_panic(expected = "at least two nodes")]
    fn test_random_bipartite_rejects_single_node() {
        let _ = Graph::new_random_bipartite(&mut rng(), 1, 0);
    }

    #[test]
    #[should_panic(expected = "at least one node")]
    fn test_random_deep_tree_rejects_zero_nodes() {
        let _ = Graph::new_random_deep_tree(&mut rng(), 0);
    }

    #[test]
    #[should_panic(expected = "at least one node")]
    fn test_random_tree_rejects_zero_nodes() {
        let _ = Graph::new_random_tree(&mut rng(), 0);
    }

    #[test]
    #[should_panic(expected = "cannot have -1 nodes")]
    fn test_new_empty_rejects_negative_nodes() {
        let _ = Graph::new_empty(&mut rng(), -1);
    }

    #[test]
    #[should_panic(expected = "marked as a tree")]
    fn test_to_output_rejects_a_false_tree() {
        let mut graph = Graph::new_empty(&mut rng(), 3);
        graph.is_tree = true;
        let _ = graph.to_output();
    }

    #[test]
    fn test_maximum_density_graphs_terminate() {
        for n in 2..12 {
            assert!(Graph::new_random(&mut rng(), n, n * (n - 1) / 2).is_full());
            assert!(Graph::new_random_connected(&mut rng(), n, n * (n - 1) / 2).is_full());
            let max_bipartite = (n / 2) * ((n + 1) / 2);
            let bipartite = Graph::new_random_bipartite(&mut rng(), n, max_bipartite);
            assert_eq!(bipartite.get_num_edges(), max_bipartite);
            assert!(bipartite.is_bipartite());
        }

        let bipartite = Graph::new_random_bipartite(&mut rng(), 400, 200 * 200);
        assert_eq!(bipartite.get_num_edges(), 200 * 200);
        assert!(bipartite.is_bipartite() && bipartite.is_connected());
    }

    #[test]
    fn test_is_full_does_not_overflow_for_large_node_counts() {
        assert!(!Graph::new_empty(&mut rng(), 100_000).is_full());
    }

    #[test]
    fn a_graph_is_written_out_the_same_way_for_the_same_seed() {
        let builders: [fn(&mut Rng) -> Graph; 8] = [
            |rng| Graph::new_random_tree(rng, 60),
            |rng| Graph::new_random_path(rng, 60),
            |rng| Graph::new_random_deep_tree(rng, 60),
            |rng| Graph::new_full(rng, 60),
            |rng| Graph::new_random(rng, 50, 200),
            |rng| Graph::new_random_bipartite(rng, 40, 300),
            |rng| Graph::new_random_connected(rng, 40, 100),
            // Same edges, so only the output shuffle can differ.
            |rng| {
                let mut graph = Graph::new_empty(rng, 30);
                for u in 1..30 {
                    graph.add_edge(u, u / 2);
                }
                graph
            },
        ];
        for build in builders {
            let rendered = |seed| build(&mut Rng::from_seed(seed)).to_output();
            assert_eq!(rendered(11), rendered(11), "the same seed produced two different graphs");
            assert_ne!(rendered(11), rendered(12), "two seeds produced the same graph");
        }
    }
}
