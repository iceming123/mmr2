use std::io::{self, Error, ErrorKind};
use std::path::Path;

use common_types::header::Header;
use ethash;
use ethash::{EthashManager, OptimizeFor};
use ethereum_types::{H256, H64};
use rlp::Rlp;

/// Ethash specific seal
#[derive(Debug, PartialEq)]
pub struct Seal {
    /// Ethash seal mix_hash
    pub mix_hash: H256,
    /// Ethash seal nonce
    pub nonce: H64,
}

impl Seal {
    /// Tries to parse rlp as ethash seal.
    pub fn parse_seal<T: AsRef<[u8]>>(seal: &[T]) -> io::Result<Self> {
        if seal.len() != 2 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("mismatch: expected: '{}', found '{}'", 2, seal.len()),
            ));
        }

        let mix_hash = Rlp::new(seal[0].as_ref()).as_val::<H256>().unwrap();
        let nonce = Rlp::new(seal[1].as_ref()).as_val::<H64>().unwrap();
        let seal = Seal { mix_hash, nonce };

        Ok(seal)
    }
}

pub fn pow_verify(headers: &Vec<&Header>) -> io::Result<()> {
    let manager = EthashManager::new(
        Path::new("light_cache_XXX"),
        OptimizeFor::Memory,
        u64::max_value(),
    );

    for header in headers {
        if header.number() == 0 {
            continue;
        }

        let seal = Seal::parse_seal(header.seal()).unwrap();

        println!("compute light for block number '{:?}'", header.number());
        let result = manager.compute_light(
            header.number() as u64,
            &header.bare_hash().0,
            seal.nonce.low_u64(),
        );
        let mix = H256(result.mix_hash);

        let difficulty = ethash::boundary_to_difficulty(&H256::from(&result.value[..]));

        if mix != seal.mix_hash {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "mismatched H256 seal element: expected: '{:?}', found '{}'",
                    mix, seal.mix_hash
                ),
            ));
        }

        if difficulty < *header.difficulty() {
            return Err(Error::new(ErrorKind::InvalidData, "invalid proof-of-work"));
        }
    }

    Ok(())
}
