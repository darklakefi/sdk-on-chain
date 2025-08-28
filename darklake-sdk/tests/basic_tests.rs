use darklake_sdk::{DarklakeAmm, DARKLAKE_PROGRAM_ID, create_darklake_amm, Amm};
use solana_sdk::pubkey::Pubkey;

#[test]
fn test_darklake_program_id() {
    // Verify the program ID is correctly set
    assert_eq!(
        DARKLAKE_PROGRAM_ID.to_string(),
        "darkr3FB87qAZmgLwKov6Hk9Yiah5UT4rUYu8Zhthw1"
    );
}

#[test]
fn test_create_darklake_amm() {
    // Test creating a Darklake AMM instance
    let key = Pubkey::new_unique();
    let account_data = vec![0u8; 100]; // Mock account data
    
    let result = create_darklake_amm(key, &account_data);
    // This should fail due to invalid account data, but we're testing the function exists
    assert!(result.is_err()); // Expected to fail with invalid data
}

#[test]
fn test_amm_trait_implementation() {
    // Test that DarklakeAmm implements the Amm trait
    let key = Pubkey::new_unique();
    let amm = DarklakeAmm {
        key,
        pool: darklake_sdk::darklake_amm::Pool {
            creator: Pubkey::default(),
            amm_config: Pubkey::default(),
            token_mint_x: Pubkey::default(),
            token_mint_y: Pubkey::default(),
            reserve_x: Pubkey::default(),
            reserve_y: Pubkey::default(),
            token_lp_supply: 0,
            protocol_fee_x: 0,
            protocol_fee_y: 0,
            locked_x: 0,
            locked_y: 0,
            user_locked_x: 0,
            user_locked_y: 0,
            bump: 0,
            padding: [0; 4],
        },
        amm_config: darklake_sdk::darklake_amm::AmmConfig {
            trade_fee_rate: 0,
            create_pool_fee: 0,
            protocol_fee_rate: 0,
            wsol_trade_deposit: 0,
            deadline_slot_duration: 0,
            ratio_change_tolerance_rate: 0,
            bump: 0,
            halted: false,
            padding: [0; 16],
        },
        reserve_x_balance: 0,
        reserve_y_balance: 0,
        token_x_owner: Pubkey::default(),
        token_y_owner: Pubkey::default(),
        token_x_transfer_fee_config: None,
        token_y_transfer_fee_config: None,
    };
    
    assert_eq!(amm.label(), "Darklake");
    assert_eq!(amm.program_id(), DARKLAKE_PROGRAM_ID);
    assert_eq!(amm.key(), key);
    assert_eq!(amm.supports_exact_out(), false);
    assert_eq!(amm.is_active(), true);
}
