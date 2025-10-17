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
use sp1_sdk::{include_elf, ProverClient, SP1Stdin};
use std::fs;

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

    #[arg(long, default_value = "./data/partial_attestation_data.json")]
    input: String,

    #[arg(long, default_value = "./proof_output")]
    output_dir: String,
}

fn main() {
    // Setup the logger.
    sp1_sdk::utils::setup_logger();
    dotenv::dotenv().ok();

    // Parse the command line arguments.
    let args = Args::parse();

    if args.execute == args.prove {
        eprintln!("Error: You must specify either --execute or --prove");
        std::process::exit(1);
    }

    // Setup the prover client.
    let client = ProverClient::from_env();

    // Setup the inputs.
    let mut stdin = SP1Stdin::new();
    let attestation_data = fs::read_to_string(args.input).unwrap();
    stdin.write(&attestation_data);
    // println!("attestation_data: {}", attestation_data);

    if args.execute {
        // Execute the program
        let (public_values, report) = client.execute(ZKTLS_ELF, &stdin).run().unwrap();
        println!("Program executed successfully.");

        // Record the number of cycles executed.
        println!("public_values: {}", public_values.raw());
        println!("Number of cycles: {}", report.total_instruction_count());
    } else {
        // Setup the program for proving.
        let (pk, vk) = client.setup(ZKTLS_ELF);

        // Generate the proof
        let proof = client.prove(&pk, &stdin).run().expect("failed to generate proof");

        println!("Successfully generated proof!");

        // Verify the proof.
        client.verify(&proof, &vk).expect("failed to verify proof");
        println!("Successfully verified proof!");

        // println!("public_values: {}", proof.public_values.raw());
        // Save the proof
        let _ = fs::create_dir_all(&args.output_dir).unwrap();
        let proof_path = format!("{}/proof.bin", args.output_dir);
        proof.save(proof_path).expect("failed to save proof");
    }
}
