use anchor_lang::{solana_program::example_mocks::solana_sdk::system_instruction, Result};
use anchor_spl::token::spl_token::instruction::{close_account, sync_native};
use anyhow::Result as AnyhowResult;
use password_hash::rand_core::{OsRng, RngCore};
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    address_lookup_table::state::AddressLookupTable, clock::Clock, instruction::Instruction,
    message::AddressLookupTableAccount, pubkey::Pubkey, sysvar::Sysvar,
};
use spl_associated_token_account::get_associated_token_address;
use spl_token::native_mint;
use spl_token_2022::extension::transfer_fee::TransferFeeConfig;

use crate::{constants::DEVNET_LOOKUP, MAINNET_LOOKUP};

pub(crate) fn get_transfer_fee(
    transfer_fee_config: Option<TransferFeeConfig>,
    pre_fee_amount: u64,
) -> Result<u64> {
    if transfer_fee_config.is_none() {
        return Ok(0);
    }

    let fee = transfer_fee_config
        .unwrap()
        .calculate_epoch_fee(Clock::get()?.epoch, pre_fee_amount)
        .unwrap();

    Ok(fee)
}

/// Generate a random 8-byte salt for order uniqueness
pub(crate) fn generate_random_salt() -> [u8; 8] {
    let mut rng = OsRng;
    let mut salt = [0u8; 8];
    rng.fill_bytes(&mut salt);
    salt
}

pub(crate) fn get_wrap_sol_to_wsol_instructions(
    payer: Pubkey,
    amount_in_lamports: u64,
) -> Result<Vec<Instruction>> {
    let mut instructions = Vec::new();

    let token_mint_wsol = native_mint::ID;
    let token_program_id = spl_token::ID;

    // 1. Get the associated token account for WSOL
    let wsol_ata = get_associated_token_address(&payer, &token_mint_wsol);

    // 2. Create instructions (in case the WSOL ATA doesn't exist)
    let create_ata_ix =
        spl_associated_token_account::instruction::create_associated_token_account_idempotent(
            &payer,
            &payer,
            &token_mint_wsol,
            &token_program_id,
        );

    // 3. Transfer SOL to the ATA
    let transfer_sol_ix = system_instruction::transfer(&payer, &wsol_ata, amount_in_lamports);

    // 4. Sync the ATA to mark it as wrapped
    let sync_native_ix = sync_native(&token_program_id, &wsol_ata)?;

    instructions.push(create_ata_ix);
    instructions.push(transfer_sol_ix);
    instructions.push(sync_native_ix);

    Ok(instructions)
}

pub(crate) fn get_close_wsol_instructions(payer: Pubkey) -> Result<Vec<Instruction>> {
    let mut instructions = Vec::new();

    let token_mint_wsol = native_mint::ID;
    let token_program_id = spl_token::ID;

    let wsol_ata = get_associated_token_address(&payer, &token_mint_wsol);

    // 1. Sync the ATA to ensure all lamports are accounted for
    let sync_native_ix = sync_native(&token_program_id, &wsol_ata)?;

    // 3. Close the WSOL token account
    let close_account_ix = close_account(&token_program_id, &wsol_ata, &payer, &payer, &[])?;

    instructions.push(sync_native_ix);
    instructions.push(close_account_ix);

    Ok(instructions)
}

pub fn convert_string_to_bytes_array(s: &str, length: usize) -> AnyhowResult<Vec<u8>> {
    let mut bytes = s.to_string().into_bytes();
    if bytes.len() > length {
        return Err(anyhow::anyhow!(
            "String length must be less than or equal to {}.",
            length
        ));
    }

    bytes.resize(length, 0u8);
    Ok(bytes)
}

pub async fn get_address_lookup_table(
    rpc_client: &RpcClient,
    is_devnet: bool,
) -> AnyhowResult<AddressLookupTableAccount> {
    let alt_pubkey = if is_devnet {
        DEVNET_LOOKUP
    } else {
        MAINNET_LOOKUP
    };

    let alt_account = rpc_client
        .get_account(&alt_pubkey)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get address lookup table: {}", e))?;

    let table = AddressLookupTable::deserialize(&alt_account.data)?;

    let address_lookup_table = AddressLookupTableAccount {
        key: alt_pubkey,
        addresses: table.addresses.to_vec(),
    };

    Ok(address_lookup_table)
}
