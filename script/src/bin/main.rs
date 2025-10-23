//! An end-to-end example of using the SP1 SDK to generate a proof of a program that can be executed
//! or have a core proof generated.
//!
//! You can run this script using the following command:
//! ```shell
//! RUST_LOG=info cargo run --release -- --execute
//! ```
//! or
//! ```shell
//! RUST_LOG=info cargo run --release -- --prove
//! ```

use clap::Parser;
use sp1_sdk::{include_elf, network::NetworkMode, Prover, ProverClient, SP1Stdin};
use std::env;
use std::fs;
use std::time::{Duration, Instant};
use tokio;

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

    if args.execute {
        // Setup the prover client.
        let client = ProverClient::from_env();

        // Execute the program
        let (public_values, report) = client.execute(ZKTLS_ELF, &stdin).run().unwrap();
        println!("Program executed successfully.");

        // Record the number of cycles executed.
        println!("public_values: {}", public_values.raw());
        println!("Number of cycles: {}", report.total_instruction_count());
    } else {
        let pk;
        let vk;
        let proof;
        if env::var("SP1_PROVER").as_deref() == Ok("network") {
            // Request a proof asynchronously and get the proof ID
            let network_prover = ProverClient::builder().network_for(NetworkMode::Mainnet).build();
            (pk, vk) = network_prover.setup(ZKTLS_ELF);
            let proof_id = network_prover
                .prove(&pk, &stdin)
                .groth16()
                .request_async()
                .await
                .unwrap();

            // Poll for the status of the proof
            let timeout_duration = Duration::from_secs(600); // 10 mins
            let start = Instant::now();
            proof = loop {
                if start.elapsed() > timeout_duration {
                    panic!("Get proof timeout after 10 mins");
                }
                let (status, proof_opt) = network_prover.get_proof_status(proof_id).await.unwrap();
                println!("Current status: {:?}", status);

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
        // println!("public_values: {}", proof.public_values.raw());

        // Save the proof and verifying key
        let _ = fs::create_dir_all(&args.output_dir).unwrap();

        let proof_path = format!("{}/proof.bin", args.output_dir);
        proof.save(proof_path).expect("failed to save proof");

        let proof_json = serde_json::to_string(&proof).expect("failed to serialize proof");
        let proof_json_path = format!("{}/proof.json", args.output_dir);
        std::fs::write(proof_json_path, proof_json).expect("failed to save proof");

        let vk_json = serde_json::to_string(&vk).expect("failed to serialize verifying key");
        let vk_json_path = format!("{}/vk.json", args.output_dir);
        std::fs::write(vk_json_path, vk_json).expect("failed to save verifying key");
    }
}
