use super::Proof;

use common_types::encoded::Header as EncHeader;
use common_types::header::Header;

trait SerItem: Sized {
    fn serialize(&self, byte_array: &mut SerializeArray);
    fn deserialize(byte_array: &mut DeserializeArray) -> Result<Self, &'static str>;
}

impl SerItem for Header {
    fn serialize(&self, byte_array: &mut SerializeArray) {
        let header_bytes = &self.encoded().into_inner();
        byte_array
            .array
            .extend_from_slice(&(header_bytes.len() as u64).to_be_bytes());
        byte_array.array.extend_from_slice(header_bytes);
    }

    fn deserialize(byte_array: &mut DeserializeArray) -> Result<Header, &'static str> {
        let header_length = byte_array.read_u64()?;

        if byte_array.position + header_length as usize > byte_array.array.len() {
            return Err("Response is invalid");
        }

        let header = byte_array.array
            [byte_array.position..byte_array.position + header_length as usize]
            .to_vec();
        byte_array.position += header_length as usize;
        let enc_header = EncHeader::new(header);

        match enc_header.decode() {
            Ok(h) => Ok(h),
            Err(_) => Err("Invalid response"),
        }
    }
}

impl SerItem for Proof {
    fn serialize(&self, byte_array: &mut SerializeArray) {
        let proof_bytes = &self.serialize();
        byte_array
            .array
            .extend_from_slice(&(proof_bytes.len() as u64).to_be_bytes());
        byte_array.array.extend_from_slice(proof_bytes);
    }

    fn deserialize(byte_array: &mut DeserializeArray) -> Result<Self, &'static str> {
        let proof_length = byte_array.read_u64()?;

        if byte_array.position + proof_length as usize > byte_array.array.len() {
            return Err("Response is invalid");
        }

        let proof =
            &byte_array.array[byte_array.position..byte_array.position + proof_length as usize];
        byte_array.position += proof_length as usize;

        Proof::deserialize(proof)
    }
}

impl SerItem for u64 {
    fn serialize(&self, byte_array: &mut SerializeArray) {
        byte_array.write_u64(*self);
    }
    fn deserialize(byte_array: &mut DeserializeArray) -> Result<Self, &'static str> {
        Ok(byte_array.read_u64()?)
    }
}

trait BitWrite {
    fn write_u128(&mut self, nb: u128);
    fn write_u64(&mut self, nb: u64);
    fn write_u8(&mut self, nb: u8);
    fn write_vec<T: SerItem>(&mut self, vec: &Vec<T>);
    fn write_header(&mut self, header: &Header);
    fn write_proof(&mut self, proof: &Proof);
    fn write_bytes(&mut self, bytes: &[u8]);
}

impl BitWrite for SerializeArray {
    fn write_u128(&mut self, nb: u128) {
        self.array.extend_from_slice(&nb.to_be_bytes());
    }
    fn write_u64(&mut self, nb: u64) {
        self.array.extend_from_slice(&nb.to_be_bytes());
    }
    fn write_u8(&mut self, nb: u8) {
        self.array.extend_from_slice(&nb.to_be_bytes());
    }
    fn write_vec<T: SerItem>(&mut self, vec: &Vec<T>) {
        self.array
            .extend_from_slice(&(vec.len() as u64).to_be_bytes());
        for elem in vec {
            elem.serialize(self);
        }
    }
    fn write_header(&mut self, header: &Header) {
        header.serialize(self);
    }

    fn write_proof(&mut self, proof: &Proof) {
        let ser_proof = proof.serialize();
        self.write_u64(ser_proof.len() as u64);
        self.array.extend_from_slice(&ser_proof);
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        self.array.extend_from_slice(bytes);
    }
}

trait BitRead {
    fn read_u128(&mut self) -> Result<u128, &'static str>;
    fn read_u64(&mut self) -> Result<u64, &'static str>;
    fn read_u8(&mut self) -> Result<u8, &'static str>;
    fn read_vec<T: SerItem>(&mut self) -> Result<Vec<T>, &'static str>;
    fn read_header(&mut self) -> Result<Header, &'static str>;
    fn read_proof(&mut self) -> Result<Proof, &'static str>;
    fn read_remaining_bytes(&self) -> &[u8];
}

impl<'a> BitRead for DeserializeArray<'a> {
    fn read_u128(&mut self) -> Result<u128, &'static str> {
        if self.position + 16 > self.array.len() {
            return Err("Request is too short");
        }

        let mut buf16 = [0; 16];
        buf16.copy_from_slice(&self.array[self.position..self.position + 16]);
        self.position += 16;

        Ok(u128::from_be_bytes(buf16))
    }
    fn read_u64(&mut self) -> Result<u64, &'static str> {
        if self.position + 8 > self.array.len() {
            return Err("Request is too short");
        }

        let mut buf8 = [0; 8];
        buf8.copy_from_slice(&self.array[self.position..self.position + 8]);
        self.position += 8;

        Ok(u64::from_be_bytes(buf8))
    }
    fn read_u8(&mut self) -> Result<u8, &'static str> {
        if self.position + 1 > self.array.len() {
            return Err("Request is too short");
        }

        let mut buf1 = [0; 1];
        buf1.copy_from_slice(&self.array[self.position..self.position + 1]);
        self.position += 1;

        Ok(u8::from_be_bytes(buf1))
    }

    fn read_vec<T: SerItem>(&mut self) -> Result<Vec<T>, &'static str> {
        let vec_length = self.read_u64()?;

        let mut vec = Vec::with_capacity(vec_length as usize);

        let mut curr_nb = 0;
        while curr_nb < vec_length {
            vec.push(T::deserialize(self)?);
            curr_nb += 1;
        }

        Ok(vec)
    }

    fn read_header(&mut self) -> Result<Header, &'static str> {
        SerItem::deserialize(self)
    }

    fn read_proof(&mut self) -> Result<Proof, &'static str> {
        SerItem::deserialize(self)
    }

    fn read_remaining_bytes(&self) -> &[u8] {
        &self.array[self.position..]
    }
}

struct SerializeArray {
    array: Vec<u8>,
}

impl SerializeArray {
    fn new() -> SerializeArray {
        SerializeArray { array: vec![] }
    }

    fn get_array(self) -> Vec<u8> {
        self.array
    }
}

struct DeserializeArray<'a> {
    array: &'a [u8],
    position: usize,
}

impl<'a> DeserializeArray<'a> {
    fn from(array: &'a [u8]) -> Self {
        DeserializeArray { array, position: 0 }
    }
}

#[derive(Debug)]
pub enum Request {
    BlockHeader(u64),
    LatestBlockNumber,
    // request for specific lambda, c and L, where c is percentage number of adversary hash power
    NonInteractiveProof(u64, u64, u64),
    ContinueNonInteractiveProof(u64, u64, u64, u64), // same as NonInteractiveProof, but client transmit last sync'd block
}

impl Request {
    pub fn serialize(&self) -> Vec<u8> {
        let mut byte_array = SerializeArray::new();

        match self {
            Request::BlockHeader(nb) => {
                byte_array.write_u8(0);
                byte_array.write_u64(*nb);
            }
            Request::LatestBlockNumber => {
                byte_array.write_u8(1);
            }
            Request::NonInteractiveProof(lambda, c, l) => {
                byte_array.write_u8(2);
                byte_array.write_u64(*lambda);
                byte_array.write_u64(*c);
                byte_array.write_u64(*l);
            }
            Request::ContinueNonInteractiveProof(lambda, c, l, last_block) => {
                byte_array.write_u8(3);
                byte_array.write_u64(*lambda);
                byte_array.write_u64(*c);
                byte_array.write_u64(*l);
                byte_array.write_u64(*last_block);
            }
        }
        byte_array.get_array()
    }

    pub fn deserialize(req: &[u8]) -> Result<Request, &'static str> {
        let mut byte_array = DeserializeArray::from(req);

        let req_type = byte_array.read_u8()?;

        match req_type {
            0 => {
                let nb = byte_array.read_u64()?;
                return Ok(Request::BlockHeader(nb));
            }
            1 => {
                return Ok(Request::LatestBlockNumber);
            }
            2 => {
                let lambda = byte_array.read_u64()?;
                let c = byte_array.read_u64()?;
                let l = byte_array.read_u64()?;
                return Ok(Request::NonInteractiveProof(lambda, c, l));
            }
            3 => {
                let lambda = byte_array.read_u64()?;
                let c = byte_array.read_u64()?;
                let l = byte_array.read_u64()?;
                let last_block = byte_array.read_u64()?;
                return Ok(Request::ContinueNonInteractiveProof(
                    lambda, c, l, last_block,
                ));
            }
            _ => {
                return Err("Invalid request");
            }
        }
    }
}

#[derive(Debug)]
pub enum Response {
    BlockHeader(Header),
    LatestBlockNumber(u64),
    NonInteractiveProof((Vec<Header>, Proof, u64, u128, Vec<Header>)), // blocks, proof, L, right_difficulty, additional blocks after proof blocks
    ContinueNonInteractiveProof((Vec<u64>, Vec<Header>, Proof, u64, u128, Vec<Header>)), // same as NonInteractiveProof, but for continuing sync
    Error(String),
}

impl Response {
    pub fn serialize(&self) -> Vec<u8> {
        let mut byte_array = SerializeArray::new();

        match self {
            Response::BlockHeader(header) => {
                byte_array.write_u8(0);
                byte_array.write_header(header);
            }
            Response::LatestBlockNumber(nb) => {
                byte_array.write_u8(1);
                byte_array.write_u64(*nb);
            }
            Response::NonInteractiveProof((headers, proof, l, right_difficulty, latest_blocks)) => {
                byte_array.write_u8(2);
                byte_array.write_vec(headers);
                byte_array.write_proof(proof);
                byte_array.write_u64(*l);
                byte_array.write_u128(*right_difficulty);
                byte_array.write_vec(latest_blocks);
            }
            Response::ContinueNonInteractiveProof((
                omitted_blocks,
                headers,
                proof,
                l,
                right_difficulty,
                latest_blocks,
            )) => {
                byte_array.write_u8(3);
                byte_array.write_vec(omitted_blocks);
                byte_array.write_vec(headers);
                byte_array.write_proof(proof);
                byte_array.write_u64(*l);
                byte_array.write_u128(*right_difficulty);
                byte_array.write_vec(latest_blocks);
            }
            Response::Error(err) => {
                byte_array.write_u8(4);
                byte_array.write_bytes(&err.as_bytes());
            }
        }
        byte_array.get_array()
    }

    pub fn deserialize(resp: &[u8]) -> Result<Response, &'static str> {
        let mut byte_array = DeserializeArray::from(resp);

        let req_type = byte_array.read_u8()?;

        match req_type {
            0 => {
                let header = byte_array.read_header()?;
                return Ok(Response::BlockHeader(header));
            }
            1 => {
                let nb = byte_array.read_u64()?;
                return Ok(Response::LatestBlockNumber(nb));
            }
            2 => {
                let headers = byte_array.read_vec()?;
                let proof = byte_array.read_proof()?;
                let l = byte_array.read_u64()?;
                let right_difficulty = byte_array.read_u128()?;
                let latest_headers = byte_array.read_vec()?;

                return Ok(Response::NonInteractiveProof((
                    headers,
                    proof,
                    l,
                    right_difficulty,
                    latest_headers,
                )));
            }
            3 => {
                let omitted_blocks = byte_array.read_vec()?;
                let headers = byte_array.read_vec()?;
                let proof = byte_array.read_proof()?;
                let l = byte_array.read_u64()?;
                let right_difficulty = byte_array.read_u128()?;
                let latest_headers = byte_array.read_vec()?;

                return Ok(Response::ContinueNonInteractiveProof((
                    omitted_blocks,
                    headers,
                    proof,
                    l,
                    right_difficulty,
                    latest_headers,
                )));
            }
            4 => {
                let err = String::from_utf8_lossy(byte_array.read_remaining_bytes());
                return Ok(Response::Error(err.to_string()));
            }
            _ => return Err("Invalid response"),
        }
    }
}
