use gravity_misc::ports::error::PortError;
use crate::relay::instruction::RelayContractInstruction;

pub fn allocation_by_instruction_index(
    instruction: usize,
    _oracles_bft: Option<usize>,
) -> Result<Vec<usize>, PortError> {
    Ok(match instruction {
        // InitContract
        0 => vec![
            RelayContractInstruction::PUBKEY_ALLOC,
            RelayContractInstruction::PUBKEY_ALLOC,
            RelayContractInstruction::PUBKEY_ALLOC,
            1,
        ],
        // CreateTransferUnwrapRequest
        1 => vec![
            RelayContractInstruction::DEST_AMOUNT_ALLOC,
            RelayContractInstruction::FOREIGN_ADDRESS_ALLOC,
            16,
            1,
        ],
        // AttachValue
        2 => vec![RelayContractInstruction::ATTACHED_DATA_ALLOC],
        // ConfirmDestinationChainRequest
        3 => vec![RelayContractInstruction::ATTACHED_DATA_ALLOC],
        // TransferTokenOwnership
        4 => vec![
            RelayContractInstruction::PUBKEY_ALLOC,
            RelayContractInstruction::PUBKEY_ALLOC,
        ],
        _ => return Err(PortError::InvalidInstructionIndex.into()),
    })
}
