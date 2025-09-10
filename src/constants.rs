use anchor_client::solana_sdk::pubkey::Pubkey;
use lazy_static::lazy_static;
use solana_sdk::pubkey;

// pub const MAX_PERCENTAGE: u64 = 1_000_000; // 100% in basis points

pub const DARKLAKE_PROGRAM_ID: Pubkey = pubkey!("darkr3FB87qAZmgLwKov6Hk9Yiah5UT4rUYu8Zhthw1");

pub const SOL_MINT: Pubkey = pubkey!("So11111111111111111111111111111111111111111");
pub const METADATA_PROGRAM_ID: Pubkey = pubkey!("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");
pub const DEVNET_CREATE_POOL_FEE_VAULT: Pubkey =
    pubkey!("6vUjEKC5mkiDMdMhkxV8SYzPQAk39aPKbjGataVnkUss");
pub const MAINNET_CREATE_POOL_FEE_VAULT: Pubkey =
    pubkey!("HNQdnRgtnsgcx7E836nZ1JwrQstWBEJMnRVy8doY366A");

// SEEDS
pub const POOL_SEED: &[u8] = b"pool";
pub const AMM_CONFIG_SEED: &[u8] = b"amm_config";
pub const AUTHORITY_SEED: &[u8] = b"authority";
pub const POOL_WSOL_RESERVE_SEED: &[u8] = b"pool_wsol_reserve";
pub const ORDER_SEED: &[u8] = b"order";
pub const LIQUIDITY_SEED: &[u8] = b"lp";
pub const ORDER_WSOL_SEED: &[u8] = b"order_wsol";
pub const METADATA_SEED: &[u8] = b"metadata";

// only used during pool initialization
// pub const POOL_RESERVE_SEED: &[u8] = b"pool_reserve";

lazy_static! {
    pub static ref AMM_CONFIG: Pubkey = Pubkey::find_program_address(
        &[AMM_CONFIG_SEED, &0u32.to_le_bytes()],
        &DARKLAKE_PROGRAM_ID,
    )
    .0;
    pub static ref AUTHORITY: Pubkey =
        Pubkey::find_program_address(&[AUTHORITY_SEED], &DARKLAKE_PROGRAM_ID).0;
}
