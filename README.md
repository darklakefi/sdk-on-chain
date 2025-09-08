# Darklake DEX SDK

A standalone SDK for interacting with Darklake AMM pools on Solana. This SDK provides two ways to interact with the Darklake DEX:

1. **Default Mode**: High-level methods that handle everything automatically
2. **Manual Mode**: Lower-level methods that give you full control over transaction building

## 🚀 Quick Start

### Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
darklake-sdk = "0.1.3"
```

### Basic Setup

```rust
use darklake_sdk::DarklakeSDK;
use solana_sdk::signer::keypair::Keypair;

// Initialize the SDK
let payer = Keypair::new(); // Your wallet keypair
let mut sdk = DarklakeSDK::new("https://api.devnet.solana.com", payer);
```

## 📖 Usage Patterns

### 1. Default Mode (Recommended for most users)

The default mode provides high-level methods that handle all the complexity for you:

#### Trading (Swap)

```rust
use solana_sdk::pubkey::Pubkey;

// Define token mints
let token_in = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(); // SOL
let token_out = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(); // USDC

// Get a quote first
let quote = sdk.quote(token_in, token_out, 1_000_000).await?; // 1 SOL
println!("Expected output: {}", quote.out_amount);

// Execute the swap
let (swap_signature, finalize_signature) = sdk.swap(
    token_in,
    token_out,
    1_000_000,  // 1 SOL (in lamports)
    950_000,    // Minimum 0.95 USDC out (5% slippage)
).await?;

println!("Swap signature: {}", swap_signature);
println!("Finalize signature: {}", finalize_signature);
```

#### Adding Liquidity

```rust
let token_x = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
let token_y = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();

let signature = sdk.add_liquidity(
    token_x,
    token_y,
    1_000_000,  // Max amount of token X
    1_000_000,  // Max amount of token Y
    1_000,      // LP token amount to mint
).await?;

println!("Add liquidity signature: {}", signature);
```

#### Removing Liquidity

```rust
let signature = sdk.remove_liquidity(
    token_x,
    token_y,
    500_000,    // Min amount of token X to receive
    500_000,    // Min amount of token Y to receive
    500,        // LP token amount to burn
).await?;

println!("Remove liquidity signature: {}", signature);
```

### 2. Manual Mode (For advanced users)

The manual mode gives you full control over transaction building and execution:

#### Trading with Manual Control

```rust
use darklake_sdk::{SwapParams, FinalizeParams, SwapMode};
use solana_sdk::instruction::Instruction;

// Step 1: Load the pool
sdk.load_pool(token_in, token_out).await?;

// Step 2: Update accounts with latest data
sdk.update_accounts().await?;

// Step 3: Create swap parameters
let swap_params = SwapParams {
    source_mint: token_in,
    destination_mint: token_out,
    token_transfer_authority: sdk.signer_pubkey(),
    in_amount: 1_000_000,
    swap_mode: SwapMode::ExactIn,
    min_out: 950_000,
    salt: [1, 2, 3, 4, 5, 6, 7, 8], // Unique salt for this order
};

// Step 4: Generate swap instruction
let swap_instruction = sdk.swap_ix(swap_params).await?;

// Step 5: Send the swap transaction
let swap_signature = rpc_client.send_and_confirm_transaction(&Transaction::new_signed_with_payer(
    &[swap_instruction],
    Some(&sdk.signer_pubkey()),
    &[&keypair],
    recent_blockhash,
))?;

// Step 6: Wait for order to be created and get order data
let order = sdk.get_order(sdk.signer_pubkey()).await?;

// Step 7: Create finalize parameters
let finalize_params = FinalizeParams {
    settle_signer: sdk.signer_pubkey(),
    order_owner: sdk.signer_pubkey(),
    unwrap_wsol: false, // Set to true if output is wrapped SOL
    min_out: 950_000,
    salt: [1, 2, 3, 4, 5, 6, 7, 8],
    output: order.d_out,
    commitment: order.c_min,
    deadline: order.deadline,
    current_slot: rpc_client.get_slot()?,
};

// Step 8: Generate finalize instruction
let finalize_instruction = sdk.finalize_ix(finalize_params).await?;

// Step 9: Send the finalize transaction
let finalize_signature = rpc_client.send_and_confirm_transaction(&Transaction::new_signed_with_payer(
    &[finalize_instruction],
    Some(&sdk.signer_pubkey()),
    &[&keypair],
    recent_blockhash,
))?;
```

#### Adding Liquidity with Manual Control

```rust
use darklake_sdk::AddLiquidityParams;

// Load pool and update accounts
sdk.load_pool(token_x, token_y).await?;
sdk.update_accounts().await?;

// Create add liquidity parameters
let add_liquidity_params = AddLiquidityParams {
    amount_lp: 1_000,
    max_amount_x: 1_000_000,
    max_amount_y: 1_000_000,
    user: sdk.signer_pubkey(),
};

// Generate instruction
let add_liquidity_instruction = sdk.add_liquidity_ix(add_liquidity_params).await?;

// Send transaction
let signature = rpc_client.send_and_confirm_transaction(&Transaction::new_signed_with_payer(
    &[add_liquidity_instruction],
    Some(&sdk.signer_pubkey()),
    &[&keypair],
    recent_blockhash,
))?;
```

#### Removing Liquidity with Manual Control

```rust
use darklake_sdk::RemoveLiquidityParams;

// Load pool and update accounts
sdk.load_pool(token_x, token_y).await?;
sdk.update_accounts().await?;

// Create remove liquidity parameters
let remove_liquidity_params = RemoveLiquidityParams {
    amount_lp: 500,
    min_amount_x: 500_000,
    min_amount_y: 500_000,
    user: sdk.signer_pubkey(),
};

// Generate instruction
let remove_liquidity_instruction = sdk.remove_liquidity_ix(remove_liquidity_params).await?;

// Send transaction
let signature = rpc_client.send_and_confirm_transaction(&Transaction::new_signed_with_payer(
    &[remove_liquidity_instruction],
    Some(&sdk.signer_pubkey()),
    &[&keypair],
    recent_blockhash,
))?;
```

## 🔧 API Reference

### DarklakeSDK Methods

#### High-Level Methods (Default Mode)

- **`quote(token_in, token_out, amount_in)`** - Get a quote for a swap
- **`swap(token_in, token_out, amount_in, min_amount_out)`** - Execute a complete swap
- **`add_liquidity(token_x, token_y, max_amount_x, max_amount_y, amount_lp)`** - Add liquidity to a pool
- **`remove_liquidity(token_x, token_y, min_amount_x, min_amount_y, amount_lp)`** - Remove liquidity from a pool

#### Low-Level Methods (Manual Mode)

- **`load_pool(token_x, token_y)`** - Load pool data (required before manual operations)
- **`update_accounts()`** - Update pool accounts with latest data
- **`swap_ix(swap_params)`** - Generate swap instruction
- **`finalize_ix(finalize_params)`** - Generate finalize instruction
- **`add_liquidity_ix(add_liquidity_params)`** - Generate add liquidity instruction
- **`remove_liquidity_ix(remove_liquidity_params)`** - Generate remove liquidity instruction
- **`get_order(user)`** - Get order data for a user
- **`signer_pubkey()`** - Get the signer's public key

### Parameter Types

#### SwapParams
```rust
pub struct SwapParams {
    pub source_mint: Pubkey,
    pub destination_mint: Pubkey,
    pub token_transfer_authority: Pubkey,
    pub in_amount: u64,
    pub swap_mode: SwapMode,
    pub min_out: u64,
    pub salt: [u8; 8],
}
```

#### FinalizeParams
```rust
pub struct FinalizeParams {
    pub settle_signer: Pubkey,
    pub order_owner: Pubkey,
    pub unwrap_wsol: bool,
    pub min_out: u64,
    pub salt: [u8; 8],
    pub output: u64,
    pub commitment: [u8; 32],
    pub deadline: u64,
    pub current_slot: u64,
}
```

#### AddLiquidityParams
```rust
pub struct AddLiquidityParams {
    pub amount_lp: u64,
    pub max_amount_x: u64,
    pub max_amount_y: u64,
    pub user: Pubkey,
}
```

#### RemoveLiquidityParams
```rust
pub struct RemoveLiquidityParams {
    pub amount_lp: u64,
    pub min_amount_x: u64,
    pub min_amount_y: u64,
    pub user: Pubkey,
}
```

## 🏗️ Architecture

The Darklake DEX SDK is built on top of the Darklake AMM program and provides:

- **Zero-Knowledge Proof Integration**: Orders are settled using ZK proofs for privacy
- **Automatic Order Management**: The SDK handles order creation, settlement, and cancellation
- **Pool State Management**: Automatic fetching and updating of pool data
- **Transaction Building**: Simplified transaction construction with proper account management

## 🔑 Prerequisites

### Wallet Setup

You need a Solana wallet with sufficient SOL for:
- Transaction fees (approximately 0.000005 SOL per transaction)
- Token swaps and liquidity operations
- Account rent for token accounts

### Required Dependencies

```toml
[dependencies]
darklake-sdk = "0.1.3"
solana-sdk = "1.17"
anchor-client = "0.31.1"
tokio = { version = "1.0", features = ["full"] }
anyhow = "1.0"
```

## 🌐 Network Configuration

SDK needs an rpc url which is used for on chain data fetching and tx execution.

## 🚨 Error Handling

The SDK uses `anyhow::Result` for comprehensive error handling:
- RPC connection failures
- Account data parsing errors
- Transaction signing failures
- Invalid parameters
- Order state errors

## 📈 Performance Considerations

- **Account Updates**: Only fetch accounts that need updating
- **Async Operations**: Uses Tokio for non-blocking I/O
- **Transaction Batching**: Multiple operations can be batched in single transactions
- **Pool Caching**: Pool data is cached to avoid unnecessary RPC calls

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests for new functionality
5. Ensure all tests pass
6. Submit a pull request

## 📄 License

MIT License - see LICENSE file for details.

## 🆘 Support

For issues and questions:
- Check the examples in the repository
- Review the SDK source code
- Open an issue on the repository

---

**Note**: This SDK is for interacting with the Darklake DEX on Solana. Always test thoroughly on devnet before using on mainnet.