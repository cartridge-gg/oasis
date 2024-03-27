use ark_ff::Field;
use ark_ff::PrimeField;
use ark_serialize::CanonicalDeserialize;
use ark_serialize::CanonicalSerialize;
use ministark::stark::Stark;
use ministark::Proof;
use ministark::ProofOptions;
use ministark_gpu::fields::p3618502788666131213697322783095070105623107215331596699973092056135872020481;
use sandstorm::claims;
use sandstorm_binary::AirPrivateInput;
use sandstorm_binary::AirPublicInput;
use sandstorm_binary::CompiledProgram;
use sandstorm_binary::Layout;
use sandstorm_binary::Memory;
use sandstorm_binary::RegisterStates;
use sandstorm_layouts::CairoWitness;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;
use structopt::StructOpt;

use wasm_bindgen::prelude::*;

/// Modulus of Starkware's 252-bit prime field used for Cairo
const STARKWARE_PRIME_HEX_STR: &str =
    "0x800000000000011000000000000000000000000000000000000000000000001";

#[wasm_bindgen]
pub fn run_prover(
    program: &str,
    air_public_input: &str,
    air_private_input: &str,
    memory: Vec<u8>,
    trace: Vec<u8>,
) -> Vec<u8> {
    let program_json: serde_json::Value = serde_json::from_str(program).unwrap();
    let prime: String = serde_json::from_value(program_json["prime"].clone()).unwrap();

    match prime.to_lowercase().as_str() {
        STARKWARE_PRIME_HEX_STR => {
            use p3618502788666131213697322783095070105623107215331596699973092056135872020481::ark::Fp;
            let program: CompiledProgram<Fp> = serde_json::from_value(program_json).unwrap();
            let air_public_input: AirPublicInput<Fp> =
                serde_json::from_str(air_public_input).unwrap();
            let private_input: AirPrivateInput = serde_json::from_str(air_private_input).unwrap();
            let register_states = RegisterStates::from_reader(trace.as_slice());
            let memory = Memory::from_reader(memory.as_slice());
            let options = ProofOptions::new(
                65,
                2,
                16,
                8,
                16,
            );
            match air_public_input.layout {
                Layout::Starknet => {
                    use claims::starknet::EthVerifierClaim;
                    let claim = EthVerifierClaim::new(program, air_public_input);
                    prove(options, private_input, claim, register_states, memory)
                }
                Layout::Recursive => {
                    use claims::recursive::CairoVerifierClaim;
                    let claim = CairoVerifierClaim::new(program, air_public_input);
                    prove(options, private_input, claim, register_states, memory)
                }
                _ => unimplemented!(),
            }
        }
        prime => unimplemented!("prime field p={prime} is not supported yet. Consider enabling the \"experimental_claims\" feature."),
    }
}

fn prove<Fp: PrimeField, Claim: Stark<Fp = Fp, Witness = CairoWitness<Fp>>>(
    options: ProofOptions,
    private_input: AirPrivateInput,
    claim: Claim,
    register_states: RegisterStates,
    memory: Memory<Fp>,
) -> Vec<u8> {
    let witness = CairoWitness::new(private_input, register_states, memory);

    let now = Instant::now();
    let proof = pollster::block_on(claim.prove(options, witness)).unwrap();
    println!("Proof generated in: {:?}", now.elapsed());
    let security_level_bits = proof.security_level_bits();
    println!("Proof security (conjectured): {security_level_bits}bit");

    let mut proof_bytes = Vec::new();
    proof.serialize_compressed(&mut proof_bytes).unwrap();
    println!("Proof size: {:?}KB", proof_bytes.len() / 1024);
    proof_bytes
}

fn prove_old<Fp: PrimeField, Claim: Stark<Fp = Fp, Witness = CairoWitness<Fp>>>(
    options: ProofOptions,
    private_input_path: &PathBuf,
    output_path: &PathBuf,
    claim: Claim,
) {
    let private_input_file =
        File::open(private_input_path).expect("could not open private input file");
    let private_input: AirPrivateInput = serde_json::from_reader(private_input_file).unwrap();

    let trace_path = &private_input.trace_path;
    let trace_file = File::open(trace_path).expect("could not open trace file");
    let register_states = RegisterStates::from_reader(trace_file);

    let memory_path = &private_input.memory_path;
    let memory_file = File::open(memory_path).expect("could not open memory file");
    let memory = Memory::from_reader(memory_file);

    let witness = CairoWitness::new(private_input, register_states, memory);

    let now = Instant::now();
    let proof = pollster::block_on(claim.prove(options, witness)).unwrap();
    println!("Proof generated in: {:?}", now.elapsed());
    let security_level_bits = proof.security_level_bits();
    println!("Proof security (conjectured): {security_level_bits}bit");

    let mut proof_bytes = Vec::new();
    proof.serialize_compressed(&mut proof_bytes).unwrap();
    println!("Proof size: {:?}KB", proof_bytes.len() / 1024);
    let mut f = File::create(output_path).unwrap();
    f.write_all(proof_bytes.as_slice()).unwrap();
    f.flush().unwrap();
    println!("Proof written to {}", output_path.as_path().display());
}
