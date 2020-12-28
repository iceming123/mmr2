use common_types::header::Header;
use crossbeam_channel::Receiver;
use flyclient_ethereum::request_response::{Request, Response};
use flyclient_ethereum::NonInteractiveProofVariableDifficulty as NonInteractiveProofVD;
use flyclient_ethereum::Proof;
use flyclient_ethereum::{eth_u128_to_u128, u256_to_u128};
use flyclient_ethereum::{InMemoryMerkleTree, MerkleTree};

use std::io;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

use crate::block_header::SuccinctBlockHeader;
use crate::rpc::RpcHandler;
use crate::RequestType;
use crate::{C, L, LAMBDA};

pub struct ProofGenerator {
    rpc_handler: RpcHandler,
    request_receiver: Receiver<RequestType>,
    nonintproof: NonInteractiveProofVD,
    proof_created: u64,
    proof: Option<(Vec<Header>, Proof, u128, Vec<u64>)>,
    last_blocks: Vec<Header>,
    mmr: Option<MerkleTree<InMemoryMerkleTree>>,
    running: Arc<AtomicBool>,
}

impl ProofGenerator {
    pub fn new(
        rpc_handler: RpcHandler,
        request_receiver: Receiver<RequestType>,
        mmr: Option<MerkleTree<InMemoryMerkleTree>>,
        running: Arc<AtomicBool>,
    ) -> Self {
        ProofGenerator {
            rpc_handler,
            request_receiver,
            nonintproof: NonInteractiveProofVD::new(LAMBDA, C),
            proof_created: 0,
            proof: None,
            last_blocks: vec![],
            mmr,
            running,
        }
    }

    pub fn run(&mut self) -> io::Result<()> {
        while self.running.load(Ordering::SeqCst) {
            select! {
                recv(self.request_receiver) -> req => self.process_request(req.unwrap())?,
                default => {
                    let mut latest_blocks = self.rpc_handler.sync_block_to_mmr(&mut self.mmr)?;
                    self.update_proof(&mut latest_blocks);

            self.last_blocks = self
                .rpc_handler
                .retrieve_block_headers(
                   latest_blocks
                        .iter()
                        .skip(L as usize)
                        .map(|header| eth_u128_to_u128(header.number) as u64)
                        .collect(),
                )
                .unwrap();

                    thread::sleep(Duration::from_secs(1));
                },
                }
        }

        Ok(())
    }

    fn get_latest_block_number(&self) -> Option<u64> {
        if let Some(ref mmr) = self.mmr {
            Some(mmr.get_leaf_number())
        } else {
            None
        }
    }

    fn get_block_header_by_number(&mut self, number: u64) -> io::Result<Header> {
        let mut blocks = self.rpc_handler.retrieve_block_headers(vec![number])?;
        Ok(blocks.remove(0))
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
                    match self.proof {
                        None => Response::Error(
                            "There does not exist a non-interactive proof yet".to_owned(),
                        ),
                        Some(ref proof) => Response::NonInteractiveProof((
                            proof.0.clone(),
                            proof.1.clone(),
                            L,
                            proof.2,
                            self.last_blocks.clone(),
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
                    match self.proof {
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
                                self.last_blocks.clone(),
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

    fn update_proof(&mut self, l_headers: &Vec<SuccinctBlockHeader>) {
        if l_headers.len() == 0 {
            return;
        }

        let right_difficulty = l_headers
            .iter()
            .take(L as usize)
            .map(|header| u256_to_u128(header.difficulty))
            .sum();

        let (proof, mut required_blocks, extra_blocks) = self
            .nonintproof
            .create_new_proof(self.mmr.as_mut().unwrap(), right_difficulty);

        required_blocks.extend(
            l_headers
                .iter()
                .take(L as usize)
                .map(|header| eth_u128_to_u128(header.number) as u64),
        );

        let blocks = self
            .rpc_handler
            .retrieve_block_headers(required_blocks)
            .unwrap();
        self.proof_created = eth_u128_to_u128(l_headers[l_headers.len() - 1].number) as u64;
        self.proof = Some((blocks, proof, right_difficulty, extra_blocks));

        println!(
            "--> Created proof for block number '{}' with {} blocks and {} last blocks",
            self.proof_created,
            self.proof.as_ref().unwrap().0.len(),
            self.last_blocks.len()
        );
    }
}
