#![feature(min_const_generics)]

#[cfg(test)]
mod tests;

use packed_simd::{
    u8x16,
};
use std::collections::{HashMap, BTreeSet};

pub struct ByteTrie16 {
    // [ 0: no_parent? ] [ 1: has value? ] [ 2: has branch? ] [ 3: unused ] [ 4-8: parent pointer ]
    nodes: u8x16,
    // Label of incoming edge
    edges: u8x16,
}

impl ByteTrie16 {
    pub fn new(edges: &BTreeSet<Edge>) -> Self {
        assert!(edges.len() <= 16);
        let (packed_edges, packed_nodes) = build_tree(edges, 8);
        let edges = u8x16::from(packed_edges);
        let nodes = u8x16::from(packed_nodes);
        Self { edges, nodes }
    }

    fn match_bitsets(&self, query: &[u8; 8]) -> u8x16 {
        let zero = u8x16::splat(0);
        let mut out = zero;
        for i in 0..8 {
            let label = u8x16::splat(query[i]);
            let bitset = u8x16::splat(1 << i);
            out |= self.edges.eq(label).select(bitset, zero);
        }
        out
    }

    pub fn traverse(&self, query: &[u8; 8], query_len: usize) -> Lookup {
        let zero = u8x16::splat(0);
        let edge_matches = self.match_bitsets(query);

        let root_byte = 0b1000_0000;
        let matches0 = (self.nodes & u8x16::splat(root_byte)).eq(zero).select(zero, edge_matches);
        let matches1 = (matches0.shuffle1_dyn(self.nodes) << 1) & edge_matches;
        let matches2 = (matches1.shuffle1_dyn(self.nodes) << 1) & edge_matches;
        let matches3 = (matches2.shuffle1_dyn(self.nodes) << 1) & edge_matches;
        let matches4 = (matches3.shuffle1_dyn(self.nodes) << 1) & edge_matches;
        let matches5 = (matches4.shuffle1_dyn(self.nodes) << 1) & edge_matches;
        let matches6 = (matches5.shuffle1_dyn(self.nodes) << 1) & edge_matches;
        let matches7 = (matches6.shuffle1_dyn(self.nodes) << 1) & edge_matches;

        let state = match query_len {
            1 => matches0,
            2 => matches1,
            3 => matches2,
            4 => matches3,
            5 => matches4,
            6 => matches5,
            7 => matches6,
            8 => matches7,
            _ => panic!("Invalid query len"),
        };
        let mask = state & u8x16::splat(1 << (query_len as u8 - 1));
        let match_mask = mask.ne(zero).bitmask();

        let values = (self.nodes & u8x16::splat(1 << 6)).ne(zero).bitmask();
        let branches = (self.nodes & u8x16::splat(1 << 5)).ne(zero).bitmask();

        let value_match = match_mask & values;
        let branch_match = match_mask & branches;

        let branch_pos = branch_match.trailing_zeros();
        if branch_pos != 16 {
            let mask = (1u16 << branch_pos) - 1;
            return Lookup::Branch((branches & mask).count_ones() as u8);
        }

        let value_pos = value_match.trailing_zeros();
        if value_pos != 16 {
            let mask = (1u16 << value_pos) - 1;
            return Lookup::Value((values & mask).count_ones() as u8);
        }

        Lookup::None
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum Lookup {
    None,
    Branch(u8),
    Value(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Edge {
    pub parent: Option<usize>,
    pub label: u8,
    pub number: usize,
    pub has_value: bool,
    pub has_branch: bool,
}

impl Edge {
    fn bound(parent: Option<usize>) -> Self {
        Self {
            parent,
            label: 0,
            number: 0,
            has_value: false,
            has_branch: false,
        }
    }
}

fn build_tree<const N: usize>(edges: &BTreeSet<Edge>, max_depth: usize) -> ([u8; N], [u8; N]) {
    let mut packed_edges = [0b0000_0000; N];
    let mut packed_nodes = [0b0000_0000; N];

    let mut next_dfs = 0u8;
    let mut dfs_assignments: HashMap<usize, u8> = HashMap::new();
    let mut stack: Vec<(Option<Edge>, usize)> = vec![(None, 0)];

    while let Some((maybe_edge, depth)) = stack.pop() {
        assert!(depth <= max_depth);
        if let Some(edge) = maybe_edge {
            let dfs_number = next_dfs;
            next_dfs += 1;
            assert!(dfs_assignments.insert(edge.number, dfs_number).is_none());

            let mut parent_byte = match edge.parent {
                Some(input_ix) => {
                    let dfs_ix = dfs_assignments[&input_ix];
                    assert!(dfs_ix < (N as u8));
                    dfs_ix
                },
                None => 0b1000_0000,
            };
            if edge.has_value {
                parent_byte |= 1 << 6;
            }
            if edge.has_branch {
                parent_byte |= 1 << 5;
            }

            packed_nodes[dfs_number as usize] = parent_byte;
            packed_edges[dfs_number as usize] = edge.label;
        }

        let src_start = maybe_edge.map(|e| e.number);
        let src_end = Some(maybe_edge.map(|e| e.number + 1).unwrap_or(0));
        for &edge in edges.range(Edge::bound(src_start)..Edge::bound(src_end)).rev() {
            stack.push((Some(edge), depth + 1));
        }
    }

    (packed_edges, packed_nodes)
}
