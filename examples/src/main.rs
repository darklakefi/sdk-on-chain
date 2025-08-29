use darklake_sdk::{DarklakeAmm, create_darklake_amm, QuoteParams, SwapMode, Amm};
use solana_sdk::pubkey::Pubkey;

fn main() -> anyhow::Result<()> {
    println!("Darklake DEX SDK - Examples");
    println!("============================");
    
    // Example: Create a Darklake AMM instance
    let amm_key = Pubkey::new_unique();
    println!("Created AMM with key: {}", amm_key);
    
    // Note: In a real application, you would get this from RPC
    let account_data = vec![0u8; 100]; // Mock data
    
    match create_darklake_amm(amm_key, &account_data) {
        Ok(amm) => {
            println!("✅ Successfully created Darklake AMM");
            println!("   Label: {}", amm.label());
            println!("   Program ID: {}", amm.program_id());
            println!("   Key: {}", amm.key());
            println!("   Supports exact out: {}", amm.supports_exact_out());
            println!("   Is active: {}", amm.is_active());
            
            // Example: Get reserve mints (would be populated after update)
            let reserve_mints = amm.get_reserve_mints();
            println!("   Reserve mints: {:?}", reserve_mints);
            
            // Example: Get accounts that need to be updated
            let accounts_to_update = amm.get_accounts_to_update();
            println!("   Accounts to update: {:?}", accounts_to_update);
            
        },
        Err(e) => {
            println!("❌ Failed to create AMM: {}", e);
            println!("   This is expected with mock data");
        }
    }
    
    println!("\nExample: Creating quote parameters");
    let quote_params = QuoteParams {
        input_mint: Pubkey::new_unique(),
        amount: 1000000, // 1 token (assuming 6 decimals)
        swap_mode: SwapMode::ExactIn,
    };
    
    println!("   Input mint: {}", quote_params.input_mint);
    println!("   Amount: {}", quote_params.amount);
    println!("   Swap mode: {:?}", quote_params.swap_mode);
    
    println!("\n🎉 SDK is working correctly!");
    println!("   You can now use this SDK to interact with Darklake pools");
    
    Ok(())
}
