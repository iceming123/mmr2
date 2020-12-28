use ethereum_types::{H256, U128, U256};
use fly_eth::{eth_u128_to_u128, u256_to_u128};
use fly_eth::{NonInteractiveProofVariableDifficulty, Proof};
use flyclient_ethereum as fly_eth;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;

use csv;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SuccinctBlockHeader {
    number: U128,
    hash: H256,
    difficulty: U256,
}

fn load_headers(nb: u128) -> Vec<SuccinctBlockHeader> {
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

        if curr_number >= nb {
            break;
        }
    }

    headers
}

const LAMBDA: u64 = 50;
const C: u64 = 50;
const L: usize = 200;

#[test]
fn proof_size() {
    let nonintproof = NonInteractiveProofVariableDifficulty::new(LAMBDA, C);

    println!("Load headers");
    let headers = load_headers(7000000);
    println!("{} headers loaded", headers.len());

    let mut mmr =
        fly_eth::MerkleTree::new(headers[0].hash, u256_to_u128(headers[0].difficulty), None);

    //TODO: als erst einmal nur eine proof size berechnen (7mio blöcke), dann alle dazwischen

    for i in 1..(headers.len() - L) {
        mmr.append_leaf(headers[i].hash, u256_to_u128(headers[i].difficulty));
    }

    let right_difficulty = headers
        .iter()
        .skip(headers.len() - L)
        .map(|header| u256_to_u128(header.difficulty))
        .sum();

    println!("create proof");
    let (mut proof, mut required_blocks, _) =
        nonintproof.create_new_proof(&mut mmr, right_difficulty);

    required_blocks.extend(
        headers[headers.len() - L..]
            .iter()
            .map(|header| eth_u128_to_u128(header.number) as u64),
    );

    //let proof_string = serde_json::to_string(&proof).unwrap();
    //let serialized_proof = proof_string.as_bytes();

    let serialized_proof = proof.serialize();

    println!(
        "complete difficulty of mmr: {}",
        proof.get_root_difficulty()
    );
    println!("Proof size: {} Kilobyte", serialized_proof.len() / 1024);
    println!("Number of required blocks: {}", required_blocks.len());

    // complete_size consists of mmr proof, the block headerser, where the assumption of 508bytes
    // per header is made * number of headers
    let complete_size = serialized_proof.len() + 508 * required_blocks.len();
    println!("Complete proof size: {} Kilobyte", complete_size / 1024);

    //let proof_response = serde_json::from_str::<Proof>(&proof_string).unwrap();
    let proof_response = Proof::deserialize(&serialized_proof).unwrap();

    println!("verify proof");

    println!("verify required blocks");
    let blocks_with_weight = nonintproof
        .verify_required_blocks(
            &required_blocks[..required_blocks.len() - L],
            proof_response.get_root_hash(),
            proof_response.get_root_difficulty(),
            right_difficulty,
            mmr.get_leaf_number(),
        )
        .expect("verify required blocks failed");

    if let Err(err) = Proof::verify_proof(&mut proof, blocks_with_weight) {
        println!("Error: {}", err);
        assert!(false);
    }
}
