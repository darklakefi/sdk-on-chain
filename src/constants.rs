use anchor_client::solana_sdk::pubkey::Pubkey;
use lazy_static::lazy_static;
use solana_sdk::pubkey;

pub const MAX_PERCENTAGE: u64 = 1_000_000; // 100% in basis points

pub const DARKLAKE_PROGRAM_ID: Pubkey = pubkey!("darkr3FB87qAZmgLwKov6Hk9Yiah5UT4rUYu8Zhthw1");

// SEEDS
pub const POOL_SEED: &[u8] = b"pool";
pub const AMM_CONFIG_SEED: &[u8] = b"amm_config";
pub const AUTHORITY_SEED: &[u8] = b"authority";
pub const POOL_WSOL_RESERVE_SEED: &[u8] = b"pool_wsol_reserve";
pub const ORDER_SEED: &[u8] = b"order";
pub const LIQUIDITY_SEED: &[u8] = b"lp";
pub const ORDER_WSOL_SEED: &[u8] = b"order_wsol";

// only used during pool initialization
// pub const POOL_RESERVE_SEED: &[u8] = b"pool_reserve";

lazy_static! {
    pub static ref AMM_CONFIG: Pubkey = Pubkey::find_program_address(
        &[AMM_CONFIG_SEED, &0u32.to_le_bytes()],
        &DARKLAKE_PROGRAM_ID,
    )
    .0;
}