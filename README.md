# DVC-Succinct

## Requirements

- [Rust](https://rustup.rs/)
- [SP1](https://docs.succinct.xyz/docs/sp1/getting-started/install)

## Running the Project

### Build the Program

```sh
cd program
cargo prove build
```

The program is automatically built through `script/build.rs` when the script is built.

### Execute the Program

To run the program without generating a proof:

```sh
cd prover
cargo run --release -- --execute
```

This will execute the program and display the output.

### Generate an SP1 Core Proof

To generate an SP1 [core proof](https://docs.succinct.xyz/docs/sp1/generating-proofs/proof-types#core-default) for your program:

```sh
cd prover
cargo run --release -- --prove
```

## Using the Prover Network

We highly recommend using the [Succinct Prover Network](https://docs.succinct.xyz/docs/network/introduction) for any non-trivial programs or benchmarking purposes. For more information, see the [key setup guide](https://docs.succinct.xyz/docs/network/developers/key-setup) to get started.

To get started, copy the example environment file:

```sh
cp .env.example .env
```

Then, set the `SP1_PROVER` environment variable to `network` and set the `NETWORK_PRIVATE_KEY` environment variable to your whitelisted private key.


## Localtest

```sh
cd prover
RUST_LOG=info cargo build --release

cd ..
RUST_LOG=info ./target/release/zktls --execute \
  --input ./prover/data/binance_pha_attestation_data.json \
  --output-dir proof_output
```
