use anyhow::Result;
use solana_sdk::{clock::Clock, sysvar::Sysvar};

/// Calculate the fee for input amount
pub fn get_transfer_fee(transfer_fee_config: &Option<spl_token_2022::extension::transfer_fee::TransferFeeConfig>, pre_fee_amount: u64) -> Result<u64> {
    if transfer_fee_config.is_none() {
        return Ok(0);
    }

    let transfer_fee_config = transfer_fee_config.unwrap();

    let fee = transfer_fee_config
        .calculate_epoch_fee(Clock::get()?.epoch, pre_fee_amount)
        .unwrap();
    Ok(fee)
}

pub const MAX_PERCENTAGE: u64 = 1_000_000; // 100% in basis points

fn ceil_div(token_amount: u128, fee_numerator: u128, fee_denominator: u128) -> Option<u128> {
    token_amount
        .checked_mul(u128::from(fee_numerator))
        .unwrap()
        .checked_add(fee_denominator)?
        .checked_sub(1)?
        .checked_div(fee_denominator)
}

pub fn floor_div(token_amount: u128, fee_numerator: u128, fee_denominator: u128) -> Option<u128> {
    Some(
        token_amount
            .checked_mul(fee_numerator)?
            .checked_div(fee_denominator)?,
    )
}

pub fn get_trade_fee(amount: u128, trade_fee_rate: u64) -> Option<u128> {
    ceil_div(
        amount,
        u128::from(trade_fee_rate),
        u128::from(MAX_PERCENTAGE),
    )
}

pub fn get_protocol_fee(amount: u128, protocol_fee_rate: u64) -> Option<u128> {
    floor_div(
        amount,
        u128::from(protocol_fee_rate),
        u128::from(MAX_PERCENTAGE),
    )
}

pub fn swap_base_input_without_fees(
    source_amount: u128,
    swap_source_amount: u128,
    swap_destination_amount: u128,
) -> u128 {
    // (x + delta_x) * (y - delta_y) = x * y
    // delta_y = (delta_x * y) / (x + delta_x)
    let numerator = source_amount.checked_mul(swap_destination_amount).unwrap();
    let denominator = swap_source_amount.checked_add(source_amount).unwrap();
    let destination_amount_swapped = numerator.checked_div(denominator).unwrap();
    destination_amount_swapped
}

pub fn swap(
    source_amount: u128,
    pool_source_amount: u128,
    pool_destination_amount: u128,
    trade_fee_rate: u64,
    protocol_fee_rate: u64,
) -> Option<SwapResult> {
    let trade_fee = get_trade_fee(source_amount, trade_fee_rate).unwrap();
    let protocol_fee = get_protocol_fee(trade_fee, protocol_fee_rate).unwrap();

    let source_amount_post_fees = source_amount.checked_sub(trade_fee).unwrap();

    let destination_amount_swapped = swap_base_input_without_fees(
        source_amount_post_fees,
        pool_source_amount,
        pool_destination_amount,
    );

    Some(SwapResult {
        from_amount: source_amount_post_fees as u64,
        to_amount: destination_amount_swapped as u64,
        trade_fee: trade_fee as u64,
        protocol_fee: protocol_fee as u64,
    })
}

pub struct SwapResult {
    pub from_amount: u64,
    pub to_amount: u64,
    pub trade_fee: u64,
    pub protocol_fee: u64,
}

pub struct SwapResultWithFromToLock {
    pub from_amount: u64,
    pub to_amount: u64,
    pub trade_fee: u64,
    pub protocol_fee: u64,
    pub from_to_lock: u64,
}

pub struct RebalanceResult {
    pub from_to_lock: u64,
    pub is_rate_tolerance_exceeded: bool,
}

pub fn rebalance_pool_ratio(
    to_amount_swapped: u64,
    current_source_amount: u64,
    current_destination_amount: u64,
    original_source_amount: u64,
    original_destination_amount: u64,
    ratio_change_tolerance_rate: u64,
) -> Option<RebalanceResult> {
    if to_amount_swapped >= current_destination_amount
        || current_source_amount == 0
        || current_destination_amount == 0
    {
        // Should never happen, but just in case
        return Some(RebalanceResult {
            from_to_lock: 0,
            is_rate_tolerance_exceeded: true,
        });
    }

    // Calculate the remaining destination amount after swap
    let remaining_destination = current_destination_amount.checked_sub(to_amount_swapped)?;

    let original_ratio = original_source_amount as f64 / original_destination_amount as f64;

    // Calculate the exact floating-point value that would give us the perfect ratio
    let exact_from_to_lock =
        current_source_amount as f64 - (remaining_destination as f64 * original_ratio);

    // Find the optimal integer from_to_lock by testing values around the exact value
    let mut best_from_to_lock = 0u64;
    let mut best_ratio_diff = f64::INFINITY;

    // Test a range of values around the exact value
    let start_val = (exact_from_to_lock - 1.0).max(0.0) as u64;
    let end_val = (exact_from_to_lock + 1.0).min(current_source_amount as f64) as u64;

    for test_from_to_lock in start_val..=end_val {
        if test_from_to_lock > current_source_amount {
            continue;
        }

        let new_source = current_source_amount.checked_sub(test_from_to_lock)?;
        let new_ratio = new_source as f64 / remaining_destination as f64;
        let ratio_diff = (new_ratio - original_ratio).abs();

        if ratio_diff < best_ratio_diff && new_ratio != 0.0 {
            best_ratio_diff = ratio_diff;
            best_from_to_lock = test_from_to_lock;
        }
    }

    let from_to_lock = best_from_to_lock;
    let new_source_amount = current_source_amount.checked_sub(from_to_lock)?;
    let new_ratio = new_source_amount as f64 / remaining_destination as f64;

    // Calculate percentage change
    let percentage_change = (new_ratio - original_ratio).abs() / original_ratio * 100.0;

    let tolerance_percentage = (ratio_change_tolerance_rate as f64 / MAX_PERCENTAGE as f64) * 100.0;
    let is_rate_tolerance_exceeded = percentage_change > tolerance_percentage;

    Some(RebalanceResult {
        from_to_lock,
        is_rate_tolerance_exceeded,
    })
}
