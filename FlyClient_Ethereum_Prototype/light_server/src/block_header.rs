use flyclient_ethereum::{InMemoryMerkleTree, MerkleTree};
use serde::{Deserialize, Serialize};
use web3::types::{H256, U128, U256};

use common_types::header::Header;
use crossbeam_channel::Receiver;
use flyclient_ethereum::request_response::{Request, Response};
use flyclient_ethereum::NonInteractiveProofVariableDifficulty as NonInteractiveProofVD;
use flyclient_ethereum::Proof;
use flyclient_ethereum::{eth_u128_to_u128, u256_to_u128};
use std::cmp;
use std::io::{self, Error, ErrorKind};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

use crate::csv::CsvHandler;
use crate::rpc::RpcHandler;
use crate::MeasurementRequestType;
use crate::RequestType;

use crate::{C, L, LAMBDA};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SuccinctBlockHeader {
    pub number: U128,
    pub hash: H256,
    pub difficulty: U256,
}

pub struct CompleteNonInteractiveProof {
    nonintproof: NonInteractiveProofVD,
    proof_created: u64,
    proof: Option<(Vec<Header>, Proof, u128, Vec<u64>, u64)>, // last element: L
    last_blocks: Vec<Header>,
    mmr: Option<MerkleTree<InMemoryMerkleTree>>,
}

impl CompleteNonInteractiveProof {
    fn new(mmr: Option<MerkleTree<InMemoryMerkleTree>>) -> Self {
        let nonintproof = NonInteractiveProofVD::new(LAMBDA, C);

        CompleteNonInteractiveProof {
            nonintproof,
            proof_created: 0,
            proof: None,
            last_blocks: vec![],

            mmr,
        }
    }

    fn update_proof(
        &mut self,
        mut rpc_handler: &mut RpcHandler,
        headers: &[SuccinctBlockHeader],
        l: u64,
    ) {
        let latest_block_number = headers.len() as u64;

        let next_block_number = match &self.mmr {
            None => 0,
            Some(mmr) => mmr.get_leaf_number(),
        };

        let begin_l_blocks = latest_block_number - l;

        // calculates the last number which is a multiple of 128 of the current number
        // by setting the last 7 bit to zero
        let value = u64::from_be_bytes([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x80]);
        let next_mmr_root_number = ((begin_l_blocks - 1) & value) + 1;

        if latest_block_number < (l + 1) || next_mmr_root_number == next_block_number {
            self.last_blocks = rpc_handler
                .retrieve_block_headers(
                    headers
                        .iter()
                        .skip((next_block_number + l) as usize)
                        .map(|header| eth_u128_to_u128(header.number) as u64)
                        .collect(),
                )
                .unwrap();
            return;
        }

        self.add_header_to_mmr(&headers[..next_mmr_root_number as usize]);
        self.create_proof(
            &mut rpc_handler,
            &headers[next_mmr_root_number as usize..],
            l,
        );
    }

    fn add_header_to_mmr(&mut self, block_header: &[SuccinctBlockHeader]) {
        let curr_leaf_number = match self.mmr {
            None => {
                self.mmr = Some(MerkleTree::new(
                    block_header[0].hash,
                    u256_to_u128(block_header[0].difficulty),
                    None,
                ));
                1
            }
            Some(ref mut mmr) => mmr.get_leaf_number(),
        };

        for header in block_header.iter().skip(curr_leaf_number as usize) {
            self.mmr
                .as_mut()
                .unwrap()
                .append_leaf(header.hash, u256_to_u128(header.difficulty));
        }
    }

    fn create_proof(
        &mut self,
        rpc_handler: &mut RpcHandler,
        l_headers: &[SuccinctBlockHeader],
        l: u64,
    ) {
        let right_difficulty = l_headers
            .iter()
            .take(l as usize)
            .map(|header| u256_to_u128(header.difficulty))
            .sum();

        let (proof, mut required_blocks, extra_blocks) = self
            .nonintproof
            .create_new_proof(self.mmr.as_mut().unwrap(), right_difficulty);

        required_blocks.extend(
            l_headers
                .iter()
                .take(l as usize)
                .map(|header| eth_u128_to_u128(header.number) as u64),
        );

        let blocks = rpc_handler.retrieve_block_headers(required_blocks).unwrap(); //TODO:unwrap entf
        self.proof_created = self.mmr.as_mut().unwrap().get_leaf_number();
        self.proof = Some((blocks, proof, right_difficulty, extra_blocks, l));

        self.last_blocks = rpc_handler
            .retrieve_block_headers(
                l_headers
                    .iter()
                    .skip(l as usize)
                    .map(|header| eth_u128_to_u128(header.number) as u64)
                    .collect(),
            )
            .unwrap();

        println!(
            "-->  Created proof for block number '{}' with {} blocks and {} last blocks",
            self.proof_created,
            self.proof.as_ref().unwrap().0.len(),
            self.last_blocks.len()
        );
    }
}

impl SuccinctBlockHeader {
    pub fn new(number: U128, hash: H256, difficulty: U256) -> SuccinctBlockHeader {
        SuccinctBlockHeader {
            number,
            hash,
            difficulty,
        }
    }

    pub fn get_block_number(&self) -> U128 {
        self.number
    }
}

pub struct BlockHeaderThread {
    headers: Vec<SuccinctBlockHeader>,
    csv_handler: CsvHandler,
    rpc_handler: RpcHandler,
    request_receiver: Receiver<RequestType>,
    nonintproof: CompleteNonInteractiveProof,
    running: Arc<AtomicBool>,
}

impl BlockHeaderThread {
    pub fn new(
        headers: Vec<SuccinctBlockHeader>,
        csv_handler: CsvHandler,
        rpc_handler: RpcHandler,
        request_receiver: Receiver<RequestType>,
        running: Arc<AtomicBool>,
        mmr: Option<MerkleTree<InMemoryMerkleTree>>,
    ) -> Self {
        BlockHeaderThread {
            headers,
            csv_handler,
            rpc_handler,
            request_receiver,
            nonintproof: CompleteNonInteractiveProof::new(mmr),
            running,
        }
    }

    fn get_latest_block_number(&self) -> Option<u64> {
        match self.headers.len() {
            0 => None,
            v => Some(v as u64),
        }
    }

    fn get_block_header_by_number(&mut self, number: u64) -> io::Result<Header> {
        let mut blocks = self.rpc_handler.retrieve_block_headers(vec![number])?;
        Ok(blocks.remove(0))
    }

    pub fn run(&mut self, test_mode: bool) -> io::Result<()> {
        if test_mode {
            self.run_test()?;
        } else {
            while self.running.load(Ordering::SeqCst) {
                select! {
                recv(self.request_receiver) -> req => self.process_request(req.unwrap())?,
                default => {self.rpc_handler.sync_new_blocks(&self.csv_handler,&mut self.headers)?;
                self.nonintproof
                    .update_proof(&mut self.rpc_handler, &self.headers, L);
                thread::sleep(Duration::from_secs(1));
                },
                }
            }
            println!("Exiting block_header_thread...");
        }
        Ok(())
    }

    pub fn run_test(&mut self) -> io::Result<()> {
        self.nonintproof
            .update_proof(&mut self.rpc_handler, &self.headers, L);

        use crossbeam_channel::RecvTimeoutError;
        while self.running.load(Ordering::SeqCst) {
            match self.request_receiver.recv_timeout(Duration::from_secs(1)) {
                Ok(req) => self.process_request(req)?,
                Err(err) => match err {
                    RecvTimeoutError::Timeout => continue,
                    RecvTimeoutError::Disconnected => {
                        return Err(Error::new(ErrorKind::BrokenPipe, err))
                    }
                },
            }
        }
        println!("Exiting block_header_thread...");
        Ok(())
    }

    pub fn run_measurements(
        &mut self,
        mm_request_receiver: Receiver<MeasurementRequestType>,
    ) -> io::Result<()> {
        // TODO: soll angenommen werden, dass sich die block nummer nur erhöht? wenn ja dann geht
        // das schneller am server

        use crossbeam_channel::RecvTimeoutError;

        while self.running.load(Ordering::SeqCst) {
            match mm_request_receiver.recv_timeout(Duration::from_secs(1)) {
                Ok(req) => {
                    // noch das so machen, dass er nur eine bestimmte anzahl nimmt von den headers
                    println!(
                        "Update proof with {} headers",
                        self.headers[..req.req.0 as usize].len()
                    );

                    let last_l = match self.nonintproof.proof {
                        None => None,
                        Some((_, _, _, _, l)) => Some(l),
                    };
                    if let Some(last_l) = last_l {
                        if last_l != req.req.1 {
                            self.nonintproof.proof_created = 0;
                            self.nonintproof.proof = None;
                            self.nonintproof.mmr = None;
                            self.nonintproof.last_blocks = vec![];
                        }
                    }

                    self.nonintproof.update_proof(
                        &mut self.rpc_handler,
                        &self.headers[..req.req.0 as usize],
                        req.req.1,
                    );

                    self.process_request_measurement(req)?
                }
                Err(err) => match err {
                    RecvTimeoutError::Timeout => continue,
                    RecvTimeoutError::Disconnected => {
                        return Err(Error::new(ErrorKind::BrokenPipe, err))
                    }
                },
            }
        }
        println!("Exiting block_header_thread...");
        Ok(())
    }

    fn process_request_measurement(&mut self, req: MeasurementRequestType) -> io::Result<()> {
        let response = match self.nonintproof.proof {
            None => Response::Error("There does not exist a non-interactive proof yet".to_owned()),
            Some(ref proof) => Response::NonInteractiveProof((
                proof.0.clone(),
                proof.1.clone(),
                req.req.1,
                proof.2,
                self.nonintproof.last_blocks.clone(),
            )),
        };

        req.response_channel.send(response).unwrap();
        Ok(())
    }

    fn process_request(&mut self, req: RequestType) -> io::Result<()> {
        let response = match req.req {
            Request::LatestBlockNumber => match self.get_latest_block_number() {
                None => Response::Error("There does not exist a block yet".to_owned()),
                Some(v) => Response::LatestBlockNumber(v),
            },
            Request::NonInteractiveProof(lambda, c, l) => {
                // at the moment there is only one allowed configuration for non-interactive proofs
                if lambda != LAMBDA || c != C || l != L {
                    Response::Error(
                        "There does not exist a non-interactive proof with this configuration"
                            .to_owned(),
                    )
                } else {
                    match self.nonintproof.proof {
                        None => Response::Error(
                            "There does not exist a non-interactive proof yet".to_owned(),
                        ),
                        Some(ref proof) => Response::NonInteractiveProof((
                            proof.0.clone(),
                            proof.1.clone(),
                            L,
                            proof.2,
                            self.nonintproof.last_blocks.clone(),
                        )),
                    }
                }
            }
            Request::ContinueNonInteractiveProof(lambda, c, l, last_block) => {
                // at the moment there is only one allowed configuration for non-interactive proofs
                if lambda != LAMBDA || c != C || l != L {
                    Response::Error(
                        "There does not exist a non-interactive proof with this configuration"
                            .to_owned(),
                    )
                } else {
                    match self.nonintproof.proof {
                        None => Response::Error(
                            "There does not exist a non-interactive proof yet".to_owned(),
                        ),
                        Some(ref proof) => {
                            let mut omitted_blocks = vec![];
                            let mut full_header = vec![];

                            if proof.3.contains(&last_block) {
                                for header in &proof.0 {
                                    if header.number() <= last_block {
                                        omitted_blocks.push(header.number());
                                    } else {
                                        full_header.push(header.clone());
                                    }
                                }
                            } else {
                                full_header = proof.0.clone();
                            }

                            Response::ContinueNonInteractiveProof((
                                omitted_blocks,
                                full_header,
                                proof.1.clone(),
                                L,
                                proof.2,
                                self.nonintproof.last_blocks.clone(),
                            ))
                        }
                    }
                }
            }
            Request::BlockHeader(number) => match self.get_block_header_by_number(number) {
                Err(err) => Response::Error(err.get_ref().unwrap().to_string()),
                Ok(block) => Response::BlockHeader(block),
            },
        };

        req.response_channel.send(response).unwrap();
        Ok(())
    }
}
