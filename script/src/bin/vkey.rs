use sp1_sdk::{include_elf, HashableKey, Prover, ProverClient};

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const ZKTLS_ELF: &[u8] = include_elf!("zktls-program");

fn main() {
    let prover = ProverClient::builder().cpu().build();
    let (_, vk) = prover.setup(ZKTLS_ELF);
    println!("vk_hash {}", vk.bytes32());

    let vk_bytes = bincode::serialize(&vk).expect("failed to serialize vk");
    println!("vk_bytes {}", format!("0x{}", hex::encode(vk_bytes)));
}
