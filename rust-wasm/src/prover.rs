use ark_ff::Field;
use ark_ff::PrimeField;
use ark_serialize::CanonicalDeserialize;
use ark_serialize::CanonicalSerialize;
use sandstorm_binary::AirPrivateInput;
use sandstorm_binary::AirPublicInput;
use sandstorm_binary::CompiledProgram;
use sandstorm_binary::Layout;
use sandstorm_binary::Memory;
use sandstorm_binary::RegisterStates;
use sandstorm_layouts::CairoWitness;
use ministark::stark::Stark;
use ministark::Proof;
use ministark::ProofOptions;
use ministark_gpu::fields::p3618502788666131213697322783095070105623107215331596699973092056135872020481;
use sandstorm::claims;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;
use structopt::StructOpt;

fn run_prover(program: PathBuf, air_public_input: PathBuf) {
    let program_file = File::open(program).expect("could not open program file");
    let air_public_input_file = File::open(air_public_input).expect("could not open public input");
    let program_json: serde_json::Value = serde_json::from_reader(program_file).unwrap();
    let prime: String = serde_json::from_value(program_json["prime"].clone()).unwrap();

    match prime.to_lowercase().as_str() {
        STARKWARE_PRIME_HEX_STR => {
            use p3618502788666131213697322783095070105623107215331596699973092056135872020481::ark::Fp;
            let program: CompiledProgram<Fp> = serde_json::from_value(program_json).unwrap();
            let air_public_input: AirPublicInput<Fp> =
                serde_json::from_reader(air_public_input_file).unwrap();
            match air_public_input.layout {
                Layout::Starknet => {
                    use claims::starknet::EthVerifierClaim;
                    let claim = EthVerifierClaim::new(program, air_public_input);
                    let options = ProofOptions::new(
                        65,
                        2,
                        16,
                        8,
                        16,
                    );
                    prove(options, claim);
                }
                Layout::Recursive => {
                    use claims::recursive::CairoVerifierClaim;
                    let claim = CairoVerifierClaim::new(program, air_public_input);
                    let options = ProofOptions::new(
                        65,
                        2,
                        16,
                        8,
                        16,
                    );
                    prove(options, claim);
                }
                _ => unimplemented!(),
            }
        }
        #[cfg(feature = "experimental_claims")]
        GOLDILOCKS_PRIME_HEX_STR => {
            use ministark::hash::Sha256HashFn;
            use ministark::merkle::MatrixMerkleTreeImpl;
            use ministark::random::PublicCoinImpl;
            use ministark_gpu::fields::p18446744069414584321;
            use p18446744069414584321::ark::Fp;
            use p18446744069414584321::ark::Fq3;
            use sandstorm::CairoClaim;
            let program: CompiledProgram<Fp> = serde_json::from_value(program_json).unwrap();
            let air_public_input: AirPublicInput<Fp> =
                serde_json::from_reader(air_public_input_file).unwrap();
            match air_public_input.layout {
                Layout::Plain => {
                    type A = layouts::plain::AirConfig<Fp, Fq3>;
                    type T = layouts::plain::ExecutionTrace<Fp, Fq3>;
                    type M = MatrixMerkleTreeImpl<Sha256HashFn>;
                    type P = PublicCoinImpl<Fq3, Sha256HashFn>;
                    type C = CairoClaim<Fp, A, T, M, P>;
                    let claim = C::new(program, air_public_input);
                    execute_command(command, claim);
                }
                Layout::Starknet => {
                    unimplemented!("'starknet' layout does not support Goldilocks field")
                }
                Layout::Recursive => {
                    unimplemented!("'recursive' layout does not support Goldilocks field")
                }
                layout => unimplemented!("layout {layout} is not supported yet"),
            }
        }
        prime => unimplemented!("prime field p={prime} is not supported yet. Consider enabling the \"experimental_claims\" feature."),
    }
}

fn prove<Fp: PrimeField, Claim: Stark<Fp = Fp, Witness = CairoWitness<Fp>>>(
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
