use solana_program::{
    msg,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::Pubkey,
};

use solana_gravity_contract::gravity::state::PartialStorage;
use spl_token::state::Mint;

use gravity_misc::model::{AbstractRecordHandler, RecordHandler};
use gravity_misc::validation::TokenMintConstrained;
use gravity_misc::ports::state::{
    GenericRequest,
    GenericPortOperation,
    RequestsQueue, 
    RequestCountConstrained,
    RequestStatus,
    ForeignAddress,
    PortOperationIdentifier
};

use arrayref::array_ref;
use borsh::{BorshDeserialize, BorshSerialize};

use gravity_misc::ports::error::PortError;


// pub type WrapRequest = GenericRequest<Pubkey, ForeignAddress>;

/**
 *  bytes32 topic0 = bytesToBytes32(impactData, 40); // [ 40: 72]
    bytes memory token = impactData[84:104]; // [ 72:104][12:32]
    bytes memory sender = impactData[116:136]; // [104:136][12:32]
    bytes memory receiver = impactData[148:168]; // [136:168][12:32]
    uint256 amount = deserializeUint(impactData, 168, 32); // [168:200]
 */

pub struct WrapRequest {
    pub token_mint: [u8; 32],
    pub origin_address: Pubkey,
    pub destination_address: ForeignAddress,
    pub destination_chain: [u8;3],
    pub amount: u64,
}

macro_rules! pack {
    ($struct_name: ident, len: $len: expr) => {
        impl Pack for $struct_name {
            const LEN: usize = $len;

            fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
                let mut mut_src: &[u8] = src;
                Self::deserialize(&mut mut_src).map_err(|err| {
                    msg!(
                        "Error: failed to deserialize RelayContract instruction: {}",
                        err
                    );
                    ProgramError::InvalidInstructionData
                })
            }

            fn pack_into_slice(&self, dst: &mut [u8]) {
                let data = self.try_to_vec().unwrap();
                dst[..data.len()].copy_from_slice(&data);
            }
        }
    };
}

pack!(WrapRequest,len: 1000);


#[repr(C)]
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Default, Debug, Clone)]
pub struct RelayContract {
    pub nebula_address: Pubkey, // distinct nebula address (not nebula data account)
    pub token_address: Pubkey,
    pub token_mint: Pubkey, // common token info, (result of spl-token create-token or as it so called - 'the mint')
    pub initializer_pubkey: Pubkey,
    pub oracles: Vec<Pubkey>,

    pub swap_status: RecordHandler<[u8; 16], RequestStatus>,
    pub requests: RecordHandler<[u8; 16], WrapRequest>,

    pub is_state_initialized: bool,

    pub requests_queue: RequestsQueue<[u8; 16]>,
}

pack!(RelayContract, len: 2000);

impl RequestCountConstrained for RelayContract {
    const MAX_IDLE_REQUESTS_COUNT: usize = 100;

    fn count_constrained_entities(&self) -> Vec<usize> {
        vec![
            self.unprocessed_burn_requests()
        ]
    }
}

impl TokenMintConstrained<PortError> for RelayContract {

    fn bound_token_mint(&self) -> (Pubkey, PortError) {
        return (
            self.token_mint,
            PortError::InvalidTokenMint
        )
    }
}

impl PartialStorage for RelayContract {
    const DATA_RANGE: std::ops::Range<usize> = 0..2000;
}

impl Sealed for RelayContract {} 

impl IsInitialized for RelayContract {
    fn is_initialized(&self) -> bool {
        self.is_state_initialized
    }
}


    fn pack_into_slice(&self, dst: &mut [u8]) {
        let data = self.try_to_vec().unwrap();
        dst[..data.len()].copy_from_slice(&data);
    }
}


pub type PortOperation<'a> = GenericPortOperation<'a, ForeignAddress>;

impl RelayContract {

    fn unprocessed_burn_requests(&self) -> usize {
        self.requests.len()
    }

    fn validate_requests_count(&self) -> Result<(), PortError> {
        if !self.count_is_below_limit() {
            return Err(PortError::TransferRequestsCountLimit);
        }
        Ok(())
    }

    pub fn unpack_byte_array(byte_data: &Vec<u8>) -> Result<PortOperation, ProgramError> {
        if byte_data.len() < 57 {
            return Err(PortError::ByteArrayUnpackFailed.into());
        }

        let mut pos = 0;
        let action = byte_data[pos];
        pos += 1;

        let swap_id = array_ref![byte_data, pos, 16];
        pos += 16;
        
        let raw_amount = array_ref![byte_data, pos, 8];
        pos += 8;

        let receiver = array_ref![byte_data, pos, 32];

        return Ok(PortOperation {
            action,
            swap_id,
            amount: raw_amount,
            receiver
        });
    }

    pub fn attach_data<'a>(&mut self, byte_data: &'a Vec<u8>, input_pubkey: &'a Pubkey, input_amount: &'a mut u64, token_mint_info: &Mint) -> Result<String, ProgramError> {
        let action = &[byte_data[0]];

        let command_char = std::str::from_utf8(action).unwrap();

        let amount = byte_data[200..232];
        let receiver = byte_data[328..360];

    }

    pub fn create_transfer_wrap_request(&mut self, record_id: &[u8; 16], amount: u64, sender_data_account: &Pubkey, receiver: &ForeignAddress) -> Result<(), PortError>  {
        self.validate_requests_count()?;

        if self.requests.contains_key(record_id) {
            return Err(PortError::RequestIDIsAlreadyBeingProcessed.into());
        }

        self.requests.insert(*record_id, WrapRequest {
            destination_address: *receiver,
            origin_address: *sender_data_account,
            amount
        });
        self.swap_status.insert(*record_id, RequestStatus::New);
        self.requests_queue.push(*record_id);

        Ok(())
    }
}
