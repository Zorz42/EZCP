#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod graph_tests {
    use crate::ToOutput;
    use crate::generators::{Graph, GraphFormat};
    use crate::rng::Rng;
    use crate::stub::stable_hash;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// A different seed on every call.
    fn rng() -> Rng {
        static NEXT_SEED: AtomicU64 = AtomicU64::new(0);
        Rng::from_seed(NEXT_SEED.fetch_add(1, Ordering::Relaxed))
    }

    /// Adds `edges` to an empty graph one at a time, recording `check` after each.
    fn after_each_edge<T>(n: usize, edges: &[(usize, usize)], check: impl Fn(&Graph) -> T) -> Vec<T> {
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
            assert_eq!((empty.num_nodes(), empty.num_edges()), (n, 0));
            assert_eq!((full.num_nodes(), full.num_edges()), (n, n * (n - 1) / 2));
            for u in 0..n {
                for v in 0..n {
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
        assert_eq!(after_each_edge(5, &edges, Graph::num_edges), vec![1, 2, 3]);

        let mut graph = Graph::new_empty(&mut rng(), 5);
        for (u, v) in edges {
            graph.add_edge(u, v);
        }
        graph.add_edge(1, 0);
        graph.add_edge(4, 4);
        assert_eq!(graph.num_edges(), 3, "duplicates and loops are not added");
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
                assert_eq!((graph.num_nodes(), graph.num_edges()), (n, m));
                let adjacent = (0..n).flat_map(|u| (0..n).map(move |v| (u, v))).filter(|&(u, v)| graph.has_edge(u, v));
                assert_eq!(adjacent.count(), 2 * m);
            }
        }
    }

    #[test]
    fn test_random_dense() {
        for n in 2..80 {
            let max_edges = n * (n - 1) / 2;
            for m in [max_edges, max_edges - 1, max_edges * 3 / 4] {
                assert_eq!(Graph::new_random(&mut rng(), n, m).num_edges(), m, "for {n} nodes and {m} edges");
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
                assert_eq!((tree.num_nodes(), tree.num_edges()), (n, n - 1));
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
        assert_eq!((connected.num_nodes(), connected.num_edges()), (10, 20));
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
        for n in 10_usize..100 {
            for m in [n / 2, n, 2 * n, 3 * n].into_iter().filter(|&m| m <= (n / 2) * n.div_ceil(2)) {
                let graph = Graph::new_random_bipartite(&mut rng(), n, m);
                assert_eq!((graph.num_nodes(), graph.num_edges()), (n, m));
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
        assert_eq!(Graph::new_empty(&mut rng(), 3).connected_components(), vec![vec![0], vec![1], vec![2]]);
        assert!(Graph::new_empty(&mut rng(), 1).is_connected());
        assert!(!Graph::new_empty(&mut rng(), 2).is_connected());

        let edges = [(0, 1), (1, 2), (2, 3), (3, 4), (0, 5), (3, 0), (6, 7), (7, 8), (8, 9), (0, 6), (9, 0)];
        let counts = after_each_edge(10, &edges, |graph| graph.connected_components().len());
        assert_eq!(counts, vec![9, 8, 7, 6, 5, 5, 4, 3, 2, 1, 1]);
        let mut component = after_each_edge(10, &edges, Graph::connected_components).pop().unwrap().remove(0);
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
        let output = path.with_format(GraphFormat::Tree).to_output();
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
            assert_eq!(edges.len(), 2 * n);
            assert!(edges.iter().all(|&(u, v)| u < v && v < n), "{edges:?}");

            let tree = Graph::new_random_tree(&mut rng(), n).to_output();
            assert_eq!(tree.lines().next().unwrap(), n.to_string());
            assert_eq!(tree.lines().count(), n);
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
    #[should_panic(expected = "written as a tree")]
    fn test_to_output_rejects_a_false_tree() {
        let _ = Graph::new_empty(&mut rng(), 3).with_format(GraphFormat::Tree).to_output();
    }

    #[test]
    #[should_panic(expected = "cannot join nodes 1 and 5")]
    fn test_add_edge_rejects_a_missing_node() {
        Graph::new_empty(&mut rng(), 5).add_edge(1, 5);
    }

    #[test]
    fn test_maximum_density_graphs_terminate() {
        for n in 2..12 {
            assert!(Graph::new_random(&mut rng(), n, n * (n - 1) / 2).is_full());
            assert!(Graph::new_random_connected(&mut rng(), n, n * (n - 1) / 2).is_full());
            let max_bipartite = (n / 2) * n.div_ceil(2);
            let bipartite = Graph::new_random_bipartite(&mut rng(), n, max_bipartite);
            assert_eq!(bipartite.num_edges(), max_bipartite);
            assert!(bipartite.is_bipartite());
        }

        let bipartite = Graph::new_random_bipartite(&mut rng(), 400, 200 * 200);
        assert_eq!(bipartite.num_edges(), 200 * 200);
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

    /// How many nodes have each degree, by degree.
    fn degree_counts(graph: &Graph) -> Vec<usize> {
        let mut counts = vec![0; graph.num_nodes()];
        for node in 0..graph.num_nodes() {
            counts[graph.degree(node)] += 1;
        }
        counts
    }

    /// The number of edges on the longest path from `root`.
    fn height(graph: &Graph, root: usize) -> usize {
        let mut depth = vec![usize::MAX; graph.num_nodes()];
        depth[root] = 0;
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            for &neighbour in graph.neighbours(node) {
                if depth[neighbour] == usize::MAX {
                    depth[neighbour] = depth[node] + 1;
                    stack.push(neighbour);
                }
            }
        }
        depth.into_iter().max().unwrap()
    }

    #[test]
    fn test_accessors() {
        let mut graph = Graph::new_empty(&mut rng(), 4);
        graph.add_edge(0, 2);
        graph.add_edge(3, 2);
        assert_eq!(graph.edges(), [(2, 0), (3, 2)], "an undirected edge is stored with its larger end first");
        assert_eq!(graph.neighbours(2), [0, 3]);
        assert_eq!((0..4).map(|node| graph.degree(node)).collect::<Vec<_>>(), [1, 0, 2, 1]);
        assert!(!graph.is_directed());
        assert_eq!(graph.weights(), None);
    }

    #[test]
    fn test_tree_shapes() {
        for n in 1..60 {
            let star = Graph::new_star(&mut rng(), n);
            assert!(star.is_tree());
            assert!(n < 3 || (0..n).any(|node| star.degree(node) == n - 1), "a star has a centre");

            let binary = Graph::new_binary_tree(&mut rng(), n);
            assert!(binary.is_tree());
            assert!((0..n).all(|node| binary.degree(node) <= 3));
            let root = (0..n).find(|&node| binary.degree(node) <= 2).unwrap();
            assert!((0..n).any(|root| height(&binary, root) <= n.ilog2() as usize), "{n} nodes, from {root}");

            for spine in [1, n.div_ceil(2), n] {
                let caterpillar = Graph::new_caterpillar(&mut rng(), n, spine);
                assert!(caterpillar.is_tree());
                let inner = (0..n).filter(|&node| caterpillar.degree(node) > 1).count();
                assert!(inner <= spine, "{inner} inner nodes with a spine of {spine}");
            }
            assert_eq!(degree_counts(&Graph::new_caterpillar(&mut rng(), n, n)), degree_counts(&Graph::new_random_path(&mut rng(), n)));

            assert!(Graph::new_random_uniform_tree(&mut rng(), n).is_tree());
        }
    }

    /// There are 16 trees on 4 labelled nodes, and each has to come up as often
    /// as any other.
    #[test]
    fn test_uniform_trees_are_uniform() {
        let mut counts = std::collections::HashMap::new();
        for seed in 0..16_000 {
            let tree = Graph::new_random_uniform_tree(&mut Rng::from_seed(seed), 4);
            let mut edges = tree.edges().to_vec();
            edges.sort_unstable();
            *counts.entry(edges).or_insert(0) += 1;
        }
        assert_eq!(counts.len(), 16);
        assert!(counts.values().all(|count| (800..1200).contains(count)), "{counts:?}");
    }

    #[test]
    fn test_uniform_trees_are_deeper_than_recursive_ones() {
        let average_height = |build: fn(&mut Rng, usize) -> Graph| (0..20).map(|_| height(&build(&mut rng(), 2000), 0)).sum::<usize>() / 20;
        let (uniform, recursive) = (average_height(Graph::new_random_uniform_tree), average_height(Graph::new_random_tree));
        assert!(uniform > 2 * recursive, "uniform trees were {uniform} deep, recursive ones {recursive}");
    }

    #[test]
    fn test_forest() {
        for n in 1_usize..40 {
            for trees in [1, n.div_ceil(2), n] {
                let forest = Graph::new_random_forest(&mut rng(), n, trees);
                assert_eq!((forest.num_edges(), forest.connected_components().len()), (n - trees, trees));
            }
        }
    }

    #[test]
    fn test_cycle_and_grid() {
        for n in 3..40 {
            let cycle = Graph::new_cycle(&mut rng(), n);
            assert!(cycle.is_connected() && cycle.num_edges() == n && (0..n).all(|node| cycle.degree(node) == 2));
        }
        for (rows, columns) in [(1, 1), (1, 7), (5, 1), (4, 6), (0, 3)] {
            let grid = Graph::new_grid(&mut rng(), rows, columns);
            assert_eq!(grid.num_nodes(), rows * columns);
            assert_eq!(grid.num_edges(), rows * columns.saturating_sub(1) + columns * rows.saturating_sub(1));
            assert!(grid.is_bipartite());
            assert_eq!(grid.is_connected(), rows * columns > 0);
        }
        let grid = Graph::new_grid(&mut rng(), 3, 4);
        assert!(grid.has_edge(0, 1) && grid.has_edge(0, 4) && !grid.has_edge(3, 4) && grid.has_edge(7, 11));
    }

    /// Whether the edges can all be made to lead from an earlier node to a later one.
    fn is_acyclic(graph: &Graph) -> bool {
        let mut incoming = vec![0; graph.num_nodes()];
        for &(_, to) in graph.edges() {
            incoming[to] += 1;
        }
        let mut ready: Vec<usize> = (0..graph.num_nodes()).filter(|&node| incoming[node] == 0).collect();
        let mut ordered = 0;
        while let Some(node) = ready.pop() {
            ordered += 1;
            for &next in graph.neighbours(node) {
                incoming[next] -= 1;
                if incoming[next] == 0 {
                    ready.push(next);
                }
            }
        }
        ordered == graph.num_nodes()
    }

    #[test]
    fn test_directed_graphs() {
        let mut graph = Graph::new_empty_directed(&mut rng(), 3);
        graph.add_edge(0, 1);
        graph.add_edge(0, 1);
        graph.add_edge(1, 0);
        graph.add_edge(2, 2);
        assert_eq!(graph.edges(), [(0, 1), (1, 0)], "the two directions are two edges");
        assert!(graph.is_directed() && graph.has_edge(0, 1) && !graph.has_edge(1, 2));
        graph.add_edge(1, 2);
        assert_eq!((graph.neighbours(1), graph.neighbours(2), graph.degree(2)), ([0, 2].as_slice(), [].as_slice(), 0));
        assert!(graph.is_connected() && !graph.is_tree() && !is_acyclic(&graph));

        for n in 2..30 {
            let most = n * (n - 1);
            for m in [0, n, most / 2, most - 1, most] {
                let graph = Graph::new_random_directed(&mut rng(), n, m);
                assert_eq!((graph.num_nodes(), graph.num_edges()), (n, m));
                assert_eq!(graph.is_full(), m == most);
                assert!(graph.edges().iter().all(|&(from, to)| from != to && graph.has_edge(from, to)));
            }
            for m in [0, n - 1, most / 4, most / 2 - 1, most / 2] {
                let dag = Graph::new_random_dag(&mut rng(), n, m);
                assert_eq!(dag.num_edges(), m);
                assert!(is_acyclic(&dag), "{:?}", dag.edges());
            }
        }
        // Not always in the order of the node numbers.
        assert!((0..20).any(|_| Graph::new_random_dag(&mut rng(), 10, 20).edges().iter().any(|&(from, to)| from > to)));
    }

    #[test]
    fn test_directed_edges_keep_their_direction_when_written() {
        let mut graph = Graph::new_empty_directed(&mut rng(), 40);
        for node in 1..40 {
            graph.add_edge(node, node - 1);
        }
        let output = graph.to_output();
        assert_eq!(output.lines().next(), Some("40 39"));
        for line in output.lines().skip(1) {
            let ends: Vec<usize> = line.split(' ').map(|end| end.parse().unwrap()).collect();
            assert_eq!(ends[0], ends[1] + 1, "{line}");
        }
    }

    #[test]
    #[should_panic(expected = "at most 6 are possible")]
    fn test_random_directed_rejects_too_many_edges() {
        let _ = Graph::new_random_directed(&mut rng(), 3, 7);
    }

    #[test]
    #[should_panic(expected = "directed acyclic graph with 3 nodes (at most 3 are possible)")]
    fn test_random_dag_rejects_too_many_edges() {
        let _ = Graph::new_random_dag(&mut rng(), 3, 4);
    }

    /// 0 - 1, 0 - 2, 2 - 3, with the edges added in that order.
    fn small_tree() -> Graph {
        let mut tree = Graph::new_empty(&mut rng(), 4);
        for (u, v) in [(0, 1), (0, 2), (2, 3)] {
            tree.add_edge(u, v);
        }
        tree
    }

    #[test]
    fn test_formats() {
        assert_eq!(small_tree().with_format(GraphFormat::ParentList).to_output(), "4\n1 1 3\n");
        assert_eq!(small_tree().with_format(GraphFormat::ParentList).zero_based().to_output(), "4\n0 0 2\n");
        assert_eq!(Graph::new_random_tree(&mut rng(), 1).with_format(GraphFormat::ParentList).to_output(), "1\n");

        let edges_only = small_tree().with_format(GraphFormat::EdgesOnly).to_output();
        assert_eq!(edges_only.lines().count(), 3);
        let mut lines: Vec<_> = small_tree().zero_based().to_output().lines().map(str::to_owned).collect();
        assert_eq!(lines.remove(0), "4 3");
        let mut edges: Vec<(usize, usize)> = lines
            .iter()
            .map(|line| {
                let (u, v) = line.split_once(' ').unwrap();
                (u.parse().unwrap(), v.parse().unwrap())
            })
            .map(|(u, v)| (usize::min(u, v), usize::max(u, v)))
            .collect();
        edges.sort_unstable();
        assert_eq!(edges, [(0, 1), (0, 2), (2, 3)]);

        // The parents do not depend on the order of the edges or of their ends.
        let mut reversed = Graph::new_empty(&mut rng(), 4);
        for (u, v) in [(3, 2), (2, 0), (1, 0)] {
            reversed.add_edge(u, v);
        }
        assert_eq!(reversed.with_format(GraphFormat::ParentList).to_output(), "4\n1 1 3\n");
    }

    #[test]
    #[should_panic(expected = "written as a tree")]
    fn test_parent_list_rejects_a_cycle() {
        let _ = Graph::new_cycle(&mut rng(), 5).with_format(GraphFormat::ParentList).to_output();
    }

    #[test]
    fn test_weights() {
        let weighted = small_tree().with_weights(vec![10, 20, 30]);
        assert_eq!(weighted.weights(), Some([10, 20, 30].as_slice()));
        let mut lines: Vec<_> = weighted
            .to_output()
            .lines()
            .skip(1)
            .map(|line| {
                let values: Vec<usize> = line.split(' ').map(|value| value.parse().unwrap()).collect();
                (usize::min(values[0], values[1]), usize::max(values[0], values[1]), values[2])
            })
            .collect();
        lines.sort_unstable();
        assert_eq!(lines, [(1, 2, 10), (1, 3, 20), (3, 4, 30)], "a weight stays with its edge");

        assert_eq!(small_tree().with_weights(vec![10, 20, 30]).with_format(GraphFormat::ParentList).to_output(), "4\n1 1 3\n10 20 30\n");

        let random = Graph::new_random(&mut rng(), 30, 100).with_random_weights(&mut rng(), -5..=5);
        let weights = random.weights().unwrap();
        assert!(weights.len() == 100 && weights.iter().all(|weight| (-5..=5).contains(weight)));
        assert!(random.to_output().lines().skip(1).all(|line| line.split(' ').count() == 3));
    }

    #[test]
    #[should_panic(expected = "one weight per edge")]
    fn test_weights_have_to_match_the_edges() {
        let _ = small_tree().with_weights(vec![1, 2]);
    }

    #[test]
    #[should_panic(expected = "after its weights")]
    fn test_edges_added_after_the_weights_are_rejected() {
        let mut graph = small_tree().with_format(GraphFormat::EdgeList).with_weights(vec![1, 2, 3]);
        graph.add_edge(1, 3);
        let _ = graph.to_output();
    }

    #[test]
    fn test_shuffled_labels() {
        let grid = Graph::new_grid(&mut rng(), 6, 7);
        let (edges, degrees) = (grid.edges().to_vec(), degree_counts(&grid));
        let shuffled = grid.with_weights((0..71).collect()).with_shuffled_labels(&mut rng());
        assert_ne!(shuffled.edges(), edges, "the nodes kept their numbers");
        assert_eq!((shuffled.num_edges(), degree_counts(&shuffled)), (71, degrees));
        assert!(shuffled.is_bipartite() && shuffled.is_connected());
        assert!(shuffled.edges().iter().all(|&(u, v)| u > v && shuffled.has_edge(u, v) && shuffled.neighbours(u).contains(&v)));
        assert_eq!(shuffled.weights().map(<[i64]>::len), Some(71));
    }

    /// As for the older constructors in the golden tests: a published seed has
    /// to keep rebuilding the same graph.
    #[test]
    fn the_newer_constructors_are_frozen() {
        let builders: [(&str, fn(&mut Rng) -> Graph, u64); 12] = [
            ("star", |rng| Graph::new_star(rng, 20), 0x57c5_b969_c5e5_d3d5),
            ("caterpillar", |rng| Graph::new_caterpillar(rng, 30, 8), 0xb942_73f1_91d1_510e),
            ("binary tree", |rng| Graph::new_binary_tree(rng, 30), 0x7ff5_399e_1a52_4f1e),
            ("uniform tree", |rng| Graph::new_random_uniform_tree(rng, 30), 0xbfcb_e66a_31f7_32ae),
            ("forest", |rng| Graph::new_random_forest(rng, 30, 4), 0xa8f4_28b0_d34f_4302),
            ("cycle", |rng| Graph::new_cycle(rng, 20), 0x299d_dd31_6dcc_d447),
            ("grid", |rng| Graph::new_grid(rng, 4, 5), 0xd825_0143_dd2d_0acc),
            ("directed sparse", |rng| Graph::new_random_directed(rng, 20, 40), 0xfce3_9680_8980_50d9),
            ("directed dense", |rng| Graph::new_random_directed(rng, 8, 50), 0x982d_5e67_1c58_04fd),
            ("dag", |rng| Graph::new_random_dag(rng, 20, 60), 0x85bd_5687_211f_a624),
            (
                "weighted parents",
                |rng| Graph::new_random_tree(rng, 20).with_random_weights(rng, 1..=1_000_000_000).with_format(GraphFormat::ParentList),
                0x5fb5_563d_9fc8_c63a,
            ),
            (
                "shuffled grid",
                |rng| Graph::new_grid(rng, 4, 5).with_shuffled_labels(rng).zero_based().with_format(GraphFormat::EdgesOnly),
                0x51b0_3a4f_b49d_8ae7,
            ),
        ];
        let got: Vec<_> = builders.iter().map(|&(name, build, _)| (name, stable_hash(&build(&mut Rng::from_seed(1)).to_output()))).collect();
        let expected: Vec<_> = builders.iter().map(|&(name, _, expected)| (name, expected)).collect();
        assert_eq!(got, expected, "\n{got:#018x?}");
    }
}
