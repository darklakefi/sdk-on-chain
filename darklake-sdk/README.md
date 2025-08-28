# Darklake DEX SDK

A standalone SDK for interacting with Darklake AMM pools on Solana. This SDK provides the core functionality for getting quotes, building swap instructions, and managing pool state without the complexity of Jupiter's routing and aggregation features.

## Features

- **Lightweight**: Focused only on Darklake-specific functionality
- **Core AMM Operations**: Get quotes, build swap instructions, manage pool state
- **Token Support**: Full support for both SPL Token and Token-2022 tokens
- **Transfer Fee Handling**: Automatic handling of transfer fees for Token-2022 tokens
- **Pool Rebalancing**: Built-in pool ratio rebalancing logic
- **No External Dependencies**: Self-contained without Jupiter-specific features

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
darklake-sdk = { path = "path/to/darklake-sdk" }
```

## Quick Start

```rust
use darklake_sdk::{DarklakeAmm, create_darklake_amm, QuoteParams, SwapMode};
use solana_sdk::pubkey::Pubkey;

// Create a Darklake AMM instance
let amm_key = Pubkey::from_str("your_amm_pool_address").unwrap();
let account_data = /* get account data from RPC */;
let mut amm = create_darklake_amm(amm_key, &account_data).unwrap();

// Get a quote for a swap
let quote_params = QuoteParams {
    input_mint: token_x_mint,
    amount: 1000000, // 1 token (assuming 6 decimals)
    swap_mode: SwapMode::ExactIn,
};

let quote = amm.quote(&quote_params).unwrap();
println!("Input: {}, Output: {}, Fee: {}", 
    quote.in_amount, quote.out_amount, quote.fee_amount);
```

## Core Types

### DarklakeAmm

The main AMM implementation that provides:

- Pool state management
- Quote calculations
- Swap instruction building
- Account metadata generation

### AMM Trait

A simplified trait that defines the core AMM interface:

```rust
pub trait Amm: Send + Sync {
    fn label(&self) -> String;
    fn program_id(&self) -> Pubkey;
    fn quote(&self, params: &QuoteParams) -> Result<Quote>;
    fn get_swap_and_account_metas(&self, params: &SwapParams) -> Result<SwapAndAccountMetas>;
    // ... and more
}
```

## Usage Examples

### Getting Pool Information

```rust
let reserve_mints = amm.get_reserve_mints();
println!("Pool tokens: {:?}", reserve_mints);

let accounts_to_update = amm.get_accounts_to_update();
println!("Accounts to update: {:?}", accounts_to_update);
```

### Building Swap Instructions

```rust
let swap_params = SwapParams {
    source_mint: token_x_mint,
    destination_mint: token_y_mint,
    source_token_account: user_token_x_account,
    destination_token_account: user_token_y_account,
    token_transfer_authority: user_authority,
    in_amount: 1000000,
    minimum_out_amount: 950000,
    swap_mode: SwapMode::ExactIn,
};

let swap_and_accounts = amm.get_swap_and_account_metas(&swap_params).unwrap();
println!("Account metas: {:?}", swap_and_accounts.account_metas);
```

### Updating Pool State

```rust
// Create an account map with current pool data
let mut account_map = AccountMap::new();
// ... populate with account data from RPC

// Update the AMM state
amm.update(&account_map).unwrap();
```

## Architecture

The SDK is organized into three main modules:

1. **`amm`**: Core AMM trait and shared types
2. **`math`**: Mathematical functions for swaps, fees, and pool rebalancing
3. **`darklake_amm`**: Darklake-specific AMM implementation

## Testing

Run the test suite:

```bash
cargo test
```

## Differences from Jupiter

This SDK is a simplified version that:

- ✅ Keeps the core AMM functionality
- ✅ Maintains the same structure for Darklake AMM
- ✅ Provides all necessary mathematical functions
- ❌ Removes Jupiter routing and aggregation
- ❌ Removes swap chaining (not supported by Darklake)
- ❌ Removes complex test harness dependencies

## License

MIT License - see LICENSE file for details.

## Contributing

Contributions are welcome! Please ensure all tests pass and follow the existing code style.
