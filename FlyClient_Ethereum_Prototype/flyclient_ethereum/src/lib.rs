use ethereum_types::{H256, U128, U256};
use std::cmp::Ordering;
use std::io::{self, Error, ErrorKind};
use tiny_keccak::Keccak;

pub mod mmr;
pub mod request_response;

pub use mmr::persistent_mmr::{InMemoryMerkleTree, MerkleTree};
pub use mmr::proof::Proof;

#[derive(Debug)]
pub struct ProofBlock {
    pub number: u64,
    pub aggr_weight: Option<f64>,
}

impl ProofBlock {
    pub fn new(number: u64, aggr_weight: Option<f64>) -> Self {
        ProofBlock {
            number,
            aggr_weight,
        }
    }
}

impl PartialEq for ProofBlock {
    fn eq(&self, other: &ProofBlock) -> bool {
        self.number == other.number
    }
}

impl Eq for ProofBlock {}

impl PartialOrd for ProofBlock {
    fn partial_cmp(&self, other: &ProofBlock) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ProofBlock {
    fn cmp(&self, other: &ProofBlock) -> Ordering {
        self.number.cmp(&other.number)
    }
}

pub struct NonInteractiveProofVariableDifficulty {
    lambda: u64, // security parameter, attacker succeeds with probability < 2^(-lambda)
    c: f64,      // hash power fraction of the adversary compared to the honest hashing power
}

impl NonInteractiveProofVariableDifficulty {
    pub fn new(lambda: u64, c_percentage: u64) -> Self {
        // c_percentage=50 means c=0.5
        if c_percentage == 0 || c_percentage > 99 {
            panic!("Invalid c");
        }
        let c = c_percentage as f64 / 100.0;
        NonInteractiveProofVariableDifficulty { lambda, c }
    }

    pub fn create_new_proof(
        &self,
        mut mmr: &mut MerkleTree<InMemoryMerkleTree>,
        right_difficulty: u128,
    ) -> (Proof, Vec<u64>, Vec<u64>) {
        let root_hash = mmr.get_root_hash().clone();

        let required_queries = (vd_calculate_m(
            self.lambda as f64,
            self.c,
            mmr.get_leaf_number(),
            right_difficulty as f64,
            (mmr.get_root_difficulty() + right_difficulty) as f64,
        ) + 1.0) as u64;

        let mut weights = vec![];
        let mut blocks = vec![];

        for i in 0..required_queries {
            let random = h256_to_f64(generate_new_hash(root_hash, i));
            let aggr_weight = cdf(
                random,
                vd_calculate_delta(right_difficulty as f64, mmr.get_root_difficulty() as f64),
            );
            weights.push(aggr_weight);
        }
        weights.sort_by(|a, b| a.partial_cmp(b).unwrap());

        for weight in weights {
            let block = mmr.get_child_by_aggr_weight(weight);
            blocks.push(block);
        }

        // Pick up at specific sync point
        // Add extra blocks, which are used for syncing from an already available state
        // 1. block : first block of current 30_000 block interval
        // 2. block : first block of previous 30_000 block interval
        // 3. block : first block of third last 30_000 block interaval
        // 4. block : first block of fourth last 30_000 block interval
        // 5. block : first block of fiftf last 30_000 block interval
        // 6. block : first block of sixth last 30_000 block interval
        // 7. block : first block of seventh last 30_000 block interval
        // 8. block : first block of eighth last 30_000 block interval
        // 9. block : first block of ninth last 30_000 block interval
        // 10. block: first block of tenth last 30_000 block interval

        let mut extra_blocks = vec![];

        let mut current_block = ((mmr.get_leaf_number() - 1) / 30000) * 30000;
        let mut added = 0;
        while current_block > 30000 && added < 10 {
            blocks.push(current_block);
            extra_blocks.push(current_block);
            current_block -= 30000;
            added += 1;
        }

        let mut blocks_dup = blocks.clone();
        blocks_dup.sort();
        let proof = Proof::generate_proof(&mut mmr, &mut blocks);

        (proof, blocks_dup, extra_blocks)
    }

    pub fn verify_required_blocks(
        &self,
        blocks: &[u64],
        root_hash: H256,
        root_difficulty: u128,
        right_difficulty: u128,
        root_leaf_number: u64,
    ) -> io::Result<Vec<ProofBlock>> {
        let required_queries = (vd_calculate_m(
            self.lambda as f64,
            self.c,
            root_leaf_number,
            right_difficulty as f64,
            (root_difficulty + right_difficulty) as f64,
        ) + 1.0) as u64;

        let mut extra_blocks = vec![];
        let mut current_block = ((root_leaf_number - 1) / 30000) * 30000;
        let mut added = 0;
        while current_block > 30000 && added < 10 {
            extra_blocks.push(current_block);
            current_block -= 30000;
            added += 1;
        }

        // required queries can contain the same block number multiple times
        // TODO: maybe multiple blocks can be pruned away?
        if required_queries != (blocks.len() - extra_blocks.len()) as u64 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "false number of blocks provided: required: {}, got: {}",
                    required_queries,
                    blocks.len()
                ),
            ));
        }

        let mut weights = vec![];

        for i in 0..required_queries {
            let random = h256_to_f64(generate_new_hash(root_hash, i));
            let aggr_weight = cdf(
                random,
                vd_calculate_delta(right_difficulty as f64, root_difficulty as f64),
            );
            weights.push(aggr_weight);
        }

        weights.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let mut weights_iter = weights.iter();
        let proof_blocks = blocks
            .iter()
            .map(|number| {
                let aggr_weight = if extra_blocks.len() > 0 {
                    let curr_extra_block = extra_blocks[extra_blocks.len() - 1];

                    if *number == curr_extra_block {
                        extra_blocks.remove(extra_blocks.len() - 1);
                        None
                    } else {
                        Some(*weights_iter.next().unwrap())
                    }
                } else {
                    Some(*weights_iter.next().unwrap())
                };

                ProofBlock::new(*number, aggr_weight)
            })
            .collect();

        Ok(proof_blocks)
    }
}

// delta in variable difficulty setting is the sum of difficulty checked with probability 1 in the
// end
fn vd_calculate_delta(block_difficulty: f64, total_difficulty: f64) -> f64 {
    block_difficulty / total_difficulty
}

// calculate how many independent queries m are required to have the specified security of lambda
// and always check the last specified block difficulty manually in variable difficulty setting
fn vd_calculate_m(
    lambda: f64,
    c: f64,
    n: u64,
    block_difficulty: f64,
    total_difficulty: f64,
) -> f64 {
    let numerator = -lambda - (c * (n as f64)).log2();

    let x = 1.0 - (1.0 / (log_b_of_x(c, block_difficulty / total_difficulty)));
    let x = if x.is_sign_negative() { 0.0 } else { x }; // x is not allowed to be negative
    let denumerator = x.log2();

    numerator / denumerator
}

// calculate logarithm of x for base b:
//
// y = log_2(x)/log_2(b)
//
fn log_b_of_x(b: f64, x: f64) -> f64 {
    x.log2() / b.log2()
}

//
//              1
// g(x) = --------------
//        (x-1)ln(delta)
//
pub fn pdf(x: f64, delta: f64) -> f64 {
    1.0 / ((x - 1.0) * delta.ln())
}

//
//             y(ln(delta))
// f(y) = 1 - e
//
// The cdf takes into account, that the last delta blocks are manually checked
pub fn cdf(y: f64, delta: f64) -> f64 {
    1.0 - (y * delta.ln()).exp()
}

// TODO: make this conversion unnecessary, because it panics if the number is too high
pub fn u256_to_u128(x: U256) -> u128 {
    let mut buf = [0; 32];
    x.to_little_endian(&mut buf);
    let mut buf2 = [0; 16];
    buf2[0..16].copy_from_slice(&buf[0..16]);
    let val = u128::from_le_bytes(buf2);
    val
}

pub fn eth_u128_to_u128(x: U128) -> u128 {
    let mut buf = [0; 16];
    x.to_little_endian(&mut buf);
    u128::from_le_bytes(buf)
}

fn bytes_to_f64(bytes: [u8; 8]) -> f64 {
    f64::from_bits(u64::from_be_bytes(bytes))
}

// extract bits from the hash value to generate an f64 value
// this is used for extracting randomness from hash values
// https://news.ycombinator.com/item?id=9207874
// http://www.math.sci.hiroshima-u.ac.jp/~m-mat/MT/ARTICLES/dSFMT.pdf
pub fn h256_to_f64(x: H256) -> f64 {
    let bytes: [u8; 32] = x.to_fixed_bytes();
    let byte0 = 63u8; // bitmuster to ensure that f64 is between 0 and 1
    let byte1 = bytes[1] | 240u8; // bitmuster to ensure that f64 is between 0 and 1
    bytes_to_f64([
        byte0, byte1, bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]) - 1.0
}

fn generate_new_hash(hash: H256, number: u64) -> H256 {
    let mut hasher = Keccak::new_sha3_256();

    hasher.update(hash.as_bytes());
    hasher.update(&number.to_be_bytes());

    let mut res: [u8; 32] = [0; 32];
    hasher.finalize(&mut res);

    H256::from(res)
}
