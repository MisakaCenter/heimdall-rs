use std::fs;

use crate::{
    constants::{ADDRESS_REGEX, BYTECODE_REGEX},
    ether::{evm::core::opcodes::opcode, rpc::get_code},
    io::logging::*,
    utils::strings::{decode_hex, encode_hex},
};
use clap::{AppSettings, Parser};

#[derive(Debug, Clone, Parser)]
#[clap(about = "Disassemble EVM bytecode to Assembly",
       after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
       global_setting = AppSettings::DeriveDisplayOrder,
       override_usage = "heimdall disassemble <TARGET> [OPTIONS]")]
pub struct DisassemblerArgs {
    /// The target to disassemble, either a file, bytecode, contract address, or ENS name.
    #[clap(required = true)]
    pub target: String,

    /// Set the output verbosity level, 1 - 5.
    #[clap(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,

    /// The RPC provider to use for fetching target bytecode.
    #[clap(long = "rpc-url", short, default_value = "", hide_default_value = true)]
    pub rpc_url: String,

    /// Whether to use base-10 for the program counter.
    #[clap(long = "decimal-counter", short = 'd')]
    pub decimal_counter: bool,
}

pub async fn disassemble(args: DisassemblerArgs) -> Result<String, Box<dyn std::error::Error>> {
    use std::time::Instant;
    let now = Instant::now();

    let (logger, _) = Logger::new(match args.verbose.log_level() {
        Some(level) => level.as_str(),
        None => "SILENT",
    });

    let contract_bytecode: String;
    if ADDRESS_REGEX.is_match(&args.target)? {
        // We are disassembling a contract address, so we need to fetch the bytecode from the RPC
        // provider.
        contract_bytecode = get_code(&args.target, &args.rpc_url).await?;
    } else if BYTECODE_REGEX.is_match(&args.target)? {
        contract_bytecode = args.target;
    } else {
        // We are disassembling a file, so we need to read the bytecode from the file.
        contract_bytecode = match fs::read_to_string(&args.target) {
            Ok(contents) => {
                let _contents = contents.replace('\n', "");
                if BYTECODE_REGEX.is_match(&_contents)? && _contents.len() % 2 == 0 {
                    _contents
                } else {
                    logger
                        .error(&format!("file '{}' doesn't contain valid bytecode.", &args.target));
                    std::process::exit(1)
                }
            }
            Err(_) => {
                logger.error(&format!("failed to open file '{}' .", &args.target));
                std::process::exit(1)
            }
        };
    }

    let mut program_counter = 0;
    let mut output: String = String::new();

    // Iterate over the bytecode, disassembling each instruction.
    let byte_array = decode_hex(&contract_bytecode.replacen("0x", "", 1))?;

    while program_counter < byte_array.len() {
        let operation = opcode(byte_array[program_counter]);
        let mut pushed_bytes: String = String::new();

        if operation.name.contains("PUSH") {
            let byte_count_to_push: u8 = operation.name.strip_prefix("PUSH").unwrap().parse()?;

            pushed_bytes = match byte_array
                .get(program_counter + 1..program_counter + 1 + byte_count_to_push as usize)
            {
                Some(bytes) => encode_hex(bytes.to_vec()),
                None => break,
            };
            program_counter += byte_count_to_push as usize;
        }

        output.push_str(
            format!(
                "{} {} {}\n",
                if args.decimal_counter {
                    program_counter.to_string()
                } else {
                    format!("{:06x}", program_counter)
                },
                operation.name,
                pushed_bytes
            )
            .as_str(),
        );
        program_counter += 1;
    }

    logger.info(&format!("disassembled {program_counter} bytes successfully."));
    logger.debug(&format!("disassembly completed in {} ms.", now.elapsed().as_millis()));

    Ok(output)
}
