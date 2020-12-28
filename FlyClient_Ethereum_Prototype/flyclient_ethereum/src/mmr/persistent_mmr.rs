use std::fs::File;
use std::fs::OpenOptions;
use std::io;
use std::io::prelude::*;
use std::io::ErrorKind;
use std::io::SeekFrom;

use ethereum_types::H256;
use tiny_keccak::Keccak;

pub trait Datastore: Sized {
    fn load_tree(file_name: &str) -> io::Result<(Self, u64)>;
    fn new(hash: H256, difficulty: u128, file_name: Option<&str>) -> Self;
    fn get_elem_by_position(&mut self, byte_position: u64) -> MmrElem; //pos=0 reads 1. elem, pos=48 -> 2.
    fn get_elem(&mut self, node_number: u64) -> MmrElem;
    fn write_elem(&mut self, mmr_elem: &mut MmrElem) -> io::Result<()>;
    fn remove_last_elem(&mut self) -> io::Result<()>;
    fn get_storage_size(&mut self) -> u64;
    fn get_root_elem(&mut self) -> MmrElem;

    fn get_len(&mut self) -> u64 {
        self.get_storage_size() / 48
    }

    fn print_view(&mut self) {
        let len = self.get_storage_size();
        let mut curr_position = 0;
        let mut curr_node = 0;

        println!();
        while curr_position < len {
            let elem = self.get_elem_by_position(curr_position);
            println!("\t[ID: {}, {}]", curr_node, &elem.view());
            curr_position += 48;
            curr_node += 1;
        }
        println!();
    }

    fn close(&mut self, leaf_number: u64);
}

pub struct FileBasedMerkleTree {
    f: File,
}

impl Datastore for FileBasedMerkleTree {
    fn new(hash: H256, difficulty: u128, file_name: Option<&str>) -> FileBasedMerkleTree {
        if file_name.is_none() {
            panic!("No filename is given");
        }

        let f = match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(file_name.unwrap())
        {
            Ok(f) => f,
            Err(err) => panic!("Could not create new FileBasedMerkleTree: {}", err),
        };

        f.set_len(48).unwrap();

        let mut tree = FileBasedMerkleTree { f };
        tree.write_elem(&mut MmrElem {
            hash,
            difficulty,
            position_in_datastore: None,
        })
        .unwrap();

        tree
    }

    fn get_elem(&mut self, node_number: u64) -> MmrElem {
        self.get_elem_by_position(node_number * 48)
    }

    fn get_elem_by_position(&mut self, mut byte_position: u64) -> MmrElem {
        byte_position += 48;

        if byte_position % 48 != 0 {
            panic!("Invalid read operation");
        }

        let mut buf = [0; 48];
        let mut buf32 = [0; 32];
        let mut buf16 = [0; 16];

        self.f
            .seek(SeekFrom::Start(byte_position))
            .expect("Seek in File did not succeed");
        self.f
            .read_exact(&mut buf)
            .expect("Read from File did not succeed");
        buf32.copy_from_slice(&buf[..32]);
        let hash = H256::from(buf32);
        buf16.copy_from_slice(&buf[32..]);
        let difficulty = u128::from_be_bytes(buf16);

        MmrElem {
            hash,
            difficulty,
            position_in_datastore: Some(byte_position - 48),
        }
    }

    fn write_elem(&mut self, mmr_elem: &mut MmrElem) -> io::Result<()> {
        let last_position = self.f.seek(SeekFrom::End(0))?;
        self.f.write_all(&mmr_elem.hash.to_fixed_bytes())?;
        self.f.write_all(&mmr_elem.difficulty.to_be_bytes())?;
        mmr_elem.position_in_datastore = Some(last_position - 48);
        Ok(())
    }

    fn remove_last_elem(&mut self) -> io::Result<()> {
        let last_position = self.f.seek(SeekFrom::End(0))?;
        if last_position <= 48 {
            panic!("There does not exist elements to remove");
        }
        self.f.set_len(last_position - 48)?;
        Ok(())
    }

    fn get_storage_size(&mut self) -> u64 {
        self.f.seek(SeekFrom::End(0)).unwrap() - 48
    }

    fn get_root_elem(&mut self) -> MmrElem {
        let last_position = self.f.seek(SeekFrom::End(0)).unwrap();
        self.get_elem_by_position(last_position - 96)
    }

    // Save current leaf number and write hash of mmr in file
    fn close(&mut self, leaf_number: u64) {
        self.f.seek(SeekFrom::Start(32)).unwrap();
        self.f.write_all(&leaf_number.to_be_bytes()).unwrap();
        self.f.seek(SeekFrom::Start(32)).unwrap();

        let mut hasher = Keccak::new_sha3_256();

        const BUF_SIZE: usize = 4096;
        let mut buffer = [0u8; BUF_SIZE];

        loop {
            let n = match self.f.read(&mut buffer) {
                Ok(n) => n,
                Err(_) => panic!("Could not read bytes from file to generate hash"),
            };
            hasher.update(&buffer[..n]);

            if n == 0 || n < BUF_SIZE {
                break;
            }
        }

        let mut res: [u8; 32] = [0; 32];
        hasher.finalize(&mut res);
        self.f.seek(SeekFrom::Start(0)).unwrap();
        self.f.write_all(&res).unwrap();
    }

    fn load_tree(file_name: &str) -> io::Result<(Self, u64)> {
        let mut f = match OpenOptions::new().read(true).write(true).open(file_name) {
            Ok(f) => f,
            Err(err) => {
                if err.kind() == ErrorKind::NotFound {
                    return Err(err);
                } else {
                    panic!("Could not load MMR file: {}", err);
                }
            }
        };

        let mut buf = [0u8; 32];
        f.read_exact(&mut buf).unwrap();
        let stored_hash = H256::from(buf);

        let mut hasher = Keccak::new_sha3_256();

        const BUF_SIZE: usize = 4096;
        let mut buffer = [0u8; BUF_SIZE];

        loop {
            let n = match f.read(&mut buffer) {
                Ok(n) => n,
                Err(_) => panic!("Could not read MMR from file"),
            };
            hasher.update(&buffer[..n]);

            if n == 0 || n < BUF_SIZE {
                break;
            }
        }

        let mut res: [u8; 32] = [0; 32];
        hasher.finalize(&mut res);
        let calculated_hash = H256::from(res);

        if calculated_hash != stored_hash {
            panic!(
                "Integrity of MMR file is violated: \n\texpected: {:?}, \n\tgot: {:?}",
                stored_hash, calculated_hash
            );
        }

        let mut buf = [0u8; 8];
        f.seek(SeekFrom::Start(32)).unwrap();
        f.read_exact(&mut buf).unwrap();
        let leaf_number = u64::from_be_bytes(buf);

        Ok((Self { f }, leaf_number))
    }
}

pub struct InMemoryMerkleTree {
    f: Option<File>,
    tree: Vec<u8>,
}

impl Datastore for InMemoryMerkleTree {
    fn new(hash: H256, difficulty: u128, file_name: Option<&str>) -> InMemoryMerkleTree {
        let f = if let Some(file) = file_name {
            let f = match OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(file)
            {
                Ok(f) => f,
                Err(err) => panic!("Could not create new FileBasedMerkleTree: {}", err),
            };

            f.set_len(48).unwrap();

            Some(f)
        } else {
            None
        };

        let mut in_memory_tree = InMemoryMerkleTree { f, tree: vec![] };

        in_memory_tree
            .write_elem(&mut MmrElem {
                hash,
                difficulty,
                position_in_datastore: None,
            })
            .unwrap();

        in_memory_tree
    }

    fn get_elem(&mut self, node_number: u64) -> MmrElem {
        self.get_elem_by_position(node_number * 48)
    }

    fn get_elem_by_position(&mut self, byte_position: u64) -> MmrElem {
        if byte_position % 48 != 0 {
            panic!("Invalid read operation");
        }

        let mut buf32 = [0; 32];
        buf32.copy_from_slice(&self.tree[byte_position as usize..byte_position as usize + 32]);
        let hash = H256::from(buf32);

        let mut buf16 = [0; 16];
        buf16.copy_from_slice(&self.tree[byte_position as usize + 32..byte_position as usize + 48]);
        let difficulty = u128::from_be_bytes(buf16);

        MmrElem {
            hash,
            difficulty,
            position_in_datastore: Some(byte_position),
        }
    }

    fn write_elem(&mut self, mmr_elem: &mut MmrElem) -> io::Result<()> {
        self.tree.extend(&mmr_elem.hash.to_fixed_bytes());
        self.tree.extend(&mmr_elem.difficulty.to_be_bytes());
        mmr_elem.position_in_datastore = Some(self.get_storage_size() - 48);
        Ok(())
    }

    fn remove_last_elem(&mut self) -> io::Result<()> {
        assert!(self.tree.len() != 0, "Cannot remove elem from empty vec!");

        unsafe {
            self.tree.set_len(self.tree.len() - 48);
        }
        Ok(())
    }

    fn get_storage_size(&mut self) -> u64 {
        self.tree.len() as u64
    }

    fn get_root_elem(&mut self) -> MmrElem {
        let max_size = self.get_storage_size() as usize;

        let mut buf32 = [0; 32];
        buf32.copy_from_slice(&self.tree[max_size - 48..max_size - 16]);
        let hash = H256::from(buf32);

        let mut buf16 = [0; 16];
        buf16.copy_from_slice(&self.tree[max_size - 16..max_size]);
        let difficulty = u128::from_be_bytes(buf16);

        MmrElem {
            hash,
            difficulty,
            position_in_datastore: Some(max_size as u64 - 48),
        }
    }

    fn close(&mut self, leaf_number: u64) {
        if let Some(ref mut f) = self.f {
            f.seek(SeekFrom::Start(32)).unwrap();
            f.write_all(&leaf_number.to_be_bytes()).unwrap();
            f.seek(SeekFrom::Start(48)).unwrap();
            f.write_all(&self.tree).unwrap();
            f.seek(SeekFrom::Start(32)).unwrap();

            let mut hasher = Keccak::new_sha3_256();

            const BUF_SIZE: usize = 4096;
            let mut buffer = [0u8; BUF_SIZE];

            loop {
                let n = match f.read(&mut buffer) {
                    Ok(n) => n,
                    Err(_) => panic!("Could not read bytes from file to generate hash"),
                };
                hasher.update(&buffer[..n]);

                if n == 0 || n < BUF_SIZE {
                    break;
                }
            }

            let mut res: [u8; 32] = [0; 32];
            hasher.finalize(&mut res);
            f.seek(SeekFrom::Start(0)).unwrap();
            f.write_all(&res).unwrap();
        }
    }

    fn load_tree(file_name: &str) -> io::Result<(Self, u64)> {
        let mut tree = vec![];

        let mut f = match OpenOptions::new().read(true).write(true).open(file_name) {
            Ok(f) => f,
            Err(err) => {
                if err.kind() == ErrorKind::NotFound {
                    return Err(err);
                } else {
                    panic!("Could not load MMR file: {}", err);
                }
            }
        };

        let mut buf = [0u8; 32];
        f.read_exact(&mut buf).unwrap();
        let stored_hash = H256::from(buf);

        let mut hasher = Keccak::new_sha3_256();

        const BUF_SIZE: usize = 4096;
        let mut buffer = [0u8; BUF_SIZE];

        let mut first_run = true; // to not store the leaf_number in Vec<u8>

        loop {
            let n = match f.read(&mut buffer) {
                Ok(n) => n,
                Err(_) => panic!("Could not read MMR from file"),
            };
            hasher.update(&buffer[..n]);

            if first_run {
                tree.extend(&buffer[16..n]);
                first_run = false;
            } else {
                tree.extend(&buffer[..n]);
            }

            if n == 0 || n < BUF_SIZE {
                break;
            }
        }

        let mut res: [u8; 32] = [0; 32];
        hasher.finalize(&mut res);
        let calculated_hash = H256::from(res);

        if calculated_hash != stored_hash {
            panic!("Integrity of MMR file is violated");
        }

        let mut buf = [0u8; 8];
        f.seek(SeekFrom::Start(32)).unwrap();
        f.read_exact(&mut buf).unwrap();
        let leaf_number = u64::from_be_bytes(buf);

        Ok((InMemoryMerkleTree { f: Some(f), tree }, leaf_number))
    }
}

#[derive(Debug)]
pub struct MmrElem {
    hash: H256,
    difficulty: u128,
    position_in_datastore: Option<u64>,
}

impl MmrElem {
    pub fn new(hash: H256, difficulty: u128) -> Self {
        MmrElem {
            hash,
            difficulty,
            position_in_datastore: None,
        }
    }

    fn serialize(&self) -> [u8; 48] {
        let mut slice = [0u8; 48];

        slice[0..32].copy_from_slice(self.hash.as_bytes());
        slice[32..48].copy_from_slice(&self.difficulty.to_be_bytes());

        slice
    }

    fn view(&self) -> String {
        format!("hash: {}, difficulty: {}", self.hash, self.difficulty)
    }

    pub fn get_hash(&self) -> H256 {
        self.hash
    }

    pub fn get_difficulty(&self) -> u128 {
        self.difficulty
    }

    pub fn has_children<T: Datastore>(&mut self, mmr: &mut MerkleTree<T>) -> bool {
        let elem_node_number = self.position_in_datastore.unwrap() / 48;
        let mut curr_root_node_number = mmr.datastore.get_storage_size() / 48;
        let mut aggr_node_number = 0;

        while curr_root_node_number > 2 {
            let leaf_number = node_to_leaf_number(curr_root_node_number);
            let left_tree_leaf_number = get_left_leaf_number(leaf_number);
            let left_tree_node_number = leaf_to_node_number(left_tree_leaf_number);

            if (aggr_node_number + curr_root_node_number) == (elem_node_number + 1) {
                return true;
            }

            if elem_node_number < (aggr_node_number + left_tree_node_number) {
                // branch left
                curr_root_node_number = left_tree_node_number;
            } else {
                // branch right
                curr_root_node_number = curr_root_node_number - left_tree_node_number - 1;
                aggr_node_number += left_tree_node_number;
            }
        }
        false
    }

    pub fn get_children<T: Datastore>(
        &mut self,
        mmr: &mut MerkleTree<T>,
    ) -> (Box<MmrElem>, Box<MmrElem>) {
        let elem_node_number = self.position_in_datastore.unwrap() / 48;
        let mut curr_root_node_number = mmr.datastore.get_storage_size() / 48;
        let mut aggr_node_number = 0;

        while curr_root_node_number > 2 {
            let leaf_number = node_to_leaf_number(curr_root_node_number);
            let left_tree_leaf_number = get_left_leaf_number(leaf_number);
            let left_tree_node_number = leaf_to_node_number(left_tree_leaf_number);

            if (aggr_node_number + curr_root_node_number) == (elem_node_number + 1) {
                let leaf_number = node_to_leaf_number(curr_root_node_number);
                let left_tree_leaf_number = get_left_leaf_number(leaf_number);
                let left_tree_node_number = leaf_to_node_number(left_tree_leaf_number);

                let left_node_position = aggr_node_number + left_tree_node_number - 1;
                let right_node_position = aggr_node_number + curr_root_node_number - 2;

                let left_elem = mmr.datastore.get_elem(left_node_position);
                let right_elem = mmr.datastore.get_elem(right_node_position);

                return (Box::new(left_elem), Box::new(right_elem));
            }

            if elem_node_number < (aggr_node_number + left_tree_node_number) {
                // branch left
                curr_root_node_number = left_tree_node_number;
            } else {
                // branch right
                curr_root_node_number = curr_root_node_number - left_tree_node_number - 1;
                aggr_node_number += left_tree_node_number;
            }
        }

        panic!("This node has no children!");
    }
}

// calc leaf number from complete node number
fn node_to_leaf_number(node_number: u64) -> u64 {
    (node_number + 1) / 2
}

fn leaf_to_node_number(leaf_number: u64) -> u64 {
    (2 * leaf_number) - 1
}

pub fn get_left_leaf_number(leaf_number: u64) -> u64 {
    if leaf_number.is_power_of_two() {
        leaf_number / 2
    } else {
        leaf_number.next_power_of_two() / 2
    }
}

pub struct MerkleTree<T: Datastore> {
    leaf_number: u64,
    datastore: T,
    // Every element in datastore is 48byte
}

impl<T: Datastore> Drop for MerkleTree<T> {
    fn drop(&mut self) {
        self.datastore.close(self.leaf_number);
    }
}

impl<T: Datastore> MerkleTree<T> {
    pub fn load(file_name: &str) -> io::Result<Self> {
        let (datastore, leaf_number) = T::load_tree(file_name)?;

        Ok(MerkleTree {
            leaf_number,
            datastore,
        })
    }

    pub fn new(hash: H256, difficulty: u128, file_name: Option<&str>) -> Self {
        MerkleTree {
            leaf_number: 1,
            datastore: T::new(hash, difficulty, file_name),
        }
    }

    pub fn get_leaf_number(&self) -> u64 {
        self.leaf_number
    }

    //TODO: noch unbenennen
    pub fn get_elem(&mut self) -> MmrElem {
        self.datastore.get_root_elem()
    }

    pub fn append_leaf(&mut self, hash: H256, difficulty: u128) {
        let mut new_elem = MmrElem {
            hash,
            difficulty,
            position_in_datastore: None,
        };

        let mut nodes_to_hash = vec![];
        let mut curr_tree_number = self.leaf_number;

        let mut aggr_node_number = 0;

        while !curr_tree_number.is_power_of_two() {
            self.datastore.remove_last_elem().unwrap();

            let left_tree_number = curr_tree_number.next_power_of_two() / 2;
            aggr_node_number += left_tree_number;
            let right_tree_number = curr_tree_number - left_tree_number;

            let left_root_node_number = get_node_number(aggr_node_number) - 1;
            nodes_to_hash.push(self.datastore.get_elem(left_root_node_number));
            curr_tree_number = right_tree_number;
        }
        nodes_to_hash.push(self.datastore.get_root_elem());
        self.datastore.write_elem(&mut new_elem).unwrap();
        nodes_to_hash.push(new_elem);

        while nodes_to_hash.len() > 1 {
            let curr_right_elem = nodes_to_hash.pop().unwrap();
            let curr_left_elem = nodes_to_hash.pop().unwrap();
            let hash = hash_children(&curr_left_elem, &curr_right_elem);
            let difficulty = curr_left_elem.difficulty + curr_right_elem.difficulty;
            let mut new_intermediate_node = MmrElem {
                hash,
                difficulty,
                position_in_datastore: None,
            };
            self.datastore
                .write_elem(&mut new_intermediate_node)
                .unwrap();
            nodes_to_hash.push(new_intermediate_node);
        }
        self.leaf_number += 1;
    }

    pub fn get_root_hash(&mut self) -> H256 {
        self.datastore.get_root_elem().hash
    }

    pub fn get_root_difficulty(&mut self) -> u128 {
        self.datastore.get_root_elem().difficulty
    }

    pub fn get_difficulty_relation(&mut self, leaf_number: u64) -> f64 {
        self.get_left_difficulty(leaf_number) as f64 / self.get_root_difficulty() as f64
    }

    pub fn get_left_difficulty(&mut self, leaf_number: u64) -> u128 {
        let mut aggr_weight = 0;
        let mut aggr_node_number = 0;
        let mut curr_tree_number = self.leaf_number;

        while aggr_node_number < leaf_number {
            let left_tree_number = if curr_tree_number.is_power_of_two() {
                curr_tree_number / 2
            } else {
                curr_tree_number.next_power_of_two() / 2
            };

            if leaf_number >= (aggr_node_number + left_tree_number) {
                // branch right
                aggr_node_number += left_tree_number;
                let left_root_node_number = get_node_number(aggr_node_number) - 1;
                aggr_weight += self.datastore.get_elem(left_root_node_number).difficulty;
                curr_tree_number = curr_tree_number - left_tree_number;
            } else {
                // branch left
                curr_tree_number = left_tree_number;
            }
        }

        aggr_weight
    }

    pub fn get_child_by_aggr_weight(&mut self, weight: f64) -> u64 {
        let root_weight = self.get_root_difficulty();
        let weight_disc = (root_weight as f64 * weight) as u128;

        self.get_child_by_aggr_weight_disc(weight_disc)
    }

    fn get_child_by_aggr_weight_disc(&mut self, weight: u128) -> u64 {
        let mut aggr_weight = 0;
        let mut aggr_node_number = 0;
        let mut curr_tree_number = self.leaf_number;

        while curr_tree_number > 1 {
            let left_tree_number = if curr_tree_number.is_power_of_two() {
                curr_tree_number / 2
            } else {
                curr_tree_number.next_power_of_two() / 2
            };

            let left_tree_difficulty = self
                .datastore
                .get_elem(get_node_number(aggr_node_number + left_tree_number) - 1)
                .difficulty;
            if weight >= (aggr_weight + left_tree_difficulty) {
                // branch right
                aggr_node_number += left_tree_number;
                let left_root_node_number = get_node_number(aggr_node_number) - 1;
                aggr_weight += self.datastore.get_elem(left_root_node_number).difficulty;
                curr_tree_number = curr_tree_number - left_tree_number;
            } else {
                // branch left
                curr_tree_number = left_tree_number;
            }
        }

        aggr_node_number
    }
}

pub fn hash_children(left: &MmrElem, right: &MmrElem) -> H256 {
    let mut hasher = Keccak::new_sha3_256();

    hasher.update(&left.serialize());
    hasher.update(&right.serialize());

    let mut res: [u8; 32] = [0; 32];
    hasher.finalize(&mut res);

    H256::from(res)
}

// retrieve node number for specific leaf number
fn get_node_number(leaf_number: u64) -> u64 {
    let mut position = 0;
    let mut remaining = leaf_number;

    while remaining != 0 {
        let left_tree_leaf_number = if remaining.is_power_of_two() {
            remaining
        } else {
            remaining.next_power_of_two() / 2
        };
        position += left_tree_leaf_number + left_tree_leaf_number - 1;
        remaining = remaining - left_tree_leaf_number;
    }

    position
}

#[cfg(test)]
mod tests {
    use super::get_node_number;
    use super::Datastore;
    use super::InMemoryMerkleTree;
    use super::MerkleTree;
    use ethereum_types::H256;

    fn delete_mmr_file(file_name: &str) {
        use std::fs;
        use std::io::ErrorKind;
        if let Err(err) = fs::remove_file(file_name) {
            if err.kind() != ErrorKind::NotFound {
                panic!("Error: {}", err);
            }
        }
    }

    #[test]
    fn test_get_node_number() {
        assert!(get_node_number(0) == 0);
        assert!(get_node_number(1) == 1);
        assert!(get_node_number(2) == 3);
        assert!(get_node_number(3) == 4);
        assert!(get_node_number(4) == 7);
        assert!(get_node_number(5) == 8);
        assert!(get_node_number(6) == 10);
        assert!(get_node_number(7) == 11);
        assert!(get_node_number(8) == 15);
        assert!(get_node_number(9) == 16);
        assert!(get_node_number(10) == 18);
        assert!(get_node_number(11) == 19);
        assert!(get_node_number(12) == 22);
        assert!(get_node_number(13) == 23);
    }

    #[test]
    fn test_deletion() {
        //mmr has not to be valid again, deletion should only delete last element
        let mut mmr = MerkleTree::<InMemoryMerkleTree>::new(H256::from_low_u64_be(0), 0, None);
        mmr.append_leaf(H256::from_low_u64_be(1), 1);
        assert_eq!(mmr.datastore.get_len(), 3);
        mmr.datastore.remove_last_elem().unwrap();
        assert_eq!(mmr.datastore.get_len(), 2);
    }

    #[test]
    fn test_mmr() {
        let mut mmr = MerkleTree::<InMemoryMerkleTree>::new(H256::from_low_u64_be(0), 0, None);

        assert_eq!(mmr.get_leaf_number(), 1);
        assert_eq!(mmr.get_root_difficulty(), 0);

        mmr.append_leaf(H256::from_low_u64_be(0), 1);

        assert_eq!(mmr.get_leaf_number(), 2);
        assert_eq!(mmr.get_root_difficulty(), 1);

        mmr.append_leaf(H256::from_low_u64_be(0), 2);

        assert_eq!(mmr.get_leaf_number(), 3);
        assert_eq!(mmr.get_root_difficulty(), 3);

        mmr.append_leaf(H256::from_low_u64_be(0), 3);

        assert_eq!(mmr.get_leaf_number(), 4);
        assert_eq!(mmr.get_root_difficulty(), 6);

        mmr.append_leaf(H256::from_low_u64_be(0), 4);

        assert_eq!(mmr.get_leaf_number(), 5);
        assert_eq!(mmr.get_root_difficulty(), 10);

        mmr.append_leaf(H256::from_low_u64_be(0), 5);

        assert_eq!(mmr.get_leaf_number(), 6);
        assert_eq!(mmr.get_root_difficulty(), 15);

        mmr.append_leaf(H256::from_low_u64_be(0), 6);

        assert_eq!(mmr.get_leaf_number(), 7);
        assert_eq!(mmr.get_root_difficulty(), 21);

        mmr.append_leaf(H256::from_low_u64_be(0), 7);

        assert_eq!(mmr.get_leaf_number(), 8);
        assert_eq!(mmr.get_root_difficulty(), 28);

        mmr.append_leaf(H256::from_low_u64_be(0), 8);

        assert_eq!(mmr.get_leaf_number(), 9);
        assert_eq!(mmr.get_root_difficulty(), 36);

        mmr.append_leaf(H256::from_low_u64_be(0), 9);

        assert_eq!(mmr.get_leaf_number(), 10);
        assert_eq!(mmr.get_root_difficulty(), 45);

        mmr.append_leaf(H256::from_low_u64_be(0), 10);

        assert_eq!(mmr.get_leaf_number(), 11);
        assert_eq!(mmr.get_root_difficulty(), 55);

        mmr.append_leaf(H256::from_low_u64_be(0), 11);

        assert_eq!(mmr.get_leaf_number(), 12);
        assert_eq!(mmr.get_root_difficulty(), 66);

        mmr.append_leaf(H256::from_low_u64_be(0), 12);

        assert_eq!(mmr.get_leaf_number(), 13);
        assert_eq!(mmr.get_root_difficulty(), 78);

        mmr.append_leaf(H256::from_low_u64_be(0), 13);

        assert_eq!(mmr.get_leaf_number(), 14);
        assert_eq!(mmr.get_root_difficulty(), 91);

        mmr.append_leaf(H256::from_low_u64_be(0), 14);

        assert_eq!(mmr.get_leaf_number(), 15);
        assert_eq!(mmr.get_root_difficulty(), 105);

        mmr.append_leaf(H256::from_low_u64_be(0), 15);

        assert_eq!(mmr.get_leaf_number(), 16);
        assert_eq!(mmr.get_root_difficulty(), 120);

        mmr.append_leaf(H256::from_low_u64_be(0), 16);

        assert_eq!(mmr.get_leaf_number(), 17);
        assert_eq!(mmr.get_root_difficulty(), 136);

        mmr.append_leaf(H256::from_low_u64_be(0), 17);

        assert_eq!(mmr.get_leaf_number(), 18);
        assert_eq!(mmr.get_root_difficulty(), 153);

        assert_eq!(mmr.get_left_difficulty(0), 0);
        assert_eq!(mmr.get_left_difficulty(1), 0);
        assert_eq!(mmr.get_left_difficulty(2), 1);
        assert_eq!(mmr.get_left_difficulty(3), 3);
        assert_eq!(mmr.get_left_difficulty(4), 6);
        assert_eq!(mmr.get_left_difficulty(5), 10);
        assert_eq!(mmr.get_left_difficulty(6), 15);
        assert_eq!(mmr.get_left_difficulty(7), 21);
        assert_eq!(mmr.get_left_difficulty(8), 28);
        assert_eq!(mmr.get_left_difficulty(9), 36);
        assert_eq!(mmr.get_left_difficulty(10), 45);
        assert_eq!(mmr.get_left_difficulty(11), 55);
        assert_eq!(mmr.get_left_difficulty(12), 66);
        assert_eq!(mmr.get_left_difficulty(13), 78);
        assert_eq!(mmr.get_left_difficulty(14), 91);
        assert_eq!(mmr.get_left_difficulty(15), 105);
        assert_eq!(mmr.get_left_difficulty(16), 120);
        assert_eq!(mmr.get_left_difficulty(17), 136);

        assert_eq!(mmr.get_child_by_aggr_weight_disc(0), 1);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(1), 2);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(2), 2);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(3), 3);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(4), 3);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(5), 3);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(6), 4);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(7), 4);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(8), 4);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(9), 4);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(10), 5);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(11), 5);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(12), 5);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(13), 5);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(14), 5);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(15), 6);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(16), 6);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(17), 6);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(18), 6);

        assert_eq!(mmr.get_child_by_aggr_weight_disc(78), 13);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(91), 14);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(105), 15);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(120), 16);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(135), 16);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(136), 17);

        let mut node0 = mmr.datastore.get_elem(0);
        let mut node1 = mmr.datastore.get_elem(1);
        let mut node2 = mmr.datastore.get_elem(2);
        let mut node3 = mmr.datastore.get_elem(3);
        let mut node4 = mmr.datastore.get_elem(4);
        let mut node5 = mmr.datastore.get_elem(5);
        let mut node6 = mmr.datastore.get_elem(6);
        let mut node7 = mmr.datastore.get_elem(7);
        let mut node8 = mmr.datastore.get_elem(8);
        let mut node9 = mmr.datastore.get_elem(9);
        let mut node10 = mmr.datastore.get_elem(10);
        let mut node11 = mmr.datastore.get_elem(11);
        let mut node12 = mmr.datastore.get_elem(12);
        let mut node13 = mmr.datastore.get_elem(13);
        let mut node14 = mmr.datastore.get_elem(14);
        let mut node15 = mmr.datastore.get_elem(15);
        let mut node16 = mmr.datastore.get_elem(16);
        let mut node17 = mmr.datastore.get_elem(17);
        let mut node18 = mmr.datastore.get_elem(18);
        let mut node19 = mmr.datastore.get_elem(19);
        let mut node20 = mmr.datastore.get_elem(20);
        let mut node21 = mmr.datastore.get_elem(21);
        let mut node22 = mmr.datastore.get_elem(22);
        let mut node23 = mmr.datastore.get_elem(23);
        let mut node24 = mmr.datastore.get_elem(24);
        let mut node25 = mmr.datastore.get_elem(25);
        let mut node26 = mmr.datastore.get_elem(26);
        let mut node27 = mmr.datastore.get_elem(27);
        let mut node28 = mmr.datastore.get_elem(28);
        let mut node29 = mmr.datastore.get_elem(29);
        let mut node30 = mmr.datastore.get_elem(30);
        let mut node31 = mmr.datastore.get_elem(31);
        let mut node32 = mmr.datastore.get_elem(32);
        let mut node33 = mmr.datastore.get_elem(33);
        let mut node34 = mmr.datastore.get_elem(34);

        assert!(!node0.has_children(&mut mmr));
        assert!(!node1.has_children(&mut mmr));
        assert!(node2.has_children(&mut mmr));
        assert!(!node3.has_children(&mut mmr));
        assert!(!node4.has_children(&mut mmr));
        assert!(node5.has_children(&mut mmr));
        assert!(node6.has_children(&mut mmr));
        assert!(!node7.has_children(&mut mmr));
        assert!(!node8.has_children(&mut mmr));
        assert!(node9.has_children(&mut mmr));
        assert!(!node10.has_children(&mut mmr));
        assert!(!node11.has_children(&mut mmr));
        assert!(node12.has_children(&mut mmr));
        assert!(node13.has_children(&mut mmr));
        assert!(node14.has_children(&mut mmr));
        assert!(!node15.has_children(&mut mmr));
        assert!(!node16.has_children(&mut mmr));
        assert!(node17.has_children(&mut mmr));
        assert!(!node18.has_children(&mut mmr));
        assert!(!node19.has_children(&mut mmr));
        assert!(node20.has_children(&mut mmr));
        assert!(node21.has_children(&mut mmr));
        assert!(!node22.has_children(&mut mmr));
        assert!(!node23.has_children(&mut mmr));
        assert!(node24.has_children(&mut mmr));
        assert!(!node25.has_children(&mut mmr));
        assert!(!node26.has_children(&mut mmr));
        assert!(node27.has_children(&mut mmr));
        assert!(node28.has_children(&mut mmr));
        assert!(node29.has_children(&mut mmr));
        assert!(node30.has_children(&mut mmr));
        assert!(!node31.has_children(&mut mmr));
        assert!(!node32.has_children(&mut mmr));
        assert!(node33.has_children(&mut mmr));
        assert!(node34.has_children(&mut mmr));

        let children = node2.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 0);
        assert_eq!(children.1.difficulty, 1);
        let children = node5.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 2);
        assert_eq!(children.1.difficulty, 3);
        let children = node6.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 1);
        assert_eq!(children.1.difficulty, 5);
        let children = node9.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 4);
        assert_eq!(children.1.difficulty, 5);
        let children = node12.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 6);
        assert_eq!(children.1.difficulty, 7);
        let children = node13.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 9);
        assert_eq!(children.1.difficulty, 13);
        let children = node14.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 6);
        assert_eq!(children.1.difficulty, 22);
        let children = node17.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 8);
        assert_eq!(children.1.difficulty, 9);
        let children = node20.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 10);
        assert_eq!(children.1.difficulty, 11);
        let children = node21.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 17);
        assert_eq!(children.1.difficulty, 21);
        let children = node24.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 12);
        assert_eq!(children.1.difficulty, 13);
        let children = node27.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 14);
        assert_eq!(children.1.difficulty, 15);
        let children = node28.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 25);
        assert_eq!(children.1.difficulty, 29);
        let children = node29.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 38);
        assert_eq!(children.1.difficulty, 54);
        let children = node30.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 28);
        assert_eq!(children.1.difficulty, 92);
        let children = node33.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 16);
        assert_eq!(children.1.difficulty, 17);
        let children = node34.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 120);
        assert_eq!(children.1.difficulty, 33);
    }

    use super::FileBasedMerkleTree;

    #[test]
    fn test_deletion_file() {
        delete_mmr_file("test_deletion_file.bin");
        let mut mmr = MerkleTree::<FileBasedMerkleTree>::new(
            H256::from_low_u64_be(0),
            0,
            Some("test_deletion_file.bin"),
        );
        mmr.append_leaf(H256::from_low_u64_be(1), 1);
        assert_eq!(mmr.datastore.get_len(), 3);
        mmr.datastore.remove_last_elem().unwrap();
        assert_eq!(mmr.datastore.get_len(), 2);
    }

    #[test]
    fn test_mmr_file() {
        delete_mmr_file("test_mmr_file.bin");
        let mut mmr = MerkleTree::<FileBasedMerkleTree>::new(
            H256::from_low_u64_be(0),
            0,
            Some("test_mmr_file.bin"),
        );

        assert_eq!(mmr.get_leaf_number(), 1);
        assert_eq!(mmr.get_root_difficulty(), 0);

        mmr.append_leaf(H256::from_low_u64_be(0), 1);

        assert_eq!(mmr.get_leaf_number(), 2);
        assert_eq!(mmr.get_root_difficulty(), 1);

        mmr.append_leaf(H256::from_low_u64_be(0), 2);

        assert_eq!(mmr.get_leaf_number(), 3);
        assert_eq!(mmr.get_root_difficulty(), 3);

        mmr.append_leaf(H256::from_low_u64_be(0), 3);

        assert_eq!(mmr.get_leaf_number(), 4);
        assert_eq!(mmr.get_root_difficulty(), 6);

        mmr.append_leaf(H256::from_low_u64_be(0), 4);

        assert_eq!(mmr.get_leaf_number(), 5);
        assert_eq!(mmr.get_root_difficulty(), 10);

        mmr.append_leaf(H256::from_low_u64_be(0), 5);

        assert_eq!(mmr.get_leaf_number(), 6);
        assert_eq!(mmr.get_root_difficulty(), 15);

        mmr.append_leaf(H256::from_low_u64_be(0), 6);

        assert_eq!(mmr.get_leaf_number(), 7);
        assert_eq!(mmr.get_root_difficulty(), 21);

        mmr.append_leaf(H256::from_low_u64_be(0), 7);

        assert_eq!(mmr.get_leaf_number(), 8);
        assert_eq!(mmr.get_root_difficulty(), 28);

        mmr.append_leaf(H256::from_low_u64_be(0), 8);

        assert_eq!(mmr.get_leaf_number(), 9);
        assert_eq!(mmr.get_root_difficulty(), 36);

        mmr.append_leaf(H256::from_low_u64_be(0), 9);

        assert_eq!(mmr.get_leaf_number(), 10);
        assert_eq!(mmr.get_root_difficulty(), 45);

        mmr.append_leaf(H256::from_low_u64_be(0), 10);

        assert_eq!(mmr.get_leaf_number(), 11);
        assert_eq!(mmr.get_root_difficulty(), 55);

        mmr.append_leaf(H256::from_low_u64_be(0), 11);

        assert_eq!(mmr.get_leaf_number(), 12);
        assert_eq!(mmr.get_root_difficulty(), 66);

        mmr.append_leaf(H256::from_low_u64_be(0), 12);

        assert_eq!(mmr.get_leaf_number(), 13);
        assert_eq!(mmr.get_root_difficulty(), 78);

        mmr.append_leaf(H256::from_low_u64_be(0), 13);

        assert_eq!(mmr.get_leaf_number(), 14);
        assert_eq!(mmr.get_root_difficulty(), 91);

        mmr.append_leaf(H256::from_low_u64_be(0), 14);

        assert_eq!(mmr.get_leaf_number(), 15);
        assert_eq!(mmr.get_root_difficulty(), 105);

        mmr.append_leaf(H256::from_low_u64_be(0), 15);

        assert_eq!(mmr.get_leaf_number(), 16);
        assert_eq!(mmr.get_root_difficulty(), 120);

        mmr.append_leaf(H256::from_low_u64_be(0), 16);

        assert_eq!(mmr.get_leaf_number(), 17);
        assert_eq!(mmr.get_root_difficulty(), 136);

        mmr.append_leaf(H256::from_low_u64_be(0), 17);

        assert_eq!(mmr.get_leaf_number(), 18);
        assert_eq!(mmr.get_root_difficulty(), 153);

        assert_eq!(mmr.get_left_difficulty(0), 0);
        assert_eq!(mmr.get_left_difficulty(1), 0);
        assert_eq!(mmr.get_left_difficulty(2), 1);
        assert_eq!(mmr.get_left_difficulty(3), 3);
        assert_eq!(mmr.get_left_difficulty(4), 6);
        assert_eq!(mmr.get_left_difficulty(5), 10);
        assert_eq!(mmr.get_left_difficulty(6), 15);
        assert_eq!(mmr.get_left_difficulty(7), 21);
        assert_eq!(mmr.get_left_difficulty(8), 28);
        assert_eq!(mmr.get_left_difficulty(9), 36);
        assert_eq!(mmr.get_left_difficulty(10), 45);
        assert_eq!(mmr.get_left_difficulty(11), 55);
        assert_eq!(mmr.get_left_difficulty(12), 66);
        assert_eq!(mmr.get_left_difficulty(13), 78);
        assert_eq!(mmr.get_left_difficulty(14), 91);
        assert_eq!(mmr.get_left_difficulty(15), 105);
        assert_eq!(mmr.get_left_difficulty(16), 120);
        assert_eq!(mmr.get_left_difficulty(17), 136);

        assert_eq!(mmr.get_child_by_aggr_weight_disc(0), 1);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(1), 2);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(2), 2);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(3), 3);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(4), 3);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(5), 3);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(6), 4);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(7), 4);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(8), 4);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(9), 4);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(10), 5);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(11), 5);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(12), 5);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(13), 5);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(14), 5);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(15), 6);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(16), 6);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(17), 6);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(18), 6);

        assert_eq!(mmr.get_child_by_aggr_weight_disc(78), 13);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(91), 14);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(105), 15);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(120), 16);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(135), 16);
        assert_eq!(mmr.get_child_by_aggr_weight_disc(136), 17);

        let mut node0 = mmr.datastore.get_elem(0);
        let mut node1 = mmr.datastore.get_elem(1);
        let mut node2 = mmr.datastore.get_elem(2);
        let mut node3 = mmr.datastore.get_elem(3);
        let mut node4 = mmr.datastore.get_elem(4);
        let mut node5 = mmr.datastore.get_elem(5);
        let mut node6 = mmr.datastore.get_elem(6);
        let mut node7 = mmr.datastore.get_elem(7);
        let mut node8 = mmr.datastore.get_elem(8);
        let mut node9 = mmr.datastore.get_elem(9);
        let mut node10 = mmr.datastore.get_elem(10);
        let mut node11 = mmr.datastore.get_elem(11);
        let mut node12 = mmr.datastore.get_elem(12);
        let mut node13 = mmr.datastore.get_elem(13);
        let mut node14 = mmr.datastore.get_elem(14);
        let mut node15 = mmr.datastore.get_elem(15);
        let mut node16 = mmr.datastore.get_elem(16);
        let mut node17 = mmr.datastore.get_elem(17);
        let mut node18 = mmr.datastore.get_elem(18);
        let mut node19 = mmr.datastore.get_elem(19);
        let mut node20 = mmr.datastore.get_elem(20);
        let mut node21 = mmr.datastore.get_elem(21);
        let mut node22 = mmr.datastore.get_elem(22);
        let mut node23 = mmr.datastore.get_elem(23);
        let mut node24 = mmr.datastore.get_elem(24);
        let mut node25 = mmr.datastore.get_elem(25);
        let mut node26 = mmr.datastore.get_elem(26);
        let mut node27 = mmr.datastore.get_elem(27);
        let mut node28 = mmr.datastore.get_elem(28);
        let mut node29 = mmr.datastore.get_elem(29);
        let mut node30 = mmr.datastore.get_elem(30);
        let mut node31 = mmr.datastore.get_elem(31);
        let mut node32 = mmr.datastore.get_elem(32);
        let mut node33 = mmr.datastore.get_elem(33);
        let mut node34 = mmr.datastore.get_elem(34);

        assert!(!node0.has_children(&mut mmr));
        assert!(!node1.has_children(&mut mmr));
        assert!(node2.has_children(&mut mmr));
        assert!(!node3.has_children(&mut mmr));
        assert!(!node4.has_children(&mut mmr));
        assert!(node5.has_children(&mut mmr));
        assert!(node6.has_children(&mut mmr));
        assert!(!node7.has_children(&mut mmr));
        assert!(!node8.has_children(&mut mmr));
        assert!(node9.has_children(&mut mmr));
        assert!(!node10.has_children(&mut mmr));
        assert!(!node11.has_children(&mut mmr));
        assert!(node12.has_children(&mut mmr));
        assert!(node13.has_children(&mut mmr));
        assert!(node14.has_children(&mut mmr));
        assert!(!node15.has_children(&mut mmr));
        assert!(!node16.has_children(&mut mmr));
        assert!(node17.has_children(&mut mmr));
        assert!(!node18.has_children(&mut mmr));
        assert!(!node19.has_children(&mut mmr));
        assert!(node20.has_children(&mut mmr));
        assert!(node21.has_children(&mut mmr));
        assert!(!node22.has_children(&mut mmr));
        assert!(!node23.has_children(&mut mmr));
        assert!(node24.has_children(&mut mmr));
        assert!(!node25.has_children(&mut mmr));
        assert!(!node26.has_children(&mut mmr));
        assert!(node27.has_children(&mut mmr));
        assert!(node28.has_children(&mut mmr));
        assert!(node29.has_children(&mut mmr));
        assert!(node30.has_children(&mut mmr));
        assert!(!node31.has_children(&mut mmr));
        assert!(!node32.has_children(&mut mmr));
        assert!(node33.has_children(&mut mmr));
        assert!(node34.has_children(&mut mmr));

        let children = node2.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 0);
        assert_eq!(children.1.difficulty, 1);
        let children = node5.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 2);
        assert_eq!(children.1.difficulty, 3);
        let children = node6.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 1);
        assert_eq!(children.1.difficulty, 5);
        let children = node9.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 4);
        assert_eq!(children.1.difficulty, 5);
        let children = node12.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 6);
        assert_eq!(children.1.difficulty, 7);
        let children = node13.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 9);
        assert_eq!(children.1.difficulty, 13);
        let children = node14.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 6);
        assert_eq!(children.1.difficulty, 22);
        let children = node17.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 8);
        assert_eq!(children.1.difficulty, 9);
        let children = node20.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 10);
        assert_eq!(children.1.difficulty, 11);
        let children = node21.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 17);
        assert_eq!(children.1.difficulty, 21);
        let children = node24.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 12);
        assert_eq!(children.1.difficulty, 13);
        let children = node27.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 14);
        assert_eq!(children.1.difficulty, 15);
        let children = node28.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 25);
        assert_eq!(children.1.difficulty, 29);
        let children = node29.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 38);
        assert_eq!(children.1.difficulty, 54);
        let children = node30.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 28);
        assert_eq!(children.1.difficulty, 92);
        let children = node33.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 16);
        assert_eq!(children.1.difficulty, 17);
        let children = node34.get_children(&mut mmr);
        assert_eq!(children.0.difficulty, 120);
        assert_eq!(children.1.difficulty, 33);
    }

    #[test]
    fn test_mmr_sync() {
        const FILE_NAME: &str = "test_mmr_sync.bin";
        delete_mmr_file(FILE_NAME);

        {
            let mut mmr =
                MerkleTree::<InMemoryMerkleTree>::new(H256::from_low_u64_be(0), 0, Some(FILE_NAME));
            mmr.append_leaf(H256::from_low_u64_be(0), 1);
            mmr.append_leaf(H256::from_low_u64_be(0), 2);
            mmr.append_leaf(H256::from_low_u64_be(0), 3);
            mmr.append_leaf(H256::from_low_u64_be(0), 4);

            assert_eq!(mmr.get_leaf_number(), 5);
            assert_eq!(mmr.get_root_difficulty(), 10);
        }

        {
            let mut mmr = MerkleTree::<FileBasedMerkleTree>::load(FILE_NAME).unwrap();

            assert_eq!(mmr.get_leaf_number(), 5);
            assert_eq!(mmr.get_root_difficulty(), 10);

            mmr.append_leaf(H256::from_low_u64_be(0), 5);
            mmr.append_leaf(H256::from_low_u64_be(0), 6);

            assert_eq!(mmr.get_leaf_number(), 7);
            assert_eq!(mmr.get_root_difficulty(), 21);
        }

        {
            let mut mmr = MerkleTree::<InMemoryMerkleTree>::load(FILE_NAME).unwrap();

            assert_eq!(mmr.get_leaf_number(), 7);
            assert_eq!(mmr.get_root_difficulty(), 21);
        }
    }
}
