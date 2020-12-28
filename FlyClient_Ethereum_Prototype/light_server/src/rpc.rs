use ethbloom;
use parity_bytes::Bytes;
use serde_json;
use web3;

use web3::types::{Block, BlockId, BlockNumber};
use web3::types::{H160, H256, U256};

use common_types::header::Header;
use flyclient_ethereum::{eth_u128_to_u128, u256_to_u128};
use flyclient_ethereum::{InMemoryMerkleTree, MerkleTree};
use std::cmp;
use std::io::{self, Error, ErrorKind};

use super::block_header::SuccinctBlockHeader;

use crate::csv::CsvHandler;
use crate::L;

const MAX_PARALLEL_REQUESTS: usize = 64;
const FILE_NAME: &str = "mmr.bin";

pub struct RpcHandler {
    fetch: AsyncFetch,
}

impl RpcHandler {
    pub fn new(addr: &str) -> Self {
        let fetch = AsyncFetch::new(addr);

        RpcHandler { fetch }
    }

    pub fn retrieve_block_headers(&mut self, required_blocks: Vec<u64>) -> io::Result<Vec<Header>> {
        self.fetch.get_block_header(required_blocks)
    }

    pub fn sync_new_blocks(
        &mut self,
        csv_handler: &CsvHandler,
        block_header: &mut Vec<SuccinctBlockHeader>,
    ) -> io::Result<()> {
        let latest_block_number = self.fetch.get_latest_block_number();
        let mut next_block_number = block_header.len() as u64;

        while next_block_number < latest_block_number {
            let next_header = cmp::min(latest_block_number + 1, next_block_number + 2048);
            let headers = self
                .fetch
                .get_succinct_block_header(next_block_number, next_header);

            block_header.extend_from_slice(&headers);
            csv_handler.write_block_headers(&headers)?;
            next_block_number = block_header.len() as u64;
            println!(
                "# Sync Block {}/{}",
                (block_header.len() - 1),
                latest_block_number
            );
        }

        Ok(())
    }

    pub fn sync_block_to_mmr(
        &mut self,
        mmr: &mut Option<MerkleTree<InMemoryMerkleTree>>,
    ) -> io::Result<Vec<SuccinctBlockHeader>> {
        let latest_block_number = self.fetch.get_latest_block_number();
        let mut next_block_number = match mmr {
            None => 0,
            Some(mmr) => mmr.get_leaf_number(),
        };

        let mut latest_blocks = vec![];

        let begin_l_blocks = latest_block_number - L;

        let value = u64::from_be_bytes([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x80]);
        let next_mmr_root_number = ((begin_l_blocks - 1) & value) + 1;

        // wenn nächster mmr der gleiche wie der vorige wäre
        if latest_block_number < (L + 1) || next_mmr_root_number == next_block_number {
            return Ok(vec![]);
        }

        while next_block_number < latest_block_number {
            let next_header = cmp::min(latest_block_number + 1, next_block_number + 2048);
            let headers = self
                .fetch
                .get_succinct_block_header(next_block_number, next_header);

            let header_len = headers.len() as u64;
            for header in headers {
                let header_number = eth_u128_to_u128(header.number) as u64;
                if header_number < next_mmr_root_number {
                    if let Some(ref mut mmr) = mmr {
                        if mmr.get_leaf_number() != header_number {
                            panic!("Unexpected header number");
                        }

                        mmr.append_leaf(header.hash, u256_to_u128(header.difficulty));
                    } else {
                        mmr.replace(MerkleTree::<InMemoryMerkleTree>::new(
                            header.hash,
                            u256_to_u128(header.difficulty),
                            Some(FILE_NAME),
                        ));
                    }
                } else {
                    latest_blocks.push(header);
                }
            }
            next_block_number += header_len;
            println!(
                "# Sync Block {}/{}",
                (next_block_number - 1),
                latest_block_number
            );
        }

        Ok(latest_blocks)
    }
}

struct AsyncFetch {
    web3: web3::Web3<web3::transports::batch::Batch<web3::transports::Http>>,
    event_loop: tokio_core::reactor::Core,
}

impl AsyncFetch {
    fn new(addr: &str) -> Self {
        let event_loop = tokio_core::reactor::Core::new().unwrap();
        let http = web3::transports::Http::with_event_loop(
            addr,
            &event_loop.handle(),
            MAX_PARALLEL_REQUESTS,
        )
        .unwrap();

        let web3 = web3::Web3::new(web3::transports::Batch::new(http));

        AsyncFetch { web3, event_loop }
    }

    pub fn get_latest_block_number(&mut self) -> u64 {
        let _ = self.web3.eth().block_number();

        let result = self.web3.transport().submit_batch();
        let number = self.event_loop.run(result).unwrap();

        let value = match number[0] {
            Ok(ref val) => val.clone(),
            Err(_) => panic!("could not fetch latest block number"),
        };

        u64::from(serde_json::from_value::<U256>(value).expect("Could not parse json"))
    }

    fn get_block_header(&mut self, required_blocks: Vec<u64>) -> io::Result<Vec<Header>> {
        let mut block_headers = Vec::with_capacity(required_blocks.len());

        for i in required_blocks {
            let _ = self
                .web3
                .eth()
                .block(BlockId::Number(BlockNumber::Number(i as u64)));

            println!(
                "Retrieve block: {}, {:?}",
                i,
                BlockId::Number(BlockNumber::Number(i as u64))
            );
            let result = self.web3.transport().submit_batch();
            let block = self.event_loop.run(result).unwrap();

            let value = match block[0] {
                Ok(ref block) => block.clone(),
                Err(_) => panic!("blabla"),
            };

            let block: Block<H256> = serde_json::from_value(value).unwrap();
            block_headers.push(convert_header(block));
        }

        Ok(block_headers)
    }

    // TODO: schauen warum das nicht funktioniert mit den mehreren blöcken auf einmal
    //    fn get_block_header(&mut self, required_blocks: Vec<u64>) -> io::Result<Vec<Header>> {
    //        let mut block_headers = Vec::with_capacity(required_blocks.len());
    //
    //        println!("Retrieve block headers: {:?}", required_blocks);
    //
    //        for i in required_blocks {
    //            let _ = self
    //                .web3
    //                .eth()
    //                .block(BlockId::Number(BlockNumber::Number(i as u64)));
    //        }
    //
    //        let result = self.web3.transport().submit_batch();
    //        let blocks = self.event_loop.run(result).unwrap();
    //
    //        for block in blocks {
    //            match block {
    //                Err(err) => {
    //                    return Err(Error::new(
    //                        ErrorKind::InvalidData,
    //                        format!("Error while receiving block headers from peer: {}", err),
    //                    ))
    //                }
    //                Ok(value) => {
    //                    let block: Block<H256> =
    //                        serde_json::from_value(value).expect("Could not parse json");
    //
    //                    block_headers.push(convert_header(block));
    //                }
    //            }
    //        }
    //
    //        Ok(block_headers)
    //    }

    // retrieve block header, inclusive start, exclusive end
    fn get_succinct_block_header(&mut self, start: u64, end: u64) -> Vec<SuccinctBlockHeader> {
        let mut hashes = Vec::with_capacity((end + 1 - start) as usize);

        for i in start..end {
            let _ = self
                .web3
                .eth()
                .block(BlockId::Number(BlockNumber::Number(i)));
        }

        let result = self.web3.transport().submit_batch();
        let blocks = self.event_loop.run(result).unwrap();

        for elem in blocks {
            match elem {
                Err(error) => {
                    println!("Error: {:?}", error);
                    panic!(error);
                }
                Ok(value) => {
                    let block: Block<H256> =
                        serde_json::from_value(value).expect("Could not parse json");
                    hashes.push(SuccinctBlockHeader::new(
                        block.number.unwrap(),
                        block.hash.expect("Block did not contain hash value"),
                        block.difficulty,
                    ));
                }
            };
        }

        hashes
    }
}

fn convert_header(block: Block<H256>) -> Header {
    let mut header = Header::new();
    header.set_parent_hash(web3_h256_to_h256(block.parent_hash));
    header.set_timestamp(block.timestamp.low_u64());
    header.set_number(block.number.unwrap().low_u64());
    header.set_author(web3_h160_to_h160(block.author));
    header.set_transactions_root(web3_h256_to_h256(block.transactions_root));
    header.set_uncles_hash(web3_h256_to_h256(block.uncles_hash));
    header.set_extra_data(block.extra_data.0);
    header.set_state_root(web3_h256_to_h256(block.state_root));
    header.set_receipts_root(web3_h256_to_h256(block.receipts_root));
    header.set_log_bloom(web3_bloom_to_bloom(block.logs_bloom));
    header.set_gas_used(web3_u265_to_u265(block.gas_used));
    header.set_gas_limit(web3_u265_to_u265(block.gas_limit));
    header.set_difficulty(web3_u265_to_u265(block.difficulty));

    let mut bytes: Vec<Bytes> = vec![];
    for b in block.seal_fields {
        let c: Bytes = b.0;
        bytes.push(c);
    }
    header.set_seal(bytes);

    header
}

pub fn web3_bloom_to_bloom(bloom: ethbloom::Bloom) -> ethereum_types::Bloom {
    ethereum_types::Bloom::from(bloom.to_fixed_bytes())
}

pub fn web3_h256_to_h256(x: H256) -> ethereum_types::H256 {
    ethereum_types::H256::from(x.to_fixed_bytes())
}

pub fn web3_h160_to_h160(x: H160) -> ethereum_types::H160 {
    ethereum_types::H160::from(x.to_fixed_bytes())
}

pub fn web3_u265_to_u265(x: U256) -> ethereum_types::U256 {
    ethereum_types::U256(x.0)
}
