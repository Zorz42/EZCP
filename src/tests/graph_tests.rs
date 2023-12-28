#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod graph_tests {
    use crate::generators::Graph;

    #[test]
    fn test_empty() {
        for i in 1..100 {
            let graph = Graph::new_empty(i);
            assert_eq!(graph.get_num_nodes(), i);
            assert_eq!(graph.get_num_edges(), 0);
            for u in 0..i {
                for v in 0..i {
                    assert!(!graph.has_edge(u as usize, v as usize));
                }
            }
        }
    }

    #[test]
    fn test_full() {
        for i in 1..100 {
            let graph = Graph::new_full(i);
            assert_eq!(graph.get_num_nodes(), i);
            assert_eq!(graph.get_num_edges(), i * (i - 1) / 2);
            for u in 0..i {
                for v in 0..i {
                    assert_eq!(graph.has_edge(u as usize, v as usize), u != v);
                }
            }
        }
    }

    #[test]
    fn test_add_edge() {
        let mut graph = Graph::new_empty(5);
        assert_eq!(graph.get_num_edges(), 0);
        graph.add_edge(0, 1);
        assert_eq!(graph.get_num_edges(), 1);
        graph.add_edge(1, 2);
        assert_eq!(graph.get_num_edges(), 2);
        graph.add_edge(0, 3);
        assert_eq!(graph.get_num_edges(), 3);

        assert!(!graph.has_edge(0, 0));
        assert!(graph.has_edge(0, 1));
        assert!(!graph.has_edge(0, 2));
        assert!(graph.has_edge(0, 3));
        assert!(!graph.has_edge(0, 4));
        assert!(graph.has_edge(1, 0));
        assert!(!graph.has_edge(1, 1));
        assert!(graph.has_edge(1, 2));
        assert!(!graph.has_edge(1, 3));
        assert!(!graph.has_edge(1, 4));
        assert!(!graph.has_edge(2, 0));
        assert!(graph.has_edge(2, 1));
        assert!(!graph.has_edge(2, 2));
        assert!(!graph.has_edge(2, 3));
        assert!(!graph.has_edge(2, 4));
        assert!(graph.has_edge(3, 0));
        assert!(!graph.has_edge(3, 1));
        assert!(!graph.has_edge(3, 2));
        assert!(!graph.has_edge(3, 3));
        assert!(!graph.has_edge(3, 4));
        assert!(!graph.has_edge(4, 0));
        assert!(!graph.has_edge(4, 1));
        assert!(!graph.has_edge(4, 2));
        assert!(!graph.has_edge(4, 3));
        assert!(!graph.has_edge(4, 4));
    }

    #[test]
    fn test_random() {
        for i in 5..100 {
            let graph = Graph::new_random(i, 2 * i);
            assert_eq!(graph.get_num_nodes(), i);
            assert_eq!(graph.get_num_edges(), 2 * i);
            let mut counted_edges = 0;
            for u in 0..i {
                for v in 0..i {
                    assert_eq!(graph.has_edge(u as usize, v as usize), graph.has_edge(v as usize, u as usize));
                    if graph.has_edge(u as usize, v as usize) {
                        counted_edges += 1;
                    }
                }
            }
            assert_eq!(counted_edges, 4 * i);
        }

        for i in 3..100 {
            let graph = Graph::new_random(i, i);
            assert_eq!(graph.get_num_nodes(), i);
            assert_eq!(graph.get_num_edges(), i);
            let mut counted_edges = 0;
            for u in 0..i {
                for v in 0..i {
                    assert_eq!(graph.has_edge(u as usize, v as usize), graph.has_edge(v as usize, u as usize));
                    if graph.has_edge(u as usize, v as usize) {
                        counted_edges += 1;
                    }
                }
            }
            assert_eq!(counted_edges, 2 * i);
        }
    }

    #[test]
    fn test_random_tree() {
        for i in 1..100 {
            let graph = Graph::new_random_tree(i);
            assert_eq!(graph.get_num_nodes(), i);
            assert_eq!(graph.get_num_edges(), i - 1);
            assert!(graph.is_tree());
        }
    }

    #[test]
    fn test_random_connected() {
        let graph = Graph::new_random_connected(10, 20);
        assert_eq!(graph.get_num_nodes(), 10);
        assert_eq!(graph.get_num_edges(), 20);
    }

    #[test]
    fn test_is_tree() {
        let mut graph = Graph::new_empty(10);
        assert!(!graph.is_tree());
        graph.add_edge(0, 1);
        assert!(!graph.is_tree());
        graph.add_edge(1, 2);
        assert!(!graph.is_tree());
        graph.add_edge(2, 3);
        assert!(!graph.is_tree());
        graph.add_edge(3, 4);
        assert!(!graph.is_tree());
        graph.add_edge(4, 5);
        assert!(!graph.is_tree());
        graph.add_edge(5, 6);
        assert!(!graph.is_tree());
        graph.add_edge(6, 7);
        assert!(!graph.is_tree());
        graph.add_edge(7, 8);
        assert!(!graph.is_tree());
        graph.add_edge(8, 9);
        assert!(graph.is_tree());
        graph.add_edge(9, 0);
        assert!(!graph.is_tree());

        let mut graph = Graph::new_empty(5);
        assert!(!graph.is_tree());
        graph.add_edge(0, 1);
        assert!(!graph.is_tree());
        graph.add_edge(0, 2);
        assert!(!graph.is_tree());
        graph.add_edge(2, 3);
        assert!(!graph.is_tree());
        graph.add_edge(2, 4);
        assert!(graph.is_tree());
        graph.add_edge(4, 0);
        assert!(!graph.is_tree());

        for i in 3..100 {
            let graph = Graph::new_full(i);
            assert!(!graph.is_tree());
        }

        for i in 3..100 {
            let graph = Graph::new_random(i, i);
            assert!(!graph.is_tree());
        }

        for i in 2..100 {
            let graph = Graph::new_random(i, i - 2);
            assert!(!graph.is_tree());
        }

        for i in 1..100 {
            let graph = Graph::new_random_connected(i, i - 1);
            assert!(graph.is_tree());
        }

        for i in 2..100 {
            let graph = Graph::new_random_connected(i, i - 2);
            assert!(graph.is_tree());
        }

        for i in 1..100 {
            let graph = Graph::new_random_tree(i);
            assert!(graph.is_tree());
        }

        let mut graph = Graph::new_empty(4);
        graph.add_edge(0, 1);
        graph.add_edge(1, 2);
        graph.add_edge(2, 0);
        assert!(!graph.is_tree());
    }

    #[test]
    fn test_random_bipartite() {
        for i in 10..100 {
            let graph = Graph::new_random_bipartite(i, 2 * i);
            assert_eq!(graph.get_num_nodes(), i);
            assert_eq!(graph.get_num_edges(), 2 * i);
            assert!(graph.is_bipartite());
        }

        for i in 20..100 {
            let graph = Graph::new_random_bipartite(i, 3 * i);
            assert_eq!(graph.get_num_nodes(), i);
            assert_eq!(graph.get_num_edges(), 3 * i);
            assert!(graph.is_bipartite());
        }

        for i in 10..100 {
            let graph = Graph::new_random_bipartite(i, i);
            assert_eq!(graph.get_num_nodes(), i);
            assert_eq!(graph.get_num_edges(), i);
            assert!(graph.is_bipartite());
        }

        for i in 10..100 {
            let graph = Graph::new_random_bipartite(i, i / 2);
            assert_eq!(graph.get_num_nodes(), i);
            assert_eq!(graph.get_num_edges(), i / 2);
            assert!(graph.is_bipartite());
        }
    }

    #[test]
    fn test_is_bipartite() {
        let mut graph = Graph::new_empty(10);
        assert!(graph.is_bipartite());
        graph.add_edge(0, 1);
        assert!(graph.is_bipartite());
        graph.add_edge(1, 2);
        assert!(graph.is_bipartite());
        graph.add_edge(2, 3);
        assert!(graph.is_bipartite());
        graph.add_edge(3, 4);
        assert!(graph.is_bipartite());
        graph.add_edge(4, 5);
        assert!(graph.is_bipartite());
        graph.add_edge(5, 6);
        assert!(graph.is_bipartite());
        graph.add_edge(6, 7);
        assert!(graph.is_bipartite());
        graph.add_edge(7, 8);
        assert!(graph.is_bipartite());
        graph.add_edge(8, 9);
        assert!(graph.is_bipartite());
        graph.add_edge(9, 0);
        assert!(graph.is_bipartite());
        graph.add_edge(0, 2);
        assert!(!graph.is_bipartite());

        let mut graph = Graph::new_empty(5);
        assert!(graph.is_bipartite());
        graph.add_edge(0, 1);
        assert!(graph.is_bipartite());
        graph.add_edge(0, 2);
        assert!(graph.is_bipartite());
        graph.add_edge(2, 3);
        assert!(graph.is_bipartite());
        graph.add_edge(2, 4);
        assert!(graph.is_bipartite());
        graph.add_edge(4, 0);
        assert!(!graph.is_bipartite());
    }

    #[test]
    #[allow(clippy::cognitive_complexity)]
    fn test_graph_input_output() {
        let mut graph = Graph::new_empty(5);
        graph.add_edge(0, 1);
        graph.add_edge(1, 2);
        graph.add_edge(0, 3);

        let input = graph.create_output();
        let graph = Graph::new_from_input(&input).unwrap();

        assert_eq!(graph.get_num_nodes(), 5);
        assert_eq!(graph.get_num_edges(), 3);
        assert!(!graph.has_edge(0, 0));
        assert!(graph.has_edge(0, 1));
        assert!(!graph.has_edge(0, 2));
        assert!(graph.has_edge(0, 3));
        assert!(!graph.has_edge(0, 4));
        assert!(graph.has_edge(1, 0));
        assert!(!graph.has_edge(1, 1));
        assert!(graph.has_edge(1, 2));
        assert!(!graph.has_edge(1, 3));
        assert!(!graph.has_edge(1, 4));
        assert!(!graph.has_edge(2, 0));
        assert!(graph.has_edge(2, 1));
        assert!(!graph.has_edge(2, 2));
        assert!(!graph.has_edge(2, 3));
        assert!(!graph.has_edge(2, 4));
        assert!(graph.has_edge(3, 0));
        assert!(!graph.has_edge(3, 1));
        assert!(!graph.has_edge(3, 2));
        assert!(!graph.has_edge(3, 3));
        assert!(!graph.has_edge(3, 4));
        assert!(!graph.has_edge(4, 0));
        assert!(!graph.has_edge(4, 1));
        assert!(!graph.has_edge(4, 2));
        assert!(!graph.has_edge(4, 3));
        assert!(!graph.has_edge(4, 4));

        for n in 1..20 {
            for m in 0..n * (n - 1) / 2 {
                let graph = Graph::new_random(n, m);
                let input = graph.create_output();
                let graph2 = Graph::new_from_input(&input).unwrap();
                assert_eq!(graph2.get_num_nodes(), n);
                assert_eq!(graph2.get_num_edges(), m);
                for u in 0..n {
                    for v in 0..n {
                        assert_eq!(graph.has_edge(u as usize, v as usize), graph2.has_edge(v as usize, u as usize));
                    }
                }
            }
        }
    }

    #[test]
    fn test_get_connected_components() {
        let mut graph = Graph::new_empty(10);
        assert_eq!(
            graph.get_connected_components(),
            vec![vec![0], vec![1], vec![2], vec![3], vec![4], vec![5], vec![6], vec![7], vec![8], vec![9]]
        );

        graph.add_edge(0, 1);
        assert_eq!(graph.get_connected_components().len(), 9);
        graph.add_edge(1, 2);
        assert_eq!(graph.get_connected_components().len(), 8);
        graph.add_edge(2, 3);
        assert_eq!(graph.get_connected_components().len(), 7);
        graph.add_edge(3, 4);
        assert_eq!(graph.get_connected_components().len(), 6);
        graph.add_edge(0, 5);
        assert_eq!(graph.get_connected_components().len(), 5);
        graph.add_edge(3, 0);
        assert_eq!(graph.get_connected_components().len(), 5);
        graph.add_edge(6, 7);
        assert_eq!(graph.get_connected_components().len(), 4);
        graph.add_edge(7, 8);
        assert_eq!(graph.get_connected_components().len(), 3);
        graph.add_edge(8, 9);
        assert_eq!(graph.get_connected_components().len(), 2);
        graph.add_edge(0, 6);
        assert_eq!(graph.get_connected_components().len(), 1);
        graph.add_edge(9, 0);
        assert_eq!(graph.get_connected_components().len(), 1);
        let mut component1 = graph.get_connected_components()[0].clone();
        component1.sort_unstable();
        assert_eq!(component1, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }
}
