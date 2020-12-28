use ethereum_types::{H256, U128, U256};
use fly_eth::{eth_u128_to_u128, u256_to_u128};
use fly_eth::{NonInteractiveProofVariableDifficulty, Proof};
use flyclient_ethereum as fly_eth;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, BufReader, BufWriter};

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

fn write_csv(data: Vec<(usize, usize, usize, usize)>) -> io::Result<()> {
    let f = File::create("proof_size.csv").unwrap();
    let f = BufWriter::new(f);
    let mut wtr = csv::Writer::from_writer(f);

    // When writing records without Serde, the header record is written just
    // like any other record.
    wtr.write_record(&["block_number", "proof_size", "blocks_queried", "L"])?;

    for line in data {
        wtr.write_record(&[
            line.0.to_string(),
            line.1.to_string(),
            line.2.to_string(),
            line.3.to_string(),
        ])?;
    }
    wtr.flush()?;
    Ok(())
}

const LAMBDA: u64 = 50;
const C: u64 = 50;
const L: usize = 100;
fn main() -> io::Result<()> {
    println!("Measure proof size");

    let step_size = 10000;

    let mut proof_sizes = vec![]; // (x,y) coordinates

    let nonintproof = NonInteractiveProofVariableDifficulty::new(LAMBDA, C);

    println!("Load headers");
    let headers = load_headers(7000000);
    println!("{} headers loaded", headers.len());

    // vor L blöcke nur die n*508 ausrechnen, weil noch kein proof gemacht wird
    // danach wird der proof auch noch hinzuaddiert

    let mut nb = 0;
    while nb <= L {
        println!("nb: {}, size: {}", nb, 508 * nb);

        proof_sizes.push((nb, 508 * nb / 1024, nb, 0));
        nb += step_size;
    }

    let mut mmr =
        fly_eth::MerkleTree::new(headers[0].hash, u256_to_u128(headers[0].difficulty), None);

    nb = L + 1;
    while nb < (headers.len() - step_size) {
        let right_difficulty = headers
            .iter()
            .skip(mmr.get_leaf_number() as usize)
            .take(nb - mmr.get_leaf_number() as usize)
            .map(|header| u256_to_u128(header.difficulty))
            .sum();

        let (mut proof, mut required_blocks, _) =
            nonintproof.create_new_proof(&mut mmr, right_difficulty);

        let nb_blocks = required_blocks.len();

        required_blocks.extend(
            headers
                .iter()
                .skip(mmr.get_leaf_number() as usize)
                .take(nb - mmr.get_leaf_number() as usize)
                .map(|header| eth_u128_to_u128(header.number) as u64),
        );

        let serialized_proof = proof.serialize();

        // complete_size consists of mmr proof, the block headerser, where the assumption of 508bytes
        // per header is made * number of headers
        let complete_size = serialized_proof.len() + 508 * required_blocks.len();
        println!("nb: {}, size: {}", nb, complete_size);

        proof_sizes.push((nb, complete_size / 1024, nb_blocks, L));

        let proof_response = Proof::deserialize(&serialized_proof).unwrap();

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

        for _ in 0..step_size {
            // erst am ende wird ein leaf appended
            mmr.append_leaf(
                headers[mmr.get_leaf_number() as usize].hash,
                u256_to_u128(headers[mmr.get_leaf_number() as usize].difficulty),
            );
        }

        nb += step_size;
    }

    println!("Write csv...");
    write_csv(proof_sizes)
}
