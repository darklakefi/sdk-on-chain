use anchor_lang::Result;
use password_hash::rand_core::{OsRng, RngCore};
use solana_sdk::{clock::Clock, sysvar::Sysvar};
use spl_token_2022::extension::transfer_fee::TransferFeeConfig;

pub fn get_transfer_fee(
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
pub fn generate_random_salt() -> [u8; 8] {
    let mut rng = OsRng;
    let mut salt = [0u8; 8];
    rng.fill_bytes(&mut salt);
    salt
}
