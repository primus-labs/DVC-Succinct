#!/bin/bash
curdir=$(pwd)

# zktls
echo "build program..."
cd ${curdir}/program
RUST_LOG=info cargo prove build

echo "build prover..."
cd ${curdir}/prover
RUST_LOG=info cargo build --release
echo "cp zktls"
cp ${curdir}/target/release/zktls ${curdir}/server/bin/
echo "finish"
