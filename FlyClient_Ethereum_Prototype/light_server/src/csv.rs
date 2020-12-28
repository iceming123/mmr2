use csv;
use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Error, ErrorKind};
use web3::types::U128;

use super::block_header::SuccinctBlockHeader;

pub struct CsvHandler {
    file_path: String,
}

impl CsvHandler {
    pub fn new(file_path: &str) -> Self {
        CsvHandler {
            file_path: String::from(file_path),
        }
    }

    pub fn read_block_headers(
        &mut self,
        number: Option<U128>,
    ) -> io::Result<Vec<SuccinctBlockHeader>> {
        let mut headers = vec![];
        let mut next_block_number = U128::from(0);
        let one = U128::from(1);

        let f = match File::open(&self.file_path) {
            Ok(f) => f,
            Err(ref err) if err.kind() == ErrorKind::NotFound => return Ok(headers),
            Err(err) => return Err(err),
        };

        let f = BufReader::new(f);
        let mut rdr = csv::Reader::from_reader(f);

        for result in rdr.deserialize() {
            let record: SuccinctBlockHeader = result?;

            if record.get_block_number() != next_block_number {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!(
                        "expected header number {} does not match {}",
                        record.get_block_number(),
                        next_block_number
                    ),
                ));
            }
            headers.push(record);

            next_block_number += one;

            if let Some(nb) = number {
                if nb <= next_block_number {
                    return Ok(headers);
                }
            }
        }

        Ok(headers)
    }

    pub fn write_block_headers(&self, headers: &[SuccinctBlockHeader]) -> io::Result<()> {
        let mut already_exists = true;

        let f = match OpenOptions::new().append(true).open(&self.file_path) {
            Ok(file) => file,
            Err(ref e) if e.kind() == ErrorKind::NotFound => {
                already_exists = false;
                File::create(&self.file_path)?
            }
            Err(err) => return Err(err),
        };

        let f = BufWriter::new(f);

        let mut wtr = if already_exists {
            csv::WriterBuilder::new().has_headers(false).from_writer(f)
        } else {
            csv::Writer::from_writer(f)
        };

        for header in headers {
            wtr.serialize(header)?;
        }

        wtr.flush()?;

        Ok(())
    }
}
