use ethereum_types::H256;
use tiny_keccak::Keccak;

pub mod persistent_mmr;
pub mod proof;

#[derive(Debug)]
struct MmrElem {
    hash: H256,
    difficulty: u128,
    children: Option<(Box<MmrElem>, Box<MmrElem>)>,
}

impl Default for MmrElem {
    fn default() -> Self {
        MmrElem {
            hash: H256::from(0),
            difficulty: 0,
            children: None,
        }
    }
}

impl MmrElem {
    pub fn new(hash: H256, difficulty: u128) -> Self {
        MmrElem {
            hash,
            difficulty,
            children: None,
        }
    }

    fn serialize(&self) -> [u8; 48] {
        let mut slice = [0u8; 48];

        slice[0..32].copy_from_slice(self.hash.as_bytes());
        slice[32..48].copy_from_slice(&self.difficulty.to_be_bytes());

        slice
    }

    pub fn get_hash(&self) -> H256 {
        self.hash
    }

    pub fn get_difficulty(&self) -> u128 {
        self.difficulty
    }

    pub fn has_children(&self) -> bool {
        self.children.is_some()
    }

    pub fn get_children(&self) -> &(Box<MmrElem>, Box<MmrElem>) {
        self.children.as_ref().unwrap()
    }
}

#[derive(Debug)]
pub struct MerkleTree {
    leaf_number: u64,
    elem: MmrElem,
}

impl MerkleTree {
    pub fn new(hash: H256, difficulty: u128) -> Self {
        let leaf = MmrElem {
            hash,
            difficulty,
            children: None,
        };

        MerkleTree {
            leaf_number: 1,
            elem: leaf,
        }
    }

    pub fn get_leaf_number(&self) -> u64 {
        self.leaf_number
    }

    fn get_elem(&self) -> &MmrElem {
        &self.elem
    }

    pub fn append_leaf(&mut self, hash: H256, difficulty: u128) {
        let leaf = MmrElem {
            hash,
            difficulty,
            children: None,
        };

        let mut elem = Default::default();
        std::mem::swap(&mut self.elem, &mut elem);
        self.elem = append_recursive(elem, leaf, self.leaf_number);
        self.leaf_number += 1;
    }

    pub fn get_root_hash(&self) -> H256 {
        self.elem.hash
    }

    pub fn get_root_difficulty(&self) -> u128 {
        self.elem.difficulty
    }

    pub fn get_difficulty_relation(&self, leaf_number: u64) -> f64 {
        self.get_left_difficulty(leaf_number) as f64 / self.get_root_difficulty() as f64
    }

    pub fn get_left_difficulty(&self, leaf_number: u64) -> u128 {
        let depth = get_depth(self.get_leaf_number());
        traverse_tree_left(
            &self.elem,
            2u64.pow(depth - 1),
            depth,
            self.leaf_number,
            leaf_number,
            0,
        )
    }

    pub fn get_child_by_aggr_weight(&self, weight: f64) -> u64 {
        let root_weight = self.get_root_difficulty();
        let weight_disc = (root_weight as f64 * weight) as u128;
        let depth = get_depth(self.get_leaf_number());

        traverse_tree_weight(
            &self.elem,
            weight_disc,
            0,
            self.get_leaf_number(),
            2u64.pow(depth - 1),
            0,
        )
    }
}

fn traverse_tree_weight(
    curr_elem: &MmrElem,
    target_weight: u128,
    curr_weight: u128,
    leaf_number_sub_tree: u64,
    max_left_tree_leaf_number: u64,
    aggr_leaf_number: u64,
) -> u64 {
    if let Some((ref left, ref right)) = curr_elem.children {
        let next_left_leaf_number_subtree = get_left_subtree_number(leaf_number_sub_tree);

        if target_weight <= (curr_weight + left.difficulty) {
            //branch left
            let depth = get_depth(next_left_leaf_number_subtree);
            let diff = if depth < 1 { 0 } else { 2u64.pow(depth - 1) };

            traverse_tree_weight(
                &left,
                target_weight,
                curr_weight,
                next_left_leaf_number_subtree,
                max_left_tree_leaf_number - diff,
                aggr_leaf_number,
            )
        } else {
            // branch right
            let depth = get_depth(leaf_number_sub_tree - next_left_leaf_number_subtree);
            let diff = if depth < 1 { 0 } else { 2u64.pow(depth - 1) };

            traverse_tree_weight(
                &right,
                target_weight,
                curr_weight + left.difficulty,
                leaf_number_sub_tree - next_left_leaf_number_subtree,
                max_left_tree_leaf_number + diff,
                aggr_leaf_number + next_left_leaf_number_subtree,
            )
        }
    } else {
        aggr_leaf_number
    }
}

fn traverse_tree_left(
    curr_node: &MmrElem,
    max_left_tree_leaf_number: u64,
    starting_depth: u32,
    leaf_number_sub_tree: u64,
    target_leaf_number: u64,
    mut aggr_weight: u128,
) -> u128 {
    if let Some((left_child, right_child)) = &curr_node.children {
        let next_left_leaf_number_subtree = get_left_subtree_number(leaf_number_sub_tree);

        if target_leaf_number < next_left_leaf_number_subtree {
            // branch left
            let depth = get_depth(next_left_leaf_number_subtree);
            let diff = if depth < 1 { 0 } else { 2u64.pow(depth - 1) };

            aggr_weight = traverse_tree_left(
                &left_child,
                max_left_tree_leaf_number - diff,
                starting_depth,
                next_left_leaf_number_subtree,
                target_leaf_number,
                aggr_weight,
            );
        } else {
            // branch right
            let depth = get_depth(leaf_number_sub_tree - next_left_leaf_number_subtree);
            let diff = if depth < 1 { 0 } else { 2u64.pow(depth - 1) };

            aggr_weight += left_child.difficulty;
            aggr_weight = traverse_tree_left(
                &right_child,
                max_left_tree_leaf_number + diff,
                starting_depth,
                leaf_number_sub_tree - next_left_leaf_number_subtree,
                target_leaf_number - next_left_leaf_number_subtree,
                aggr_weight,
            );
        }
    }

    aggr_weight
}

// Because a node in an MMR has always maximum node numbers on the left child, it is always the
// biggest value 2^k (k elem N), which is smaller than the whole leaf_number
fn get_left_subtree_number(leaf_number: u64) -> u64 {
    if leaf_number.is_power_of_two() {
        leaf_number / 2
    } else {
        leaf_number.next_power_of_two() / 2
    }
}

// Get depth of the MMR with a specified leaf_number
fn get_depth(leaf_number: u64) -> u32 {
    let mut depth = 64 - leaf_number.leading_zeros() - 1;
    if !leaf_number.is_power_of_two() {
        depth += 1;
    }
    depth
}

fn hash_children(left: &MmrElem, right: &MmrElem) -> H256 {
    let mut hasher = Keccak::new_sha3_256();

    hasher.update(&left.serialize());
    hasher.update(&right.serialize());

    let mut res: [u8; 32] = [0; 32];
    hasher.finalize(&mut res);

    H256::from(res)
}

fn append_recursive(elem: MmrElem, new_leaf: MmrElem, leaf_number: u64) -> MmrElem {
    if leaf_number.is_power_of_two() {
        let hash = hash_children(&elem, &new_leaf);

        let difficulty = elem.difficulty + new_leaf.difficulty;

        let mut new_node = MmrElem {
            hash,
            difficulty,
            children: None,
        };

        new_node.children = Some((Box::new(elem), Box::new(new_leaf)));

        new_node
    } else {
        let MmrElem {
            hash: _hash,
            difficulty: _difficulty,
            children,
        } = elem;

        let (child0, child1) = children.expect("No children to unwrap");
        let leading_zeros = leaf_number.leading_zeros();
        let msb = 64 - leading_zeros - 1;
        let value = 2u64.pow(msb);
        let leaf_number = leaf_number - value;
        let child1 = append_recursive(*child1, new_leaf, leaf_number);

        let hash = hash_children(&child0, &child1);
        let difficulty = child0.difficulty + child1.difficulty;

        MmrElem {
            hash,
            difficulty,
            children: Some((child0, Box::new(child1))),
        }
    }
}
