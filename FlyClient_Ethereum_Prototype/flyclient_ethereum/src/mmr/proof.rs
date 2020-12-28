use ethereum_types::H256;
use std::collections::VecDeque;
use std::fmt;

use super::get_depth;
use super::persistent_mmr::{get_left_leaf_number, hash_children, Datastore, MerkleTree, MmrElem};
use crate::ProofBlock;

#[derive(Debug, Clone)]
pub struct Proof {
    root_hash: H256,
    root_difficulty: u128,
    leaf_number: u64,
    elem: VecDeque<ProofElem>,
}

impl fmt::Display for Proof {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "")?;
        writeln!(f, "--- Proof ---")?;
        writeln!(f, "Root hash: {:?}", self.root_hash)?;
        writeln!(f, "Leaf number: {:?}", self.leaf_number)?;
        for e in &self.elem {
            write!(f, "{}", e)?;
        }
        writeln!(f, "-------------")
    }
}

impl Proof {
    pub fn get_leaf_number(&self) -> u64 {
        self.leaf_number
    }

    pub fn get_root_hash(&self) -> H256 {
        self.root_hash
    }

    pub fn get_root_difficulty(&self) -> u128 {
        self.root_difficulty
    }

    pub fn generate_proof<T: Datastore>(
        mmr: &mut MerkleTree<T>,
        block_numbers: &mut Vec<u64>,
    ) -> Self {
        block_numbers.sort();
        block_numbers.dedup();

        let mut proof = VecDeque::new();

        let mut current_node = mmr.get_elem();

        let depth = get_depth(mmr.get_leaf_number());

        generate_proof_recursive(
            &mut current_node,
            block_numbers,
            &mut proof,
            2u64.pow(depth - 1),
            depth,
            mmr.get_leaf_number(),
            0,
            mmr,
        );

        proof.push_back(ProofElem::Root(
            (current_node.get_hash(), current_node.get_difficulty()),
            mmr.get_leaf_number(),
        ));

        Proof {
            root_hash: mmr.get_root_hash(),
            root_difficulty: mmr.get_root_difficulty(),
            leaf_number: mmr.get_leaf_number(),
            elem: proof,
        }
    }

    pub fn verify_proof(&mut self, mut proof_blocks: Vec<ProofBlock>) -> Result<(), String> {
        let proof = &mut self.elem;
        proof_blocks.sort();
        proof_blocks.dedup();
        proof_blocks.reverse();

        let (root, leaf_number) = match proof.pop_back() {
            None => return Err("Empty proof".to_owned()),
            Some(ProofElem::Root(hash, leaf_number)) => (hash, leaf_number),
            _ => return Err("Got proof element, which is not the expected root".to_owned()),
        };

        // edge case: if only one element proof
        if proof.len() == 1 {
            match proof.pop_back() {
                Some(ProofElem::Child(ch)) if ch.0 == root.0 => return Ok(()),
                _ => return Err("Single element in proof is not correct".to_owned()),
            }
        }

        let mut nodes: VecDeque<((H256, u128), Option<u64>, Option<u64>)> = VecDeque::new(); // consists of (node, index, leaf_number)

        while !proof.is_empty() {
            let proof_elem = proof.pop_front().unwrap();

            match proof_elem {
                ProofElem::Child(child) => {
                    let proof_block = proof_blocks.pop().unwrap();
                    let number = proof_block.number;

                    if !nodes.is_empty() {
                        //TODO: Verification of previous MMR should happen here
                        //weil in einem Ethereum block header kein mmr hash vorhanden ist, kann man
                        //dies nicht überprüfen, wenn doch irgendwann vorhanden, dann einfach
                        //'block_header.mmr == old_root_hash' überprüfen
                        let (_old_root_hash, left_difficulty) = get_root(&nodes);

                        if let Some(aggr_weight) = proof_block.aggr_weight {
                            let left = left_difficulty as f64;
                            let middle = (aggr_weight * root.1 as f64).ceil();
                            let right = (left_difficulty + child.1) as f64;

                            if (left > middle) || (right <= middle) {
                                return Err(format!(
                                "aggregated difficulty is not correct, should coincide with: {} <= {} < {}",
                                left, middle, right
                            ));
                            }
                        }
                    }
                    if number % 2 == 0 && number != (leaf_number - 1) {
                        let right_node = proof.pop_front().unwrap();

                        let (right_node_hash, right_node_diff) = match right_node {
                            ProofElem::Child(child) => {
                                proof_blocks.pop().unwrap();
                                (child.0, child.1)
                            }
                            ProofElem::Node(node, _) => (node.0, node.1),
                            _ => return Err("Expected ???".to_owned()),
                        };

                        let hash = hash_children(
                            &MmrElem::new(child.0, child.1),
                            &MmrElem::new(right_node_hash, right_node_diff),
                        );

                        nodes.push_back((
                            (hash, child.1 + right_node_diff),
                            Some(number / 2),
                            Some(leaf_number / 2),
                        ));
                    } else {
                        let (left_node, _, _) = nodes.pop_back().unwrap();

                        let hash = hash_children(
                            &MmrElem::new(left_node.0, left_node.1),
                            &MmrElem::new(child.0, child.1),
                        );

                        nodes.push_back((
                            (hash, child.1 + left_node.1),
                            Some(number / 2),
                            Some(leaf_number / 2),
                        ));
                    }
                }
                ProofElem::Node(node, dir) => {
                    if dir {
                        let (left_node, left_index, curr_leaf_nb) = nodes.pop_back().unwrap();

                        let hash = hash_children(
                            &MmrElem::new(left_node.0, left_node.1),
                            &MmrElem::new(node.0, node.1),
                        );
                        nodes.push_back((
                            (hash, left_node.1 + node.1),
                            Some(left_index.unwrap() / 2),
                            Some(curr_leaf_nb.unwrap() / 2),
                        ));
                    } else {
                        nodes.push_back((node, None, None));
                    }
                }
                ProofElem::Root(..) => {}
            }

            while nodes.len() > 1 {
                let (node2, index2, leaf_nb_2) = nodes.pop_back().unwrap();
                let (node1, index1, leaf_nb_1) = nodes.pop_back().unwrap();

                if index2.is_none() || (index2.unwrap() % 2 != 1 && !proof.is_empty()) {
                    nodes.push_back((node1, index1, leaf_nb_1));
                    nodes.push_back((node2, index2, leaf_nb_2));
                    break;
                }

                let hash = hash_children(
                    &MmrElem::new(node1.0, node1.1),
                    &MmrElem::new(node2.0, node2.1),
                );
                nodes.push_back((
                    (hash, node1.1 + node2.1),
                    Some(index2.unwrap() / 2),
                    Some(leaf_nb_2.unwrap() / 2),
                ));
            }
        }

        match nodes.pop_back() {
            None => return Err("Expected ??".to_owned()),
            Some((v, _, _)) => {
                // check if calculated hash and difficulty is correct
                if v.0 != root.0 {
                    return Err(
                        format! {"Calculated root node hash is not correct, Expected: {:?}, Got: {:?}", root.0, v.0},
                    );
                } else if v.1 != root.1 {
                    return Err("Calculated root difficulty is not correct".to_owned());
                } else {
                    return Ok(());
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum ProofElem {
    Node((H256, u128), bool), // bool: left -> false, right -> true
    Root((H256, u128), u64),
    Child((H256, u128)),
}

impl Proof {
    pub fn serialize(&self) -> Vec<u8> {
        let root_hash = self.root_hash;
        let root_difficulty = self.root_difficulty;
        let leaf_number = self.leaf_number;

        let mut serialized_proof: Vec<u8> = vec![];

        serialized_proof.extend_from_slice(root_hash.as_bytes());
        serialized_proof.extend_from_slice(&root_difficulty.to_be_bytes());
        serialized_proof.extend_from_slice(&leaf_number.to_be_bytes());

        for elem in &self.elem {
            match elem {
                ProofElem::Child((hash, diff)) => {
                    serialized_proof.push(0);
                    serialized_proof.extend_from_slice(hash.as_bytes());
                    serialized_proof.extend_from_slice(&diff.to_be_bytes());
                }
                ProofElem::Root((hash, diff), leaf_number) => {
                    serialized_proof.push(1);
                    serialized_proof.extend_from_slice(hash.as_bytes());
                    serialized_proof.extend_from_slice(&diff.to_be_bytes());
                    serialized_proof.extend_from_slice(&leaf_number.to_be_bytes());
                }
                ProofElem::Node((hash, diff), direction) => {
                    if *direction {
                        serialized_proof.push(2);
                    } else {
                        serialized_proof.push(3)
                    }
                    serialized_proof.extend_from_slice(hash.as_bytes());
                    serialized_proof.extend_from_slice(&diff.to_be_bytes());
                }
            }
        }

        serialized_proof
    }

    pub fn deserialize(proof: &[u8]) -> Result<Proof, &'static str> {
        if proof.len() < 64 {
            return Err("Proof is invalid");
        }

        let mut buf = [0; 32];
        let mut buf16 = [0; 16];
        let mut buf8 = [0; 8];

        let mut n = 0;
        buf.copy_from_slice(&proof[n..n + 32]);
        let root_hash = H256::from(&buf);
        n += 32;

        buf16.copy_from_slice(&proof[n..n + 16]);
        let root_difficulty = u128::from_be_bytes(buf16);
        n += 16;

        buf8.copy_from_slice(&proof[n..n + 8]);
        let leaf_number = u64::from_be_bytes(buf8);
        n += 8;

        let mut elem: VecDeque<ProofElem> = VecDeque::new();

        // parse all remaining elements
        while n < proof.len() {
            match proof[n] {
                0 => {
                    n += 1;
                    if n + 48 > proof.len() {
                        return Err("Proof is invalid");
                    }
                    buf.copy_from_slice(&proof[n..n + 32]);
                    let child_hash = H256::from(&buf);
                    n += 32;

                    buf16.copy_from_slice(&proof[n..n + 16]);
                    let child_difficulty = u128::from_be_bytes(buf16);
                    n += 16;

                    elem.push_back(ProofElem::Child((child_hash, child_difficulty)));
                }
                1 => {
                    n += 1;
                    if n + 56 > proof.len() {
                        return Err("Proof is invalid");
                    }
                    buf.copy_from_slice(&proof[n..n + 32]);
                    let root_hash = H256::from(&buf);
                    n += 32;

                    buf16.copy_from_slice(&proof[n..n + 16]);
                    let root_difficulty = u128::from_be_bytes(buf16);
                    n += 16;

                    buf8.copy_from_slice(&proof[n..n + 8]);
                    let root_leaf_number = u64::from_be_bytes(buf8);
                    n += 8;

                    elem.push_back(ProofElem::Root(
                        (root_hash, root_difficulty),
                        root_leaf_number,
                    ));
                }
                2 => {
                    n += 1;
                    if n + 48 > proof.len() {
                        return Err("Proof is invalid");
                    }
                    buf.copy_from_slice(&proof[n..n + 32]);
                    let node_hash = H256::from(&buf);
                    n += 32;

                    buf16.copy_from_slice(&proof[n..n + 16]);
                    let node_difficulty = u128::from_be_bytes(buf16);
                    n += 16;

                    elem.push_back(ProofElem::Node((node_hash, node_difficulty), true));
                }
                3 => {
                    n += 1;
                    if n + 48 > proof.len() {
                        return Err("Proof is invalid");
                    }
                    buf.copy_from_slice(&proof[n..n + 32]);
                    let node_hash = H256::from(&buf);
                    n += 32;

                    buf16.copy_from_slice(&proof[n..n + 16]);
                    let node_difficulty = u128::from_be_bytes(buf16);
                    n += 16;

                    elem.push_back(ProofElem::Node((node_hash, node_difficulty), false));
                }
                _ => return Err("Proof is invalid"),
            }
        }

        Ok(Proof {
            root_hash,
            root_difficulty,
            leaf_number,
            elem,
        })
    }
}

impl fmt::Display for ProofElem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ProofElem::Node((hash, difficulty), dir) => {
                let dir = match dir {
                    false => "Left ",
                    true => "Right",
                };
                writeln!(f, "{} Node {:?}, Diff: {}", dir, hash, difficulty)
            }
            ProofElem::Root((hash, difficulty), leaf_number) => writeln!(
                f,
                "Root       {:?}, Diff: {}, Leaves: {}",
                hash, difficulty, leaf_number
            ),
            ProofElem::Child((hash, difficulty)) => {
                writeln!(f, "Child      {:?}, Diff: {}", hash, difficulty)
            }
        }
    }
}

fn generate_proof_recursive<T: Datastore>(
    current_node: &mut MmrElem,
    block_numbers: &mut [u64],
    proof: &mut VecDeque<ProofElem>,
    max_left_tree_leaf_number: u64,
    starting_depth: u32,
    leaf_number_sub_tree: u64,
    space: usize,
    mut mmr: &mut MerkleTree<T>,
) {
    if !current_node.has_children(&mut mmr) {
        proof.push_back(ProofElem::Child((
            current_node.get_hash(),
            current_node.get_difficulty(),
        )));
        return;
    }

    let (mut left_node, mut right_node) = current_node.get_children(&mut mmr);

    match block_numbers.binary_search(&max_left_tree_leaf_number) {
        Ok(v) | Err(v) => {
            let (left, right) = block_numbers.split_at_mut(v);

            let next_left_leaf_number_subtree = get_left_leaf_number(leaf_number_sub_tree);

            if !left.is_empty() {
                let depth = get_depth(next_left_leaf_number_subtree);

                let diff = if depth < 1 { 0 } else { 2u64.pow(depth - 1) };

                generate_proof_recursive(
                    &mut left_node,
                    left,
                    proof,
                    max_left_tree_leaf_number - diff,
                    starting_depth,
                    next_left_leaf_number_subtree,
                    space + 1,
                    mmr,
                );
            } else {
                proof.push_back(ProofElem::Node(
                    (left_node.get_hash(), left_node.get_difficulty()),
                    false,
                ));
            }

            if !right.is_empty() {
                let depth = get_depth(leaf_number_sub_tree - next_left_leaf_number_subtree);
                let diff = if depth < 1 { 0 } else { 2u64.pow(depth - 1) };

                generate_proof_recursive(
                    &mut right_node,
                    right,
                    proof,
                    max_left_tree_leaf_number + diff,
                    starting_depth,
                    leaf_number_sub_tree - next_left_leaf_number_subtree,
                    space + 1,
                    mmr,
                );
            } else {
                proof.push_back(ProofElem::Node(
                    (right_node.get_hash(), right_node.get_difficulty()),
                    true,
                ));
            }
        }
    }
}

fn get_root(nodes: &VecDeque<((H256, u128), Option<u64>, Option<u64>)>) -> (H256, u128) {
    let mut temp_nodes = VecDeque::new();
    for node in nodes {
        temp_nodes.push_back(node.clone());
    }

    while temp_nodes.len() > 1 {
        let (node2, _, _) = temp_nodes.pop_back().unwrap();
        let (node1, _, _) = temp_nodes.pop_back().unwrap();

        let hash = hash_children(
            &MmrElem::new(node1.0, node1.1),
            &MmrElem::new(node2.0, node2.1),
        );
        temp_nodes.push_back(((hash, node1.1 + node2.1), None, None));
    }
    let (old_root, _, _) = temp_nodes.pop_back().unwrap();
    (old_root.0, old_root.1)
}
