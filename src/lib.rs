//! # Darklake DEX SDK
//!
//! A standalone SDK for interacting with Darklake AMM pools on Solana. This SDK provides two main usage flows:
//!
//! 1. **Transaction Functions (`_tx`)**: Return fully formatted transaction that can be signed and sent
//! 2. **Instruction Functions (`_ix`)**: Return core instruction, allowing users to manage additional calls as needed
//!
//! > **📚 Detailed Examples**: For comprehensive examples and advanced usage patterns, see the [SDK Examples Repository](https://github.com/darklakefi/sdk-on-chain-examples).
//!
//! ## Internal State Management
//!
//! The SDK includes internal chain state tracking functions:
//! - **`load_pool`**: Loads pool data for internal state tracking
//! - **`update_accounts`**: Updates internal state with latest chain data
//! - **`get_order`**: Exception helper that bypasses internal cache and fetches the latest order state directly from the chain
//!
//! ## 🚀 Quick Start
//!
//! ### Installation
//!
//! Add this to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! darklake-sdk = "0.1.7"
//! ```
//!
//! ### Basic Setup
//!
//! ```rust
//! use darklake_sdk::DarklakeSDK;
//! use solana_sdk::commitment_config::CommitmentLevel;
//!
//! // Initialize the SDK (no longer requires a keypair)
//! let mut sdk = DarklakeSDK::new("https://api.devnet.solana.com", CommitmentLevel::Confirmed);
//! ```
//!
//! ## ⚠️ Important: SOL/WSOL Handling
//!
//! **The Darklake DEX does not support direct SOL pairs - only WSOL (Wrapped SOL) pairs are supported.**
//!
//! ### Automatic Handling (Transaction Functions)
//! The transaction functions (`swap_tx`, `add_liquidity_tx`, `remove_liquidity_tx`) automatically handle SOL/WSOL conversion by:
//! - Adding wrap instructions when SOL is provided as input
//! - Adding unwrap instructions when WSOL is received as output
//!
//! ### Manual Handling (Instruction Functions)
//! The instruction functions (`swap_ix`, `finalize_ix`, `add_liquidity_ix`, `remove_liquidity_ix`) **do not** automatically handle SOL/WSOL conversion. When using these methods:
//! - You must manually add wrap/unwrap instructions if needed
//! - Ensure proper WSOL token account management
//! - Supply the `unwrap_wsol` parameter in `FinalizeParams` if necessary or add a WSOL token account closing.
//!
//! ## 📖 Usage Patterns
//!
//! ### 1. Transaction Functions (`_tx`) - Fully Formatted Transactions
//!
//! These functions return complete transactions ready to be signed and sent:
//!
//! #### Trading (Swap)
//!
//! ```rust
//! use solana_sdk::pubkey::Pubkey;
//! use solana_sdk::signer::keypair::Keypair;
//!
//! // Define token mints
//! let token_in = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(); // WSOL (Wrapped SOL)
//! let token_out = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(); // USDC
//! let user_keypair = Keypair::new(); // Your wallet keypair
//! let user_pubkey = user_keypair.pubkey();
//!
//! // Get a quote first
//! let quote = sdk.quote(token_in, token_out, 1_000_000).await?; // 1 WSOL
//! println!("Expected output: {}", quote.out_amount);
//!
//! // Execute the swap - returns transaction and extra parameters needed for finalize
//! let (swap_tx, order_key, min_amount_out, salt) = sdk.swap_tx(
//!     token_in,
//!     token_out,
//!     1_000_000,  // 1 WSOL (in lamports)
//!     950_000,    // Minimum 0.95 USDC out (5% slippage)
//!     user_pubkey, // Token owner
//! ).await?;
//!
//! // Sign and send the swap transaction
//! let recent_blockhash = rpc_client.get_latest_blockhash().await?;
//! let swap_tx_signed = Transaction::new_signed_with_payer(
//!     &swap_tx.message.instructions,
//!     Some(&user_pubkey),
//!     &[&user_keypair],
//!     recent_blockhash,
//! );
//! let swap_signature = rpc_client.send_and_confirm_transaction(&swap_tx_signed)?;
//!
//! // Generate the finalize transaction using the returned parameters
//! let finalize_tx = sdk.finalize_tx(
//!     order_key,
//!     true, // unwrap_wsol if output is WSOL
//!     min_amount_out,
//!     salt,
//!     None, // settle_signer (optional, defaults to order owner)
//! ).await?;
//!
//! // Sign and send the finalize transaction
//! let finalize_tx_signed = Transaction::new_signed_with_payer(
//!     &finalize_tx.message.instructions,
//!     Some(&user_pubkey),
//!     &[&user_keypair],
//!     recent_blockhash,
//! );
//! let finalize_signature = rpc_client.send_and_confirm_transaction(&finalize_tx_signed)?;
//!
//! println!("Swap signature: {}", swap_signature);
//! println!("Finalize signature: {}", finalize_signature);
//! ```
//!
//! #### Adding Liquidity
//!
//! ```rust
//! let token_x = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(); // WSOL
//! let token_y = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(); // USDC
//! let user_keypair = Keypair::new(); // Your wallet keypair
//! let user_pubkey = user_keypair.pubkey();
//!
//! let tx = sdk.add_liquidity_tx(
//!     token_x,
//!     token_y,
//!     1_000_000,  // Max amount of token X
//!     1_000_000,  // Max amount of token Y
//!     1_000,      // LP token amount to mint
//!     user_pubkey, // User public key
//! ).await?;
//!
//! let recent_blockhash = rpc_client.get_latest_blockhash().await?;
//! let tx_signed = Transaction::new_signed_with_payer(
//!     &tx.message.instructions,
//!     Some(&user_pubkey),
//!     &[&user_keypair],
//!     recent_blockhash,
//! );
//! let signature = rpc_client.send_and_confirm_transaction(&tx_signed)?;
//! println!("Add liquidity signature: {}", signature);
//! ```
//!
//! #### Removing Liquidity
//!
//! ```rust
//! let tx = sdk.remove_liquidity_tx(
//!     token_x,
//!     token_y,
//!     500_000,    // Min amount of token X to receive
//!     500_000,    // Min amount of token Y to receive
//!     500,        // LP token amount to burn
//!     user_pubkey, // User public key
//! ).await?;
//!
//! let recent_blockhash = rpc_client.get_latest_blockhash().await?;
//! let tx_signed = Transaction::new_signed_with_payer(
//!     &tx.message.instructions,
//!     Some(&user_pubkey),
//!     &[&user_keypair],
//!     recent_blockhash,
//! );
//! let signature = rpc_client.send_and_confirm_transaction(&tx_signed)?;
//! println!("Remove liquidity signature: {}", signature);
//! ```
//!
//! #### Initializing Pool
//!
//! ```rust
//! let token_x = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(); // WSOL
//! let token_y = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(); // USDC
//! let user_keypair = Keypair::new(); // Your wallet keypair
//! let user_pubkey = user_keypair.pubkey();
//!
//! let tx = sdk.initialize_pool_tx(
//!     token_x,
//!     token_y,
//!     1_000_000,  // Amount of token X to deposit
//!     1_000_000,  // Amount of token Y to deposit
//!     user_pubkey, // User public key
//! ).await?;
//!
//! let recent_blockhash = rpc_client.get_latest_blockhash().await?;
//! let tx_signed = Transaction::new_signed_with_payer(
//!     &tx.message.instructions,
//!     Some(&user_pubkey),
//!     &[&user_keypair],
//!     recent_blockhash,
//! );
//! let signature = rpc_client.send_and_confirm_transaction(&tx_signed)?;
//! println!("Initialize pool signature: {}", signature);
//! ```
//!
//! ### 2. Instruction Functions (`_ix`) - Core Instructions
//!
//! These functions return core instructions, allowing you to manage additional calls as needed:
//!
//! #### Trading with Manual Control
//!
//! ```rust
//! use darklake_sdk::{SwapParams, FinalizeParams, SwapMode};
//! use solana_sdk::instruction::Instruction;
//! use solana_sdk::signer::keypair::Keypair;
//! use solana_sdk::commitment_config::CommitmentLevel;
//!
//! // Step 1: Load the pool (for internal state tracking)
//! sdk.load_pool(token_in, token_out).await?;
//!
//! // Step 2: Update accounts with latest data
//! sdk.update_accounts().await?;
//!
//! // Step 3: Create swap parameters
//! let swap_params = SwapParams {
//!     source_mint: token_in,
//!     destination_mint: token_out,
//!     token_transfer_authority: user_pubkey, // Your wallet's public key
//!     in_amount: 1_000_000,
//!     swap_mode: SwapMode::ExactIn,
//!     min_out: 950_000,
//!     salt: [1, 2, 3, 4, 5, 6, 7, 8], // Unique salt for this order
//! };
//!
//! // Step 4: Generate swap instruction
//! let swap_instruction = sdk.swap_ix(swap_params)?;
//!
//! // Step 5: Build and send the swap transaction
//! let swap_tx = Transaction::new_signed_with_payer(
//!     &[swap_instruction],
//!     Some(&user_pubkey),
//!     &[&user_keypair],
//!     recent_blockhash,
//! );
//! let swap_signature = rpc_client.send_and_confirm_transaction(&swap_tx)?;
//!
//! // Step 6: Get order data (bypasses internal cache for latest state)
//! let order = sdk.get_order(user_pubkey, CommitmentLevel::Confirmed).await?;
//!
//! // Step 7: Create finalize parameters
//! let finalize_params = FinalizeParams {
//!     settle_signer: user_pubkey,
//!     order_owner: user_pubkey,
//!     unwrap_wsol: true, // Set to true if output is WSOL and you want to unwrap it to SOL
//!     min_out: 950_000,
//!     salt: [1, 2, 3, 4, 5, 6, 7, 8],
//!     output: order.d_out,
//!     commitment: order.c_min,
//!     deadline: order.deadline,
//!     current_slot: rpc_client.get_slot()?,
//! };
//!
//! // Step 8: Generate finalize instruction
//! let finalize_instruction = sdk.finalize_ix(finalize_params)?;
//!
//! // Step 9: Build and send the finalize transaction
//! let finalize_tx = Transaction::new_signed_with_payer(
//!     &[finalize_instruction],
//!     Some(&user_pubkey),
//!     &[&user_keypair],
//!     recent_blockhash,
//! );
//! let finalize_signature = rpc_client.send_and_confirm_transaction(&finalize_tx)?;
//! ```
//!
//! #### Adding Liquidity with Manual Control
//!
//! ```rust
//! use darklake_sdk::AddLiquidityParams;
//!
//! // Load pool and update accounts (for internal state tracking)
//! sdk.load_pool(token_x, token_y).await?;
//! sdk.update_accounts().await?;
//!
//! // Create add liquidity parameters
//! let add_liquidity_params = AddLiquidityParams {
//!     amount_lp: 1_000,
//!     max_amount_x: 1_000_000,
//!     max_amount_y: 1_000_000,
//!     user: user_pubkey, // Your wallet's public key
//! };
//!
//! // Generate instruction
//! let add_liquidity_instruction = sdk.add_liquidity_ix(add_liquidity_params)?;
//!
//! // Build and send transaction
//! let tx = Transaction::new_signed_with_payer(
//!     &[add_liquidity_instruction],
//!     Some(&user_pubkey),
//!     &[&user_keypair],
//!     recent_blockhash,
//! );
//! let signature = rpc_client.send_and_confirm_transaction(&tx)?;
//! ```
//!
//! #### Removing Liquidity with Manual Control
//!
//! ```rust
//! use darklake_sdk::RemoveLiquidityParams;
//!
//! // Load pool and update accounts (for internal state tracking)
//! sdk.load_pool(token_x, token_y).await?;
//! sdk.update_accounts().await?;
//!
//! // Create remove liquidity parameters
//! let remove_liquidity_params = RemoveLiquidityParams {
//!     amount_lp: 500,
//!     min_amount_x: 500_000,
//!     min_amount_y: 500_000,
//!     user: user_pubkey, // Your wallet's public key
//! };
//!
//! // Generate instruction
//! let remove_liquidity_instruction = sdk.remove_liquidity_ix(remove_liquidity_params)?;
//!
//! // Build and send transaction
//! let tx = Transaction::new_signed_with_payer(
//!     &[remove_liquidity_instruction],
//!     Some(&user_pubkey),
//!     &[&user_keypair],
//!     recent_blockhash,
//! );
//! let signature = rpc_client.send_and_confirm_transaction(&tx)?;
//! ```
//!
//! #### Initializing Pool with Manual Control
//!
//! ```rust
//! use darklake_sdk::InitializePoolParams;
//!
//! // Get token program IDs (required for pool initialization)
//! let token_x_account = rpc_client.get_account(&token_x).await?;
//! let token_y_account = rpc_client.get_account(&token_y).await?;
//!
//! // Create initialize pool parameters
//! let initialize_pool_params = InitializePoolParams {
//!     user: user_pubkey, // Your wallet's public key
//!     token_x,
//!     token_x_program: token_x_account.owner,
//!     token_y,
//!     token_y_program: token_y_account.owner,
//!     amount_x: 1_000_000, // Amount of token X to deposit
//!     amount_y: 1_000_000, // Amount of token Y to deposit
//! };
//!
//! // Generate instruction
//! let initialize_pool_instruction = sdk.initialize_pool_ix(initialize_pool_params)?;
//!
//! // Build and send transaction
//! let tx = Transaction::new_signed_with_payer(
//!     &[initialize_pool_instruction],
//!     Some(&user_pubkey),
//!     &[&user_keypair],
//!     recent_blockhash,
//! );
//! let signature = rpc_client.send_and_confirm_transaction(&tx)?;
//! ```
//!
//! ## 🔧 API Reference
//!
//! ### DarklakeSDK Methods
//!
//! #### Transaction Functions (`_tx`) - Fully Formatted Transactions
//!
//! - **`quote(token_in, token_out, amount_in)`** - Get a quote for a swap
//! - **`swap_tx(token_in, token_out, amount_in, min_amount_out, token_owner)`** - Generate swap transaction, returns `(Transaction, order_key, min_amount_out, salt)`
//! - **`finalize_tx(order_key, unwrap_wsol, min_out, salt, settle_signer)`** - Generate finalize transaction using parameters from swap_tx
//! - **`add_liquidity_tx(token_x, token_y, max_amount_x, max_amount_y, amount_lp, user)`** - Generate add liquidity transaction
//! - **`remove_liquidity_tx(token_x, token_y, min_amount_x, min_amount_y, amount_lp, user)`** - Generate remove liquidity transaction
//! - **`initialize_pool_tx(token_x, token_y, amount_x, amount_y, user)`** - Generate initialize pool transaction
//!
//! #### Instruction Functions (`_ix`) - Core Instructions
//!
//! - **`swap_ix(swap_params)`** - Generate swap instruction
//! - **`finalize_ix(finalize_params)`** - Generate finalize instruction
//! - **`add_liquidity_ix(add_liquidity_params)`** - Generate add liquidity instruction
//! - **`remove_liquidity_ix(remove_liquidity_params)`** - Generate remove liquidity instruction
//! - **`initialize_pool_ix(initialize_pool_params)`** - Generate initialize pool instruction
//!
//! #### Internal State Management
//!
//! - **`load_pool(token_x, token_y)`** - Load pool data for internal state tracking
//! - **`update_accounts()`** - Update internal state with latest chain data
//! - **`get_order(user, commitment_level)`** - Get order data (bypasses internal cache, fetches latest state directly from chain)
//!
//! ### Parameter Types
//!
//! #### SwapParams
//! ```rust
//! pub struct SwapParams {
//!     pub source_mint: Pubkey,
//!     pub destination_mint: Pubkey,
//!     pub token_transfer_authority: Pubkey,
//!     pub in_amount: u64,
//!     pub swap_mode: SwapMode,
//!     pub min_out: u64,
//!     pub salt: [u8; 8],
//! }
//! ```
//!
//! #### FinalizeParams
//! ```rust
//! pub struct FinalizeParams {
//!     pub settle_signer: Pubkey,
//!     pub order_owner: Pubkey,
//!     pub unwrap_wsol: bool, // Set to true if output is WSOL and you want to unwrap it to SOL
//!     pub min_out: u64,
//!     pub salt: [u8; 8],
//!     pub output: u64,
//!     pub commitment: [u8; 32],
//!     pub deadline: u64,
//!     pub current_slot: u64,
//! }
//! ```
//!
//! #### AddLiquidityParams
//! ```rust
//! pub struct AddLiquidityParams {
//!     pub amount_lp: u64,
//!     pub max_amount_x: u64,
//!     pub max_amount_y: u64,
//!     pub user: Pubkey,
//! }
//! ```
//!
//! #### RemoveLiquidityParams
//! ```rust
//! pub struct RemoveLiquidityParams {
//!     pub amount_lp: u64,
//!     pub min_amount_x: u64,
//!     pub min_amount_y: u64,
//!     pub user: Pubkey,
//! }
//! ```
//!
//! #### InitializePoolParams
//! ```rust
//! pub struct InitializePoolParams {
//!     pub user: Pubkey,
//!     pub token_x: Pubkey,
//!     pub token_x_program: Pubkey,
//!     pub token_y: Pubkey,
//!     pub token_y_program: Pubkey,
//!     pub amount_x: u64,
//!     pub amount_y: u64,
//! }
//! ```
//!
//! ## 🌐 Network Configuration
//!
//! SDK needs an rpc url which is used for on chain data fetching.
//!
//! ## 📈 Performance Considerations
//!
//! - **Proof generation**: currently non-async and blocking
//!
//! ## 📄 License
//!
//! MIT License - see LICENSE file for details.
//!
//! ## 🆘 Support
//!
//! For issues and questions:
//! - Check the examples in the repository
//! - Review the SDK source code
//! - Open an issue on the repository
//!
//! ---
//!
//! **Note**: This SDK is for interacting with the Darklake DEX on Solana. Always test thoroughly on devnet before using on mainnet.

mod account_metas;
mod amm; // Private module - users should use re-exported types
mod constants;
mod darklake_amm;
mod proof;
mod reduced_amm_params;
mod sdk;
mod utils;

pub use sdk::DarklakeSDK;

// Re-export commonly used AMM types for easier access
pub use reduced_amm_params::{
    AddLiquidityParamsIx, FinalizeParamsIx, InitializePoolParamsIx, RemoveLiquidityParamsIx,
    SwapParamsIx,
};

pub use amm::SwapMode;

pub use constants::{DEVNET_LOOKUP, MAINNET_LOOKUP};
