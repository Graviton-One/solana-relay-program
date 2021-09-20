use solana_program::{
    account_info::AccountInfo,
    entrypoint,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
};

use crate::relay::processor::RelayProcessor;

pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    RelayProcessor::process(program_id, accounts, instruction_data)
}

entrypoint!(process);
