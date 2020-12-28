use csv;
use ethereum_types::{H256, U128, U256};
use fly_eth::{eth_u128_to_u128, u256_to_u128};
use fly_eth::{NonInteractiveProofVariableDifficulty, Proof};
use flyclient_ethereum as fly_eth;
use serde::{Deserialize, Serialize};

use std::fs::File;
use std::io::BufReader;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SuccinctBlockHeader {
    number: U128,
    hash: H256,
    difficulty: U256,
}

fn load_headers(nb: Option<u128>) -> Vec<SuccinctBlockHeader> {
    let mut headers = vec![];
    let mut curr_number = 0;
    let one = 1;

    let f = File::open("../light_server/block_headers.csv").unwrap();
    let f = BufReader::new(f);
    let mut rdr = csv::Reader::from_reader(f);

    for result in rdr.deserialize() {
        let record: SuccinctBlockHeader = result.unwrap();
        assert!(eth_u128_to_u128(record.number) == curr_number);
        headers.push(record);

        curr_number += one;

        if nb.is_some() {
            if curr_number >= nb.unwrap() {
                break;
            }
        }
    }

    headers
}

const LAMBDA: u64 = 50;
const C: u64 = 50;
const L: usize = 200;

#[test]
fn test0() {
    let nonintproof = NonInteractiveProofVariableDifficulty::new(LAMBDA, C);

    println!("load headers");
    let headers = load_headers(None);

    println!("create MMR");
    let mut mmr =
        fly_eth::MerkleTree::new(headers[0].hash, u256_to_u128(headers[0].difficulty), None);
    for j in 1..(headers.len() - L) {
        mmr.append_leaf(headers[j].hash, u256_to_u128(headers[j].difficulty));
    }

    let right_difficulty = headers
        .iter()
        .skip(headers.len() - L)
        .map(|header| u256_to_u128(header.difficulty))
        .sum();

    println!("create proof");
    let (mut proof, required_blocks, _) = nonintproof.create_new_proof(&mut mmr, right_difficulty);

    println!("verify required blocks");

    let blocks_with_weight = nonintproof
        .verify_required_blocks(
            &required_blocks,
            proof.get_root_hash(),
            proof.get_root_difficulty(),
            right_difficulty,
            mmr.get_leaf_number(),
        )
        .expect("verify required blocks failed");

    println!("verify proof");

    if let Err(err) = Proof::verify_proof(&mut proof, blocks_with_weight) {
        println!("Error: {}", err);
        assert!(false);
    }
}
