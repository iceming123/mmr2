use flyclient_ethereum::request_response::{Request, Response};
use flyclient_ethereum::{InMemoryMerkleTree, MerkleTree};
use std::io::{self, prelude::*, ErrorKind};
use std::net::TcpListener;
use std::net::TcpStream;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use web3::types::U128;

use structopt::StructOpt;

#[macro_use]
extern crate crossbeam_channel;

use crossbeam_channel::Sender;

mod block_header;
mod csv;
mod proof_generator;
mod rpc;

use crate::csv::CsvHandler;

pub const LAMBDA: u64 = 50;
pub const C: u64 = 50;
pub const L: u64 = 100;

pub const FILE_NAME: &str = "mmr.bin";

#[derive(StructOpt, Debug)]
struct Opt {
    /// Number of blocks to use for testing purposes
    #[structopt(short = "n", long = "nb-blocks")]
    nb_blocks: Option<u64>,

    /// Url of the Ethereum node to connect
    #[structopt(
        short = "e",
        long = "ethereum-node",
        default_value = "http://localhost:8545"
    )]
    eth_node: String,

    /// Server address for this application to bind to
    #[structopt(short = "s", long = "server-address", default_value = "0.0.0.0:7878")]
    server_address: String,

    /// Filename for caching partial Block header
    #[structopt(short = "f", long = "file-name", default_value = "block_headers.csv")]
    file_name: String,

    /// Specify mode, in which application should run
    #[structopt(long = "mode", default_value = "old_mode")]
    mode: String,
}

fn main() -> io::Result<()> {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    let opt = Opt::from_args();

    // channel to send requests from client to block header thread
    let (request_sender, request_receiver) = crossbeam_channel::bounded(1024);

    if opt.mode == "old_mode" {
        println!("Old mode");

        let mut csv_handler = CsvHandler::new(&opt.file_name);

        let (block_numbers, test_mode) = match opt.nb_blocks {
            None => (None, false),
            Some(nb) => (Some(U128::from(nb)), true),
        };

        let headers_in_files = csv_handler.read_block_headers(block_numbers)?;

        println!(
            "Load {} existing block headers from CSV file",
            headers_in_files.len()
        );

        let server_address = String::from(opt.server_address);
        thread::spawn(move || {
            let listener = TcpListener::bind(&server_address).unwrap();
            for stream in listener.incoming() {
                let stream = stream.unwrap();
                println!("\t\tClient connection established!");
                handle_connection(stream, request_sender.clone());
            }
        });

        let rpc_handler = rpc::RpcHandler::new(&opt.eth_node);

        let mmr = if test_mode {
            println!("Testmode");
            None
        } else {
            MerkleTree::<InMemoryMerkleTree>::load(FILE_NAME).ok()
        };

        block_header::BlockHeaderThread::new(
            headers_in_files,
            csv_handler,
            rpc_handler,
            request_receiver,
            running,
            mmr,
        )
        .run(test_mode)
        .expect("block header thread died");
    } else if opt.mode == "new_mode" {
        println!("New mode");

        let mmr = match MerkleTree::<InMemoryMerkleTree>::load(FILE_NAME) {
            Ok(mmr) => {
                println!("Load MMR with {} block headers", mmr.get_leaf_number());
                Some(mmr)
            }
            Err(err) => {
                if err.kind() != ErrorKind::NotFound {
                    panic!(err);
                }
                None
            }
        };

        let server_address = String::from(opt.server_address);
        thread::spawn(move || {
            let listener = TcpListener::bind(&server_address).unwrap();
            for stream in listener.incoming() {
                let stream = stream.unwrap();
                println!("\t\tClient connection established!");
                handle_connection(stream, request_sender.clone());
            }
        });

        let rpc_handler = rpc::RpcHandler::new(&opt.eth_node);
        proof_generator::ProofGenerator::new(rpc_handler, request_receiver, mmr, running)
            .run()
            .expect("proof generator thread died");
    } else if opt.mode == "measurements" {
        println!("Measurement mode");

        // channel to send requests from client to block header thread
        let (mm_request_sender, mm_request_receiver) = crossbeam_channel::bounded(1024);

        let mut csv_handler = CsvHandler::new(&opt.file_name);

        //TODO: noch überlegen, was ma machen soll, wenn test_mode da aktiv ist
        //vllt eh nix, weil ma sowieso keinen rpc sync block drin hat
        let (block_numbers, _) = match opt.nb_blocks {
            None => (None, false),
            Some(nb) => (Some(U128::from(nb)), true),
        };

        let headers_in_files = csv_handler.read_block_headers(block_numbers)?;

        println!(
            "Load {} existing block headers from CSV file",
            headers_in_files.len()
        );

        let server_address = String::from(opt.server_address);
        thread::spawn(move || {
            let listener = TcpListener::bind(&server_address).unwrap();
            for stream in listener.incoming() {
                let stream = stream.unwrap();
                println!("\t\tClient connection established!");
                // handle connection muss auch noch anders gebaut werden, weil der client mir sagen
                // können muss, was er einen proof haben möchte (also bis zu welcher nummer)
                handle_measurement_connection(stream, mm_request_sender.clone());
            }
        });

        let rpc_handler = rpc::RpcHandler::new(&opt.eth_node);
        // rpc sync brauch ich nicht, weil ich ja keine block header mehr brauch
        // aber rpc fetch blocks brauch ich weil ich ja die blocks generieren mag

        block_header::BlockHeaderThread::new(
            headers_in_files,
            csv_handler,
            rpc_handler,
            request_receiver,
            running,
            None,
        )
        .run_measurements(mm_request_receiver)
        .expect("block header thread died");
    } else {
        println!("Unsupported mode!");
    }

    Ok(())
}

fn handle_measurement_connection(
    mut stream: TcpStream,
    mm_request_sender: Sender<MeasurementRequestType>,
) {
    let (response_sender, response_receiver) = crossbeam_channel::bounded(1);

    let mut buffer = vec![];
    let _ = stream.read_to_end(&mut buffer).unwrap();
    println!("\t\tRequest: {:?}", buffer);

    let mut nb_buffer = [0u8; 8];

    if buffer.len() != 16 {
        println!("No valid block number and L request");
        return;
    }

    nb_buffer[..].copy_from_slice(&buffer[..8]);

    let block_number = u64::from_be_bytes(nb_buffer);

    nb_buffer[..].copy_from_slice(&buffer[8..]);

    let l = u64::from_be_bytes(nb_buffer);

    println!(
        "\t\tRequest (decoded): Blocknumber: {}, L: {}",
        block_number, l
    );

    println!("\t\tSend request to other thread");

    mm_request_sender
        .send(MeasurementRequestType {
            req: (block_number, l),
            response_channel: response_sender,
        })
        .unwrap();

    let response = response_receiver.recv().unwrap();
    println!("\t\tGot response from other thread");
    stream.write(&response.serialize()).unwrap();
    stream.flush().unwrap();
    println!("\t\tSending response finished");
}

fn handle_connection(mut stream: TcpStream, request_sender: Sender<RequestType>) {
    let (response_sender, response_receiver) = crossbeam_channel::bounded(1);

    let mut buffer = vec![];
    let _ = stream.read_to_end(&mut buffer).unwrap();

    println!("\t\tRequest: {:?}", buffer);

    let req: Request = match Request::deserialize(&buffer) {
        Ok(req) => {
            println!("\t\tRequest (decoded): {:?}", req);
            req
        }
        Err(err) => {
            println!("Could not deserialize request: {}", err);
            return;
        }
    };

    println!("\t\tSend request to other thread");

    request_sender
        .send(RequestType {
            req: req,
            response_channel: response_sender,
        })
        .unwrap();

    let response = response_receiver.recv().unwrap();
    println!("\t\tGot response from other thread");
    stream.write(&response.serialize()).unwrap();
    stream.flush().unwrap();
    println!("\t\tSending response finished");
}

pub struct MeasurementRequestType {
    pub req: (u64, u64), // block number and L
    pub response_channel: Sender<Response>,
}

pub struct RequestType {
    pub req: Request,
    pub response_channel: Sender<Response>,
}
