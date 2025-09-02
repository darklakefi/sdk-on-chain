# Darklake AMM Implementation

A complete implementation of the Darklake Automated Market Maker (AMM) system on Solana, featuring zero-knowledge proof-based order settlement, cancellation, and slashing mechanisms with an intelligent finalization helper.

## 🏗️ Architecture Overview

Darklake AMM is a sophisticated decentralized exchange that uses zero-knowledge proofs to ensure order privacy and security. The system operates in three main phases:

1. **Swap Phase**: Users submit orders with encrypted parameters
2. **Settlement Phase**: Orders are settled using ZK proofs when conditions are met
3. **Management Phase**: Orders can be cancelled or slashed based on various conditions

**🆕 New Feature**: The system now includes an intelligent `finalize` helper that automatically determines the appropriate action (settle, cancel, or slash) based on order conditions, eliminating the need for manual decision logic.

## 🔑 Prerequisites

### Wallet Setup

You need a Solana wallet with a `key.json` file in the project root. The file should contain a 64-byte private key array:

```json
[123, 45, 67, 89, ...] // 64 bytes total
```

**⚠️ Security Note**: Never commit your `key.json` file to version control. Add it to `.gitignore`.

### Required SOL Balance

Your wallet needs sufficient SOL for:
- Transaction fees (approximately 0.000005 SOL per transaction)
- Any token swaps you want to perform
- Pool liquidity if you're a liquidity provider

## 🚀 Complete Flow Example

The `examples/src/main.rs` demonstrates the entire Darklake AMM lifecycle:

### 1. Pool Initialization

```rust
// Get pool key from token mints
let pool_key = get_pool_key(token_mint_x, token_mint_y);

// Fetch pool data from Solana RPC
let pool_account = rpc_client.get_account(&pool_key)?;

// Initialize AMM structure
let mut darklake_amm = create_darklake_amm(pool_key, &pool_account.data)?;
```

### 2. Pool State Update

```rust
// Get accounts that need updating
let accounts_to_update = darklake_amm.get_accounts_to_update();

// Fetch latest account data
let mut account_map = HashMap::new();
for account_key in &accounts_to_update {
    if let Ok(account) = rpc_client.get_account(account_key) {
        account_map.insert(*account_key, AccountData {
            data: account.data,
            owner: account.owner,
        });
    }
}

// Update AMM with latest data
darklake_amm.update(&account_map)?;
```

### 3. Swap Order Creation

```rust
let swap_params = SwapParams {
    source_mint: token_mint_x,
    destination_mint: token_mint_y,
    source_token_account: source_account,
    destination_token_account: dest_account,
    token_transfer_authority: user_keypair.pubkey(),
    in_amount: 1_000, // Amount to swap in
    swap_mode: SwapMode::ExactIn,
    min_out: 950,     // Minimum output (5% slippage tolerance)
    salt: [1,2,3,4,5,6,7,8], // Unique order identifier
};

// Get swap instruction and account metadata
let swap_and_account_metas = darklake_amm.get_swap_and_account_metas(&swap_params)?;

// Build and send transaction
let swap_transaction = Transaction::new_signed_with_payer(
    &[swap_instruction],
    Some(&user_keypair.pubkey()),
    &[&user_keypair],
    recent_blockhash,
);

let swap_signature = rpc_client.send_and_confirm_transaction(&swap_transaction)?;
```

### 4. Order Lifecycle Management

**🆕 Simplified Approach**: Use the new `finalize` helper that automatically determines the appropriate action:

```rust
// The finalize helper automatically determines whether to settle, cancel, or slash
let finalize_params = FinalizeParams {
    settle_signer: user_keypair.pubkey(),
    order_owner: user_keypair.pubkey(),
    unwrap_wsol: true,
    min_out: swap_params.min_out,
    salt: swap_params.salt,
    output: order_output,
    commitment: swap_commitment,
    deadline: order_deadline,
    current_slot: rpc_client.get_slot()?,
};

let finalize_result = darklake_amm.get_finalize_and_account_metas(&finalize_params)?;

// The data field is now pre-serialized - no manual construction needed!
let transaction_data = finalize_result.data();
let account_metas = finalize_result.account_metas();

// Build and send the finalization transaction
let finalize_instruction = Instruction {
    program_id: darklake_amm.program_id(),
    accounts: account_metas,
    data: transaction_data,
};

let finalize_transaction = Transaction::new_signed_with_payer(
    &[finalize_instruction],
    Some(&user_keypair.pubkey()),
    &[&user_keypair],
    recent_blockhash,
);

let finalize_signature = rpc_client.send_and_confirm_transaction(&finalize_transaction)?;
```

**🔄 Legacy Manual Approach**: For users who prefer explicit control, the individual methods are still available:

#### Path A: Order Expiration (Slash)
```rust
// Wait for order to expire (for testing purposes)
println!("Waiting for order to be outdated...");
let mut is_outdated = false;
let mut attempt_count = 0;

while !is_outdated {
    attempt_count += 1;
    is_outdated = darklake_amm.is_order_expired(&order_data.data, rpc_client.get_slot()?)?;
    
    if is_outdated {
        println!("✅ Order is now outdated (attempt {})", attempt_count);
        break;
    }
    
    println!("   Attempt {}: Order not yet outdated, waiting 1 second...", attempt_count);
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
}

// Execute slash transaction
if is_outdated {
    let slash_and_account_metas = darklake_amm.get_slash_and_account_metas(&slash_params)?;
    
    // 🆕 Data is now pre-serialized - use it directly!
    let slash_transaction = Transaction::new_signed_with_payer(
        &[Instruction {
            program_id: darklake_amm.program_id(),
            accounts: slash_and_account_metas.account_metas,
            data: slash_and_account_metas.data, // Pre-serialized data
        }],
        Some(&user_keypair.pubkey()),
        &[&user_keypair],
        recent_blockhash,
    );
    
    let slash_signature = rpc_client.send_and_confirm_transaction(&slash_transaction)?;
}
```

#### Path B: Order Cancellation
```rust
let is_cancel = order_output < swap_params.min_out;

if is_cancel {
    println!("Cancelling order -------|");
    let cancel_and_account_metas = darklake_amm.get_cancel_and_account_metas(&cancel_params)?;
    
    // 🆕 Data is now pre-serialized - no manual construction needed!
    let cancel_transaction = Transaction::new_signed_with_payer(
        &[Instruction {
            program_id: darklake_amm.program_id(),
            accounts: cancel_and_account_metas.account_metas,
            data: cancel_and_account_metas.data, // Pre-serialized data
        }],
        Some(&user_keypair.pubkey()),
        &[&user_keypair],
        recent_blockhash,
    );
    
    let cancel_signature = rpc_client.send_and_confirm_transaction(&cancel_transaction)?;
}
```

#### Path C: Order Settlement
```rust
println!("Settling order ------->");

// Get settle instruction with ZK proofs
let settle_and_account_metas = darklake_amm.get_settle_and_account_metas(&settle_params)?;

// 🆕 Data is now pre-serialized - no manual construction needed!
let settle_transaction = Transaction::new_signed_with_payer(
    &[Instruction {
        program_id: darklake_amm.program_id(),
        accounts: settle_and_account_metas.account_metas,
        data: settle_and_account_metas.data, // Pre-serialized data
    }],
    Some(&user_keypair.pubkey()),
    &[&user_keypair],
    recent_blockhash,
);

let settle_signature = rpc_client.send_and_confirm_transaction(&settle_transaction)?;
```

## 🔧 Key Parameters Explained

### Swap Parameters
- **`source_mint`**: Token mint address you're swapping from
- **`destination_mint`**: Token mint address you're swapping to
- **`in_amount`**: Amount of source tokens to swap
- **`min_out`**: Minimum amount of destination tokens to receive (slippage protection)
- **`salt`**: Unique 8-byte identifier for the order

### Finalize Parameters (🆕)
- **`settle_signer`**: Public key of the account signing the finalization transaction
- **`order_owner`**: Public key of the order owner
- **`unwrap_wsol`**: Whether to unwrap wrapped SOL after settlement (only used for settle operations)
- **`min_out`**: Same as swap min_out (ensures consistency)
- **`output`**: Expected output amount (calculated from order data)
- **`commitment`**: Cryptographic commitment from the swap
- **`deadline`**: Order expiration timestamp
- **`current_slot`**: Current Solana slot number

### Settle Parameters
- **`settle_signer`**: Public key of the account signing the settle transaction
- **`order_owner`**: Public key of the order owner
- **`unwrap_wsol`**: Whether to unwrap wrapped SOL after settlement
- **`min_out`**: Same as swap min_out (ensures consistency)
- **`output`**: Expected output amount (calculated from order data)
- **`commitment`**: Cryptographic commitment from the swap
- **`deadline`**: Order expiration timestamp

### Cancel Parameters
- **`settle_signer`**: Public key of the account signing the cancel transaction
- **`order_owner`**: Public key of the order owner
- **`min_out`**: Same as swap min_out
- **`output`**: Expected output amount
- **`commitment`**: Cryptographic commitment from the swap
- **`deadline`**: Order expiration timestamp

### Slash Parameters
- **`settle_signer`**: Public key of the account signing the slash transaction
- **`order_owner`**: Public key of the order owner
- **`deadline`**: Order expiration timestamp
- **`current_slot`**: Current Solana slot number

## 🧮 Zero-Knowledge Proofs

The system uses Groth16 ZK proofs for:

1. **Settlement Proofs**: Prove that an order meets settlement conditions without revealing private order details
2. **Cancellation Proofs**: Prove that an order should be cancelled (e.g., slippage exceeded)
3. **Slashing Proofs**: Prove that an order has expired and should be slashed

### Proof Structure
- **Proof A**: 64 bytes
- **Proof B**: 128 bytes  
- **Proof C**: 64 bytes
- **Public Signals**: 2 × 32 bytes

## 🚦 Decision Flow Logic

**🆕 Enhanced with Finalize Helper**:

```
Swap Order Created
        ↓
    Check Order Status
        ↓
┌─────────────────┬─────────────────┬─────────────────┐
│   Order Expired │ Slippage Exceed │  Ready to Settle│
│        ↓        │        ↓        │        ↓        │
│   SLASH ORDER   │  CANCEL ORDER   │  SETTLE ORDER   │
│                 │                 │                 │
│ - No ZK proof   │ - ZK proof      │ - ZK proof      │
│ - Simple data   │ - Complex data  │ - Complex data  │
│ - Immediate     │ - Immediate     │ - Immediate     │
└─────────────────┴─────────────────┴─────────────────┘

🆕 NEW: Use finalize() helper to automatically choose the right path!
```

## 🆕 New Features

### 1. Finalize Helper
The `get_finalize_and_account_metas()` method automatically determines whether an order should be:
- **Settled**: When `min_out <= output` and order hasn't expired
- **Cancelled**: When `min_out > output` and order hasn't expired  
- **Slashed**: When order has expired (`current_slot > deadline`)

### 2. Pre-serialized Data
All transaction data is now pre-serialized in the `data` field, eliminating the need for manual byte array construction:
- **Before**: Users had to manually concatenate discriminator, proofs, and public signals
- **After**: Simply use `finalize_result.data()` directly in the transaction

### 3. Simplified API
```rust
// Old way (still supported)
let settle_result = darklake_amm.get_settle_and_account_metas(&settle_params)?;
let mut data = settle_result.discriminator.to_vec();
data.extend_from_slice(&settle_result.settle.proof_a);
data.extend_from_slice(&settle_result.settle.proof_b);
data.extend_from_slice(&settle_result.settle.proof_c);
// ... more manual construction

// New way (recommended)
let finalize_result = darklake_amm.get_finalize_and_account_metas(&finalize_params)?;
let data = finalize_result.data(); // Ready to use!
```

## 🛠️ Development Setup

### 1. Install Dependencies
```bash
# Install Rust and Cargo
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Solana CLI
sh -c "$(curl -sSfL https://release.solana.com/stable/install)"
```

### 2. Clone and Build
```bash
git clone <repository-url>
cd jupiter-amm-implementation
cargo build
```

### 3. Configure Wallet
```bash
# Create or copy your key.json file to the project root
# Ensure it contains a 64-byte private key array
```

### 4. Run Examples
```bash
# Run the complete example
cargo run -p darklake-examples

# Run tests
cargo test
```

## 🔍 Testing Scenarios

The example demonstrates three main scenarios:

1. **Expired Order Slashing**: Waits for order expiration and executes slash
2. **Slippage Cancellation**: Cancels orders that exceed slippage tolerance
3. **Successful Settlement**: Settles orders that meet all conditions

**🆕 New**: All scenarios can now be handled with a single `finalize` call!

## 🌐 Network Configuration

- **Default**: Solana Devnet (`https://api.devnet.solana.com`)
- **Custom**: Modify `DEVNET_ENDPOINT` constant in `examples/src/main.rs`
- **Mainnet**: Change endpoint to mainnet-beta RPC URL

## 📊 Pool Information

The SDK provides access to:
- Pool label and program ID
- Reserve token mints
- Active status
- Exact out swap support
- Account update requirements

## 🔐 Security Features

- **Private Order Details**: Orders are encrypted and only revealed through ZK proofs
- **Slippage Protection**: Automatic cancellation if output falls below minimum
- **Expiration Handling**: Automatic slashing of expired orders
- **Commitment Verification**: Cryptographic verification of order parameters
- **🆕 Intelligent Finalization**: Automatic action selection based on order state

## 🚨 Error Handling

The SDK uses `anyhow` for comprehensive error handling:
- RPC connection failures
- Account data parsing errors
- Transaction signing failures
- ZK proof generation errors
- **🆕 Finalization decision errors**

## 📈 Performance Considerations

- **Account Updates**: Only fetch accounts that need updating
- **Proof Generation**: ZK proofs are generated on-demand
- **Transaction Batching**: Multiple operations can be batched in single transactions
- **Async Operations**: Uses Tokio for non-blocking I/O
- **🆕 Pre-serialized Data**: Eliminates runtime serialization overhead

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
- Check the example code in `examples/src/main.rs`
- Review the SDK source in `darklake-sdk/src/`
- Open an issue on the repository

---

**Note**: This implementation is for educational and development purposes. Always test thoroughly on devnet before using on mainnet.

**🆕 Latest Updates**: The SDK now includes an intelligent finalization helper and pre-serialized transaction data for simplified order lifecycle management.
