use std::io::{self, prelude::*, Error, ErrorKind};
use std::net::{Shutdown, TcpStream};
use std::time::Instant;

use flyclient_ethereum::request_response::{Request, Response};
use flyclient_ethereum::NonInteractiveProofVariableDifficulty;

use env_logger;
use rayon::prelude::*;
use structopt::StructOpt;

use common_types::header::Header;

mod pow_verifier;

const LAMBDA: u64 = 50;
const C: u64 = 50;
const L: u64 = 100;
const ETHASH_EPOCH_LENGTH: u64 = 30000;

#[derive(StructOpt, Debug)]
enum Opt {
    #[structopt(name = "latest_block_number")]
    LatestBlockNumber {
        /// Url of the light server to connect to
        #[structopt(short = "l", long = "light-server", default_value = "localhost:7878")]
        light_server: String,
    },

    #[structopt(name = "blockheader")]
    BlockHeader {
        /// Block number to fetch
        number: u64,

        /// Url of the light server to connect to
        #[structopt(short = "l", long = "light-server", default_value = "localhost:7878")]
        light_server: String,
    },

    #[structopt(name = "proof")]
    NonInteractiveProof {
        /// Url of the light server to connect to
        #[structopt(short = "l", long = "light-server", default_value = "localhost:7878")]
        light_server: String,
    },

    #[structopt(name = "measurements")]
    MeasurementMode {
        /// Url of the light server to connect to
        #[structopt(short = "l", long = "light-server", default_value = "localhost:7878")]
        light_server: String,
    },

    #[structopt(name = "measureContinuation")]
    MeasurementModeContinuation {
        /// Url of the light server to connect to
        #[structopt(short = "l", long = "light-server", default_value = "localhost:7878")]
        light_server: String,
    },

    #[structopt(name = "measureTradeoff")]
    MeasureTradeoff {
        /// Url of the light server to connect to
        #[structopt(short = "l", long = "light-server", default_value = "localhost:7878")]
        light_server: String,
    },
}

fn main() -> io::Result<()> {
    env_logger::init();

    let opt = Opt::from_args();

    match opt {
        Opt::LatestBlockNumber { light_server } => {
            println!(
                "Latest Block Number: {}",
                get_lastest_block_number(&light_server)?
            );
        }
        Opt::BlockHeader {
            number,
            light_server,
        } => {
            get_block_by_number(number, &light_server)?;
        }
        Opt::NonInteractiveProof { light_server } => {
            get_non_interactive_proof(&light_server)?;
        }
        Opt::MeasurementMode { light_server } => {
            measurement(&light_server)?;
        }
        Opt::MeasurementModeContinuation { light_server } => {
            measure_continuation(&light_server)?;
        }
        Opt::MeasureTradeoff { light_server } => {
            measure_tradeoff(&light_server)?;
        }
    }
    Ok(())
}

fn get_block_by_number(number: u64, light_server: &str) -> io::Result<()> {
    let req = Request::BlockHeader(number);
    let (resp, _) = query_server(req, light_server)?;

    let block = if let Response::BlockHeader(v) = resp {
        v
    } else {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "Got unexpected response",
        ));
    };

    println!("Block: {:?}", &block);
    println!("Hash: {:?}", &block.hash());

    Ok(())
}

fn get_lastest_block_number(light_server: &str) -> io::Result<(u64)> {
    let req = Request::LatestBlockNumber;
    let (resp, _) = query_server(req, light_server)?;

    let v = if let Response::LatestBlockNumber(v) = resp {
        v
    } else {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "Got unexpected response",
        ));
    };

    Ok(v)
}

fn measure_tradeoff(light_server: &str) -> io::Result<()> {
    println!("Measure tradeoff");

    let step_size = 100;

    let mut measurements = vec![]; // (block number, validation time, proof size)

    measurements.push((0 as usize, 0, 0, 0, 0, 0));

    let mut l = step_size;

    while l <= 1000 {
        println!("l: {}", l);

        let mut send_buffer = vec![];
        send_buffer.extend_from_slice(&(7000000 as u64).to_be_bytes());
        send_buffer.extend_from_slice(&(l as u64).to_be_bytes());

        let (resp, buffer_length) = query_server_by_bytes(&send_buffer, light_server)?;

        let (blocks, mut proof, _, right_difficulty, last_blocks) =
            if let Response::NonInteractiveProof(variables) = resp {
                variables
            } else {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "Got unexpected response",
                ));
            };

        let start = Instant::now();

        let proof_blocks: Vec<u64> = blocks.iter().map(|block| block.number()).collect();

        println!("Blocks: {:?}", proof_blocks);

        let start_proof_verification = Instant::now();

        // check if server sent correct blocks and not arbitrarily chosen one
        let non_interactive_proof = NonInteractiveProofVariableDifficulty::new(LAMBDA, C);

        let blocks_with_weight = non_interactive_proof.verify_required_blocks(
            &proof_blocks[..proof_blocks.len() - l as usize],
            proof.get_root_hash(),
            proof.get_root_difficulty(),
            right_difficulty,
            proof.get_leaf_number(),
        )?;

        let end_proof_verification = Instant::now();

        let mut proof_verification_time = end_proof_verification
            .duration_since(start_proof_verification)
            .as_secs();

        println!("Verify hash and proof of work of all blocks!");
        let start_pow_verification = Instant::now();
        let epoch_number = verify_pow(&blocks, &last_blocks)?;
        let end_pow_verification = Instant::now();
        println!(
            "PoW verification time: {} seconds",
            end_pow_verification
                .duration_since(start_pow_verification)
                .as_secs()
        );

        let start_proof_verification = Instant::now();
        proof
            .verify_proof(blocks_with_weight)
            .map_err(|err| Error::new(ErrorKind::InvalidData, err))?;

        let end_proof_verification = Instant::now();

        proof_verification_time += end_proof_verification
            .duration_since(start_proof_verification)
            .as_secs();

        println!("Proof is valid!");
        println!("Complete proof size: {} kilobyte", buffer_length / 1024);
        let end = Instant::now();
        println!(
            "Verification time: {} seconds",
            end.duration_since(start).as_secs()
        );
        measurements.push((
            l as usize,
            proof_verification_time,
            end.duration_since(start).as_secs(),
            buffer_length / 1024,
            epoch_number,
            blocks.len() + last_blocks.len(),
        ));

        l = l + step_size;
    }

    println!("Write measurements to csv: {:?}", measurements);
    write_csv(measurements)
}

fn measure_continuation(light_server: &str) -> io::Result<()> {
    println!("Measure sync continuation");

    let mut measurements = vec![]; // (block number, validation time, proof size)

    let req = Request::NonInteractiveProof(LAMBDA, C, L);

    let (resp, buffer_length) = query_server(req, light_server)?;

    let (blocks, mut proof, l, right_difficulty, last_blocks) =
        if let Response::NonInteractiveProof(variables) = resp {
            variables
        } else {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Got unexpected response",
            ));
        };

    let start = Instant::now();

    let proof_blocks: Vec<u64> = blocks.iter().map(|block| block.number()).collect();

    println!("Blocks: {:?}", proof_blocks);

    let start_proof_verification = Instant::now();

    // check if server sent correct blocks and not arbitrarily chosen one
    let non_interactive_proof = NonInteractiveProofVariableDifficulty::new(LAMBDA, C);

    let blocks_with_weight = non_interactive_proof.verify_required_blocks(
        &proof_blocks[..proof_blocks.len() - l as usize],
        proof.get_root_hash(),
        proof.get_root_difficulty(),
        right_difficulty,
        proof.get_leaf_number(),
    )?;

    let end_proof_verification = Instant::now();

    let mut proof_verification_time = end_proof_verification
        .duration_since(start_proof_verification)
        .as_secs();

    println!("Verify hash and proof of work of all blocks!");
    let start_pow_verification = Instant::now();
    let epoch_number = verify_pow(&blocks, &last_blocks)?;
    let end_pow_verification = Instant::now();
    println!(
        "PoW verification time: {} seconds",
        end_pow_verification
            .duration_since(start_pow_verification)
            .as_secs()
    );

    let start_proof_verification = Instant::now();
    proof
        .verify_proof(blocks_with_weight)
        .map_err(|err| Error::new(ErrorKind::InvalidData, err))?;
    let end_proof_verification = Instant::now();

    proof_verification_time += end_proof_verification
        .duration_since(start_proof_verification)
        .as_secs();

    println!("Proof is valid!");
    println!("Complete proof size: {} kilobyte", buffer_length / 1024);
    let end = Instant::now();
    println!(
        "Verification time: {} seconds",
        end.duration_since(start).as_secs()
    );
    measurements.push((
        0,
        proof_verification_time,
        end.duration_since(start).as_secs(),
        buffer_length / 1024,
        epoch_number,
        blocks.len() + last_blocks.len(),
    ));

    let mut extra_blocks = vec![];

    let mut current_block = ((proof.get_leaf_number() - 1) / 30000) * 30000;
    let mut added = 0;
    while current_block > 30000 && added < 10 {
        extra_blocks.push(current_block);
        current_block -= 30000;
        added += 1;
    }

    extra_blocks.reverse();
    println!("Extra blocks: {:?}", extra_blocks);
    for extra_block in extra_blocks {
        let req = Request::ContinueNonInteractiveProof(LAMBDA, C, L, extra_block);

        let (resp, buffer_length) = query_server(req, light_server)?;

        let (omitted_blocks, blocks, mut proof, l, right_difficulty, last_blocks) =
            if let Response::ContinueNonInteractiveProof(variables) = resp {
                variables
            } else {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "Got unexpected response",
                ));
            };

        let start = Instant::now();

        let proof_blocks: Vec<u64> = omitted_blocks
            .iter()
            .map(|nb| *nb)
            .chain(blocks.iter().map(|block| block.number()))
            .collect();

        println!("Blocks: {:?}", proof_blocks);

        let start_proof_verification = Instant::now();

        // check if server sent correct blocks and not arbitrarily chosen one
        let non_interactive_proof = NonInteractiveProofVariableDifficulty::new(LAMBDA, C);

        let blocks_with_weight = non_interactive_proof.verify_required_blocks(
            &proof_blocks[..proof_blocks.len() - l as usize],
            proof.get_root_hash(),
            proof.get_root_difficulty(),
            right_difficulty,
            proof.get_leaf_number(),
        )?;

        let end_proof_verification = Instant::now();

        let mut proof_verification_time = end_proof_verification
            .duration_since(start_proof_verification)
            .as_secs();

        println!("Verify hash and proof of work of all blocks!");
        let start_pow_verification = Instant::now();
        let epoch_number = verify_pow(&blocks, &last_blocks)?;
        let end_pow_verification = Instant::now();
        println!(
            "PoW verification time: {} seconds",
            end_pow_verification
                .duration_since(start_pow_verification)
                .as_secs()
        );

        let start_proof_verification = Instant::now();
        proof
            .verify_proof(blocks_with_weight)
            .map_err(|err| Error::new(ErrorKind::InvalidData, err))?;
        let end_proof_verification = Instant::now();

        proof_verification_time += end_proof_verification
            .duration_since(start_proof_verification)
            .as_secs();

        println!("Proof is valid!");
        println!("Complete proof size: {} kilobyte", buffer_length / 1024);
        let end = Instant::now();
        println!(
            "Verification time: {} seconds",
            end.duration_since(start).as_secs()
        );
        measurements.push((
            extra_block as usize,
            proof_verification_time,
            end.duration_since(start).as_secs(),
            buffer_length / 1024,
            epoch_number,
            blocks.len() + last_blocks.len(),
        ));
    }

    write_csv(measurements)
}

fn measurement(light_server: &str) -> io::Result<()> {
    let step_size = 1000000;

    let mut measurements = vec![]; // (block number, validation time, proof size)

    measurements.push((0 as usize, 0, 0, 0, 0, 0));

    let mut block_number = step_size;

    while block_number <= 7000000 {
        println!("Block number: {}", block_number);

        let mut send_buffer = vec![];
        send_buffer.extend_from_slice(&(block_number as u64).to_be_bytes());
        send_buffer.extend_from_slice(&L.to_be_bytes());

        let (resp, buffer_length) = query_server_by_bytes(&send_buffer, light_server)?;

        let (blocks, mut proof, l, right_difficulty, last_blocks) =
            if let Response::NonInteractiveProof(variables) = resp {
                variables
            } else {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "Got unexpected response",
                ));
            };

        let start = Instant::now();

        let proof_blocks: Vec<u64> = blocks.iter().map(|block| block.number()).collect();

        println!("Blocks: {:?}", proof_blocks);

        let start_proof_verification = Instant::now();

        // check if server sent correct blocks and not arbitrarily chosen one
        let non_interactive_proof = NonInteractiveProofVariableDifficulty::new(LAMBDA, C);

        let blocks_with_weight = non_interactive_proof.verify_required_blocks(
            &proof_blocks[..proof_blocks.len() - l as usize],
            proof.get_root_hash(),
            proof.get_root_difficulty(),
            right_difficulty,
            proof.get_leaf_number(),
        )?;

        let end_proof_verification = Instant::now();

        let mut proof_verification_time = end_proof_verification
            .duration_since(start_proof_verification)
            .as_secs();

        println!("Verify hash and proof of work of all blocks!");
        let start_pow_verification = Instant::now();
        let epoch_number = verify_pow(&blocks, &last_blocks)?;
        let end_pow_verification = Instant::now();
        println!(
            "PoW verification time: {} seconds",
            end_pow_verification
                .duration_since(start_pow_verification)
                .as_secs()
        );

        let start_proof_verification = Instant::now();
        proof
            .verify_proof(blocks_with_weight)
            .map_err(|err| Error::new(ErrorKind::InvalidData, err))?;
        let end_proof_verification = Instant::now();

        proof_verification_time += end_proof_verification
            .duration_since(start_proof_verification)
            .as_secs();

        println!("Proof is valid!");
        println!("Complete proof size: {} kilobyte", buffer_length / 1024);
        let end = Instant::now();
        println!(
            "Verification time: {} seconds",
            end.duration_since(start).as_secs()
        );
        measurements.push((
            block_number,
            proof_verification_time,
            end.duration_since(start).as_secs(),
            buffer_length / 1024,
            epoch_number,
            blocks.len() + last_blocks.len(),
        ));

        block_number = block_number + step_size;
    }

    println!("Write measurements to csv: {:?}", measurements);
    write_csv(measurements)
}

fn get_non_interactive_proof(light_server: &str) -> io::Result<()> {
    let req = Request::NonInteractiveProof(LAMBDA, C, L);

    let (resp, buffer_length) = query_server(req, light_server)?;

    let (blocks, mut proof, l, right_difficulty, last_blocks) =
        if let Response::NonInteractiveProof(variables) = resp {
            variables
        } else {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Got unexpected response",
            ));
        };

    let start = Instant::now();

    let proof_blocks: Vec<u64> = blocks.iter().map(|block| block.number()).collect();

    println!("Blocks: {:?}", proof_blocks);

    // check if server sent correct blocks and not arbitrarily chosen one
    let non_interactive_proof = NonInteractiveProofVariableDifficulty::new(LAMBDA, C);

    let blocks_with_weight = non_interactive_proof.verify_required_blocks(
        &proof_blocks[..proof_blocks.len() - l as usize],
        proof.get_root_hash(),
        proof.get_root_difficulty(),
        right_difficulty,
        proof.get_leaf_number(),
    )?;

    println!("Verify hash and proof of work of all blocks!");
    let start_pow_verification = Instant::now();
    verify_pow(&blocks, &last_blocks)?;
    let end_pow_verification = Instant::now();
    println!(
        "PoW verification time: {} seconds",
        end_pow_verification
            .duration_since(start_pow_verification)
            .as_secs()
    );

    proof
        .verify_proof(blocks_with_weight)
        .map_err(|err| Error::new(ErrorKind::InvalidData, err))?;
    println!("Proof is valid!");
    println!("Complete proof size: {} kilobyte", buffer_length / 1024);
    let end = Instant::now();
    println!(
        "Verification time: {} seconds",
        end.duration_since(start).as_secs()
    );
    return Ok(());
}

fn verify_pow(proof_blocks: &Vec<Header>, last_blocks: &Vec<Header>) -> io::Result<usize> {
    let mut prev_epoch = None;
    let mut epoch_blocks = vec![];
    for block in proof_blocks.iter().chain(last_blocks.iter()) {
        let block_epoch = block.number() / ETHASH_EPOCH_LENGTH;

        let mut blocks = match prev_epoch {
            None => vec![],
            Some(nb) if nb != block_epoch => vec![],
            Some(_) => epoch_blocks.pop().unwrap(),
        };
        blocks.push(block);
        epoch_blocks.push(blocks);
        prev_epoch = Some(block_epoch);
    }

    epoch_blocks
        .par_iter()
        .try_for_each(|blocks| pow_verifier::pow_verify(blocks))?;

    Ok(epoch_blocks.len())
}

fn parse_response(buffer: &[u8]) -> io::Result<Response> {
    match Response::deserialize(&buffer) {
        Ok(resp) => Ok(resp),
        Err(err) => Err(Error::new(
            ErrorKind::InvalidData,
            format!("Could not deserialize response: {}", err),
        )),
    }
}

fn query_server_by_bytes(to_send: &Vec<u8>, addr: &str) -> io::Result<(Response, usize)> {
    let mut stream = TcpStream::connect(addr)?;

    let _ = stream.write_all(to_send)?;
    let _ = stream.flush()?;
    let _ = stream.shutdown(Shutdown::Write)?;

    let mut buffer = vec![];
    stream.read_to_end(&mut buffer).unwrap();

    let response = parse_response(&buffer)?;

    if let Response::Error(err) = response {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!("Got error from server: {}", err),
        ));
    }

    Ok((response, buffer.len()))
}

fn query_server(request: Request, addr: &str) -> io::Result<(Response, usize)> {
    query_server_by_bytes(&request.serialize(), addr)
}

fn write_csv(data: Vec<(usize, u64, u64, usize, usize, usize)>) -> io::Result<()> {
    use std::fs::File;
    use std::io::BufWriter;

    let f = File::create("measurements.csv").unwrap();
    let f = BufWriter::new(f);
    let mut wtr = csv::Writer::from_writer(f);

    // When writing records without Serde, the header record is written just
    // like any other record.
    wtr.write_record(&[
        "block_number",
        "proof_validation_time",
        "complete_validation_time",
        "complete_proof_size",
        "epoch_numbers",
        "required_blocks",
    ])?;

    for line in data {
        wtr.write_record(&[
            line.0.to_string(),
            line.1.to_string(),
            line.2.to_string(),
            line.3.to_string(),
            line.4.to_string(),
            line.5.to_string(),
        ])?;
    }
    wtr.flush()?;
    Ok(())
}
