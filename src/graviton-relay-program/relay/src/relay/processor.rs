use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::invoke_signed,
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
};

use spl_token::{
    instruction::transfer,
    state::Mint,
};

use crate::relay::instruction::RelayContractInstruction;
use crate::relay::state::RelayContract;
use gravity_misc::ports::error::PortError;
use gravity_misc::ports::state::{PortOperationIdentifier, ForeignAddress};
use gravity_misc::validation::{PDAResolver, TokenMintConstrained, validate_pubkey_match, validate_contract_emptiness};


pub struct RelayProcessor;

impl RelayProcessor {
    fn process_init_relay_contract(
        accounts: &[AccountInfo],
        token_address: &Pubkey,
        token_mint: &Pubkey,
        nebula_address: &Pubkey,
        oracles: &Vec<Pubkey>,
        _program_id: &Pubkey,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();

        let initializer = next_account_info(account_info_iter)?;
        if !initializer.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }

        let relay_contract_account = next_account_info(account_info_iter)?;

        validate_contract_emptiness(&relay_contract_account.try_borrow_data()?[0..3000])?;

        let mut relay_contract_info = RelayContract::default();

        relay_contract_info.is_state_initialized = true;
        relay_contract_info.token_address = *token_address;
        relay_contract_info.token_mint = *token_mint;
        relay_contract_info.nebula_address = *nebula_address;
        relay_contract_info.oracles = oracles.clone();
        relay_contract_info.initializer_pubkey = *initializer.key;

        msg!("instantiated ib port contract");

        msg!("packing ib port contract");

        RelayContract::pack(
            relay_contract_info,
            &mut relay_contract_account.try_borrow_mut_data()?[0..RelayContract::LEN],
        )?;

        Ok(())
    }

    fn process_create_transfer_unwrap_request(
        accounts: &[AccountInfo],
        request_id: &[u8; 16],
        ui_amount: f64,
        foreign_receiver: &ForeignAddress,
        _program_id: &Pubkey,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();

        let initializer = next_account_info(account_info_iter)?;
        if !initializer.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }

        let relay_contract_account = next_account_info(account_info_iter)?;

        let mut relay_contract_info =
            RelayContract::unpack(&relay_contract_account.data.borrow()[0..RelayContract::LEN])?;


        let token_program_id = next_account_info(account_info_iter)?;

        if *token_program_id.key != relay_contract_info.token_address {
            return Err(PortError::InvalidInputToken.into());
        }

        // common token info
        let mint = next_account_info(account_info_iter)?;

        let token_mint_info = Mint::unpack(&mint.data.borrow()[0..Mint::LEN])?;

        let amount = spl_token::ui_amount_to_amount(ui_amount, token_mint_info.decimals);

        let token_holder = next_account_info(account_info_iter)?;
        let token_receiver = next_account_info(account_info_iter)?;

        relay_contract_info.validate_token_mint(mint.key)?;

        // lock tockens
        let transfer_ix = transfer(
            &token_program_id.key,
            &token_holder.key,
            &token_receiver.key,
            &initializer.key,
            &[],
            amount
        )?;

        invoke_signed(
            &transfer_ix,
            &[
                token_holder.clone(),
                token_receiver.clone(),
                initializer.clone(),
                token_program_id.clone(),
            ],
            &[&[PDAResolver::Gravity.bump_seeds()]],
        )?;

        msg!("saving request info");
        relay_contract_info.create_transfer_wrap_request(request_id, amount, token_holder.key, foreign_receiver)?;

        RelayContract::pack(
            relay_contract_info,
            &mut relay_contract_account.try_borrow_mut_data()?[0..RelayContract::LEN],
        )?;

        Ok(())
    }

    fn validate_data_provider(
        multisig_owner_keys: &Vec<Pubkey>,
        data_provider: &Pubkey,
    ) -> Result<(), PortError> {

        validate_pubkey_match(
            multisig_owner_keys,
            data_provider,
            PortError::AccessDenied
        )
    }

    fn process_attach_value<'a, 't: 'a>(
        accounts: &[AccountInfo<'t>],
        byte_data: &Vec<u8>,
        _program_id: &Pubkey,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();

        msg!("got the attach!");
        let initializer = next_account_info(account_info_iter)?;

        // TODO: Caller validation (1)
        if !initializer.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }

        let relay_contract_account = next_account_info(account_info_iter)?;

        let mut relay_contract_info =
            RelayContract::unpack(&relay_contract_account.data.borrow()[0..RelayContract::LEN])?;

        Self::validate_data_provider(
            &relay_contract_info.oracles,
            initializer.key,
        )?;

        // Get the accounts to unlock
        let token_program_id = next_account_info(account_info_iter)?;
        let mint = next_account_info(account_info_iter)?;
        let recipient_account = next_account_info(account_info_iter)?;
        let pda_account = next_account_info(account_info_iter)?;

        let _ = next_account_info(account_info_iter)?;
        
        let token_holder = next_account_info(account_info_iter)?;

        relay_contract_info.validate_token_mint(mint.key)?;

        msg!("Creating unlock IX");

        let mut amount: u64 = 0;

        let token_mint_info = Mint::unpack(&mint.data.borrow()[0..Mint::LEN])?;
        let operation = relay_contract_info.attach_data(byte_data, recipient_account.key, &mut amount, &token_mint_info)?;

        if operation == PortOperationIdentifier::UNLOCK {
            let transfer_ix = transfer(
                &token_program_id.key,
                &token_holder.key,
                &recipient_account.key,
                &pda_account.key,
                &[],
                amount,
            )?;

            invoke_signed(
                &transfer_ix,
                &[
                    token_holder.clone(),
                    recipient_account.clone(),
                    pda_account.clone(),
                    token_program_id.clone(),
                ],
                &[&[PDAResolver::Gravity.bump_seeds()]]
            )?;
        }

        RelayContract::pack(
            relay_contract_info,
            &mut relay_contract_account.try_borrow_mut_data()?[0..RelayContract::LEN],
        )?;

        Ok(())
    }

    pub fn process(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> ProgramResult {
        let instruction = RelayContractInstruction::unpack(instruction_data)?;

        match instruction {
            RelayContractInstruction::InitContract {
                token_address,
                token_mint,
                nebula_address,
                oracles,
            } => {
                msg!("Instruction: Init IB Port Contract");

                Self::process_init_relay_contract(
                    accounts,
                    &token_address,
                    &token_mint,
                    &nebula_address,
                    &oracles,
                    program_id,
                )
            }
            RelayContractInstruction::CreateTransferUnwrapRequest {
                request_id,
                amount,
                receiver,
            } => {
                msg!("Instruction: CreateTransferUnwrapRequest");

                Self::process_create_transfer_unwrap_request(
                    accounts,
                    &request_id,
                    amount,
                    &receiver,
                    program_id,
                )
            }
            RelayContractInstruction::AttachValue {
                byte_data
            } => {
                msg!("Instruction: AttachValue");

                Self::process_attach_value(
                    accounts,
                    &byte_data,
                    program_id,
                )
            }
        }
    }    
}
