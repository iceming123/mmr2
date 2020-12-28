use ethereum_types::{H256, U128, U256};
use fly_eth::{eth_u128_to_u128, u256_to_u128};
use fly_eth::{InMemoryMerkleTree, MerkleTree, Proof, ProofBlock};
use flyclient_ethereum as fly_eth;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;

use csv;
use rand::{thread_rng, Rng};

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

    let f = File::open("block_headers.csv").unwrap();
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

#[test]
fn fuzzy() {
    let mut rng = thread_rng();

    const I_MAX: usize = 200;

    for i in 1..=I_MAX {
        let headers = load_headers(i as u128);

        let mut mmr = MerkleTree::<InMemoryMerkleTree>::new(
            headers[0].hash,
            u256_to_u128(headers[0].difficulty),
            None,
        );

        for j in 1..i {
            mmr.append_leaf(headers[j].hash, u256_to_u128(headers[j].difficulty));
        }

        for j in 1..i {
            println!("------------------- new turn ----------------");
            println!("i: {}/{}, j: {}/{}", i, I_MAX, j, i);
            let mut block_numbers: Vec<u64> = vec![];
            for _ in 1..=j {
                let random = rng.gen_range(1, j + 1);
                block_numbers.push(random as u64);
            }

            let proof_blocks = block_numbers
                .iter()
                .map(|block| ProofBlock {
                    number: *block,
                    aggr_weight: Some(mmr.get_difficulty_relation(*block)),
                })
                .collect();

            let blocks = block_numbers[..].to_vec();
            let mut proof = Proof::generate_proof(&mut mmr, &mut block_numbers);

            let result = Proof::verify_proof(&mut proof, proof_blocks);

            if let Err(ref err) = result {
                println!("Headers: {:?}", headers);
                println!("Block numbers: {:?}", blocks);
                println!("Error: {}", err);
            }
            assert!(result.is_ok());
        }
    }
}
