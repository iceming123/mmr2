use csv;
use ethereum_types::{H256, U128, U256};
use flyclient_ethereum as fly_eth;
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SuccinctBlockHeader {
    number: U128,
    hash: H256,
    difficulty: U256,
}

fn load_headers(nb: Option<usize>) -> Vec<SuccinctBlockHeader> {
    let mut headers = vec![];
    let mut curr_number = U128::from(0);
    let one = U128::from(1);

    let f = File::open("../light_server/block_headers.csv").unwrap();
    let f = BufReader::new(f);
    let mut rdr = csv::Reader::from_reader(f);

    for result in rdr.deserialize() {
        let record: SuccinctBlockHeader = result.unwrap();
        assert!(record.number == curr_number);
        headers.push(record);

        curr_number += one;

        if nb.is_some() {
            if curr_number >= U128::from(nb.unwrap()) {
                break;
            }
        }
    }

    headers
}

#[test]
fn test_random_generator() {
    let headers = load_headers(None);
    let mut values = Vec::with_capacity(headers.len());
    // [0, 0.1), [0.1, 0.2), [0.2, 0.3), [0.3, 0.4), [0.4, 0.5), [0.5, 0.6), [0.6, 0.7), [0.7,
    // 0.8), [0.8, 0.9), [0.9, 1)
    let mut distribution = vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    println!("Check {} hash values", headers.len());
    for header in headers {
        let rng = fly_eth::h256_to_f64(header.hash);
        values.push(rng);
        distribution[(rng * 10.0) as usize] += 1;
        assert!(rng <= 1.0);
        assert!(rng >= 0.0);
    }
    println!("distribution: {:?}", distribution);
}

#[test]
fn test_random_generator_2() {
    let headers = load_headers(None);
    let nb = headers.len();
    let mut values = Vec::with_capacity(nb);
    let mut distribution = vec![0.; 100];
    for header in headers {
        let rng = fly_eth::h256_to_f64(header.hash);
        values.push(rng);
        distribution[(rng * 100.0) as usize] += 100.0 / nb as f64;
    }
    println!("Values: {:?}", distribution);

    let max = *distribution
        .iter()
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap() as usize
        + 5;

    println!("\nDistribution, which should be uniformly distributed:");
    for i in (0..max).rev() {
        for j in 0..100 {
            if distribution[j] > i as f64 {
                print!("o");
            } else {
                print!(" ");
            }
        }
        println!();
    }
}

#[test]
fn test_value_distribution() {
    let l = 100.0;
    let n = 10000000.0;

    let mut distribution = vec![0.0; 100];

    let mut rng = rand::thread_rng();
    for _ in 0..100000 {
        let random = rng.gen();
        let continuous_block = fly_eth::cdf(random, l / n);
        distribution[(continuous_block * 100.0) as usize] += 0.001;
    }
    println!("Values: {:?}", distribution);

    let max = *distribution
        .iter()
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap() as usize
        + 5;

    println!("\nDistribution, which should have the most values on the far-right side:");
    for i in (0..max).rev() {
        for j in 0..100 {
            if distribution[j] > i as f64 {
                print!("o");
            } else {
                print!(" ");
            }
        }
        println!();
    }
}
