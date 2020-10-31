use super::{ByteTrie16, Edge, Lookup};

use rand_distr::{Distribution, Exp};
use rand::{SeedableRng, Rng};
use rand_isaac::IsaacRng;
use std::collections::{VecDeque, BTreeSet, HashMap};

pub struct TestTree {
    edges: BTreeSet<Edge>,
}

impl TestTree {
    pub fn generate(rng: &mut impl Rng) -> Self {
        let num_children_dist = Exp::new(0.25).unwrap();

        let mut queue = VecDeque::new();
        queue.push_back((None, 1));

        let mut edges = BTreeSet::new();

        while let Some((parent, depth)) = queue.pop_front() {
            if depth > 8 {
                continue;
            }

            let num_children: f64 = num_children_dist.sample(rng);
            let num_children = num_children.floor() as usize;
            let mut labels = BTreeSet::new();

            for _ in 0..num_children {
                if edges.len() >= 16 {
                    break;
                }
                let mut label = rng.gen();
                while labels.contains(&label) {
                    label = rng.gen();
                }
                let number = edges.len();

                let has_value_pr: f64 = rng.gen();
                let has_value = has_value_pr <= 0.4;
                let edge = Edge {
                    parent,
                    label,
                    number,
                    has_value,
                    has_branch: false,
                };
                edges.insert(edge);
                if !has_value {
                    queue.push_back((Some(number), depth + 1));
                }
                labels.insert(label);
            }
        }

        Self { edges }
    }

    fn traverse(&self, query: &[u8]) -> Lookup {
        let mut cur_node = None;

        for &byte in query {
            let start = Edge::bound(cur_node);
            let end = Edge::bound(Some(cur_node.map(|n| n + 1).unwrap_or(0)));

            if let Some(e) = self.edges.range(start..end).find(|e| e.label == byte) {
                cur_node = Some(e.number);
                continue;
            }
            return Lookup::None;
        }

        let e = self.edges.iter().find(|e| Some(e.number) == cur_node).unwrap();
        if e.has_branch {
            let branch_rank = self.edges.iter().filter(|e| e.has_branch && Some(e.number) < cur_node).count();
            return Lookup::Branch(branch_rank as u8);
        }
        if e.has_value {
            let value_rank = self.edges.iter().filter(|e| e.has_value && Some(e.number) < cur_node).count();
            return Lookup::Value(value_rank as u8);
        };
        Lookup::None
    }
}

#[test]
fn test_random() {
    let num_iters: usize = std::env::var("NUM_ITERS")
        .map(|s| s.parse().unwrap())
        .unwrap_or(1);
    for _ in 0..num_iters {
        let seed = rand::thread_rng().gen();
        println!("Seed: {:02x?}", seed);
        let mut rng = IsaacRng::from_seed(seed);

        let slow = TestTree::generate(&mut rng);
        let fast = ByteTrie16::new(&slow.edges);

        println!("Edges:");
        for edge in &slow.edges {
            println!("{:?}", edge);
        }

        // First iterate over all of the paths in the tree.
        let mut stack: Vec<(Option<usize>, bool)> = vec![(None, true)];
        let mut query = [0u8; 8];
        let mut query_len = 0;

        let mut labels = HashMap::new();
        for edge in slow.edges.iter() {
            assert!(labels.insert(edge.number, edge.label).is_none());
        }

        let mut keys = BTreeSet::new();

        while let Some((node, first_visit)) = stack.pop() {
            if first_visit {
                if let Some(n) = node {
                    let label = labels[&n];
                    query[query_len] = label;
                    query_len += 1;

                    let slow_query = slow.traverse(&query[..query_len]);
                    let fast_query = fast.traverse(&query, query_len);
                    println!("query: {:?} -> {:?}", &query[..query_len], slow_query);
                    assert_eq!(slow_query, fast_query);
                    keys.insert(query[..query_len].to_owned());
                }

                stack.push((node, false));

                let start = Edge::bound(node);
                let end = Edge::bound(Some(node.map(|n| n + 1).unwrap_or(0)));
                for edge in slow.edges.range(start..end) {
                    stack.push((Some(edge.number), true));
                }
            } else {
                if let Some(..) = node {
                    query_len -= 1;
                }
            }
        }

        // Try a key that isn't in the tree.
        for query_v in &keys {
            let mut query = [0u8; 8];
            let query_len = query_v.len();
            query[..query_len].copy_from_slice(&query_v[..]);

            for _ in 0..=255 {
                query[query_len - 1] = query[query_len - 1].wrapping_add(1);
                if !keys.contains(&query[..query_len]) {
                    let slow_query = slow.traverse(&query[..query_len]);
                    let fast_query = fast.traverse(&query, query_len);
                    println!("negative query: {:?} -> {:?}", &query[..query_len], slow_query);
                    assert_eq!(slow_query, fast_query);
                    assert_eq!(slow_query, Lookup::None);
                    break;
                }
            }
        }
    }
}

// connected
// tree
// max depth 8
// max nodes 16






#[test]
fn test_tree() {
    //       .
    //  1  /    \ 2
    //  3 / \ 4 | 5
    //          | 7
    let e = &[
        Edge { parent: None,    label: 1, number: 0, has_value: false, has_branch: false },
        Edge { parent: None,    label: 2, number: 1, has_value: false, has_branch: false },
        Edge { parent: Some(0), label: 3, number: 2, has_value: true,  has_branch: false },
        Edge { parent: Some(0), label: 4, number: 3, has_value: true,  has_branch: false },
        Edge { parent: Some(1), label: 5, number: 4, has_value: false, has_branch: false },
        Edge { parent: Some(4), label: 7, number: 5, has_value: true,  has_branch: false },
    ];
    let edges = e.iter().cloned().collect();
    let t = ByteTrie16::new(&edges);
    assert_eq!(t.traverse(&[0, 1, 4, 0, 0, 0, 0, 0], 1), Lookup::None);
}
