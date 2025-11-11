//! An end-to-end example of using the SP1 SDK to generate a proof of a program that can be executed
//! or have a core proof generated.
//!
//! You can run this prover using the following command:
//! ```shell
//! RUST_LOG=info cargo run --release -- --execute
//! ```
//! or
//! ```shell
//! RUST_LOG=info cargo run --release -- --prove
//! ```

use clap::Parser;
use sp1_sdk::network::proto::types::FulfillmentStatus;
use sp1_sdk::network::proto::GetProofRequestStatusResponse;
use sp1_sdk::{include_elf, network::FulfillmentStrategy, Prover, ProverClient, SP1Stdin};
use std::env;
use std::fs;
use std::time::{Duration, Instant};
use tokio;
use zktls_lib::{PublicValuesStruct, SP1ZktlsProofFixture};

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const ZKTLS_ELF: &[u8] = include_elf!("zktls-program");

/// The arguments for the command.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    execute: bool,

    #[arg(long)]
    prove: bool,

    #[arg(long, default_value = "./data/hash_attestation_data.json")]
    input: String,

    #[arg(long, default_value = "./proof_output")]
    output_dir: String,
}

#[tokio::main]
async fn main() {
    // Setup the logger.
    sp1_sdk::utils::setup_logger();
    dotenv::dotenv().ok();

    // Parse the command line arguments.
    let args = Args::parse();

    if args.execute == args.prove {
        eprintln!("Error: You must specify either --execute or --prove");
        std::process::exit(1);
    }

    // Setup the inputs.
    let mut stdin = SP1Stdin::new();
    let attestation_data = fs::read_to_string(args.input).unwrap();
    stdin.write(&attestation_data);
    println!("Receive proof task!");

    if args.execute {
        // Setup the prover client.
        let client = ProverClient::from_env();

        // Execute the program
        let (public_values, report) = client.execute(ZKTLS_ELF, &stdin).run().unwrap();
        println!("Program executed successfully.");

        println!("public_values: {}", public_values.raw());

        // let bytes = hex::decode(public_values.raw().trim_start_matches("0x")).unwrap();
        let value: PublicValuesStruct = bincode::deserialize(public_values.as_slice()).unwrap();
        println!("public_values:\n{:#?}", value);
        let value_json = serde_json::to_string(&value).expect("failed to serialize public values");
        println!("public_values(json):\n{}", value_json);

        // Record the number of cycles executed.
        println!("Number of cycles: {}", report.total_instruction_count());
    } else {
        let pk;
        let vk;
        let proof;
        let mut proof_fixture = SP1ZktlsProofFixture::default();
        let _ = fs::create_dir_all(&args.output_dir).unwrap();

        if env::var("SP1_PROVER").as_deref() == Ok("network") {
            // Request a proof asynchronously and get the proof ID
            let network_prover = ProverClient::builder().network().private().build();
            (pk, vk) = network_prover.setup(ZKTLS_ELF);
            let proof_id = network_prover
                .prove(&pk, &stdin)
                .groth16()
                .timeout(Duration::from_secs(600))
                .strategy(FulfillmentStrategy::Reserved)
                .request_async()
                .await
                .unwrap();
            println!("Proof ID: {}", proof_id);

            proof_fixture.proof_id = proof_id.to_string();

            // let mut proof_uri = "".to_string();
            // let mut proof_public_uri = "".to_string();

            // Poll for the status of the proof
            let timeout_duration = Duration::from_secs(600); // 10 mins
            let start = Instant::now();
            proof = loop {
                if start.elapsed() > timeout_duration {
                    panic!("Get proof timeout after 10 mins");
                }
                let (status, proof_opt) = network_prover.get_proof_status(proof_id).await.unwrap();
                // println!("Current status: {:?}", status);
                match status {
                    GetProofRequestStatusResponse::Base(resp) => {
                        if resp.fulfillment_status == FulfillmentStatus::Fulfilled as i32 {
                            println!("Base fulfilled ✅");
                            // if let Some(uri) = &resp.proof_uri {
                            //     proof_uri = uri.to_string();
                            // }
                            // if let Some(public_uri) = &resp.proof_public_uri {
                            //     proof_public_uri = public_uri.to_string();
                            // }
                            // println!("proof_uri: {}", proof_uri);
                            // println!("proof_public_uri: {}", proof_public_uri);
                        }
                    }
                    GetProofRequestStatusResponse::Auction(resp) => {
                        if resp.fulfillment_status == FulfillmentStatus::Fulfilled as i32 {
                            println!("Auction fulfilled ✅");
                            // if let Some(uri) = &resp.proof_uri {
                            //     proof_uri = uri.to_string();
                            // }
                            // if let Some(public_uri) = &resp.proof_public_uri {
                            //     proof_public_uri = public_uri.to_string();
                            // }
                            // println!("proof_uri: {}", proof_uri);
                            // println!("proof_public_uri: {}", proof_public_uri);
                        }
                    }
                }

                if let Some(proof) = proof_opt {
                    println!("Proof ready!");
                    break proof;
                }
                tokio::time::sleep(Duration::from_secs(10)).await;
            };
            println!("Successfully generated proof!");

            // Verify the proof.
            network_prover.verify(&proof, &vk).expect("failed to verify proof");
            println!("Successfully verified proof!");
        } else {
            let client = ProverClient::from_env();

            // Setup the program for proving.
            (pk, vk) = client.setup(ZKTLS_ELF);

            // Generate the proof
            proof = client.prove(&pk, &stdin).run().expect("failed to generate proof");
            println!("Successfully generated proof!");

            // Verify the proof.
            client.verify(&proof, &vk).expect("failed to verify proof");
            println!("Successfully verified proof!");
        }
        let vk_bytes = bincode::serialize(&vk).expect("failed to serialize vk");
        let proof_bytes = bincode::serialize(&proof).expect("failed to serialize proof");
        proof_fixture.vk = format!("0x{}", hex::encode(vk_bytes));
        proof_fixture.proof = format!("0x{}", hex::encode(proof_bytes));

        let public_values: PublicValuesStruct = bincode::deserialize(proof.public_values.as_slice()).unwrap();
        proof_fixture.public_values = public_values;

        // println!("proof_fixture:\n{:#?}", proof_fixture);
        let proof_fixture_json = serde_json::to_string(&proof_fixture).expect("failed to serialize proof fixture");
        // println!("proof_fixture(json):\n{}", proof_fixture_json);

        let proof_json_path = format!("{}/proof_fixture.json", args.output_dir);
        std::fs::write(proof_json_path, proof_fixture_json).expect("failed to save proof");
        println!("Successfully saved proof!");
    }
}
