use anchor_lang::prelude::AccountMeta;
use solana_sdk::pubkey::Pubkey;

pub(crate) struct DarklakeAmmSwap {
    pub user: Pubkey,
    pub token_mint_x: Pubkey,
    pub token_mint_y: Pubkey,
    pub token_mint_wsol: Pubkey,
    pub pool: Pubkey,
    pub authority: Pubkey,
    pub amm_config: Pubkey,
    pub user_token_account_x: Pubkey,
    pub user_token_account_y: Pubkey,
    pub user_token_account_wsol: Pubkey,
    pub pool_token_reserve_x: Pubkey,
    pub pool_token_reserve_y: Pubkey,
    pub pool_wsol_reserve: Pubkey,
    pub order: Pubkey,
    pub associated_token_program: Pubkey,
    pub system_program: Pubkey,
    pub token_mint_x_program: Pubkey,
    pub token_mint_y_program: Pubkey,
    pub token_program: Pubkey,
}

impl From<DarklakeAmmSwap> for Vec<AccountMeta> {
    fn from(accounts: DarklakeAmmSwap) -> Self {
        vec![
            AccountMeta::new(accounts.user, true),
            AccountMeta::new_readonly(accounts.token_mint_x, false),
            AccountMeta::new_readonly(accounts.token_mint_y, false),
            AccountMeta::new_readonly(accounts.token_mint_wsol, false),
            AccountMeta::new(accounts.pool, false),
            AccountMeta::new_readonly(accounts.authority, false),
            AccountMeta::new_readonly(accounts.amm_config, false),
            AccountMeta::new(accounts.user_token_account_x, false),
            AccountMeta::new(accounts.user_token_account_y, false),
            AccountMeta::new(accounts.user_token_account_wsol, false),
            AccountMeta::new(accounts.pool_token_reserve_x, false),
            AccountMeta::new(accounts.pool_token_reserve_y, false),
            AccountMeta::new(accounts.pool_wsol_reserve, false),
            AccountMeta::new(accounts.order, false),
            AccountMeta::new_readonly(accounts.associated_token_program, false),
            AccountMeta::new_readonly(accounts.system_program, false),
            AccountMeta::new_readonly(accounts.token_mint_x_program, false),
            AccountMeta::new_readonly(accounts.token_mint_y_program, false),
            AccountMeta::new_readonly(accounts.token_program, false),
        ]
    }
}

pub(crate) struct DarklakeAmmSettle {
    pub caller: Pubkey,
    pub order_owner: Pubkey,
    pub token_mint_x: Pubkey,
    pub token_mint_y: Pubkey,
    pub token_mint_wsol: Pubkey,
    pub pool: Pubkey,
    pub authority: Pubkey,
    pub pool_token_reserve_x: Pubkey,
    pub pool_token_reserve_y: Pubkey,
    pub pool_wsol_reserve: Pubkey,
    pub amm_config: Pubkey,
    pub user_token_account_x: Pubkey,
    pub user_token_account_y: Pubkey,
    pub user_token_account_wsol: Pubkey,
    pub caller_token_account_wsol: Pubkey,
    pub order: Pubkey,
    pub order_token_account_wsol: Pubkey,
    pub system_program: Pubkey,
    pub associated_token_program: Pubkey,
    pub token_mint_x_program: Pubkey,
    pub token_mint_y_program: Pubkey,
    pub token_program: Pubkey,
}

impl From<DarklakeAmmSettle> for Vec<AccountMeta> {
    fn from(accounts: DarklakeAmmSettle) -> Self {
        vec![
            AccountMeta::new(accounts.caller, true),
            AccountMeta::new(accounts.order_owner, false),
            AccountMeta::new_readonly(accounts.token_mint_x, false),
            AccountMeta::new_readonly(accounts.token_mint_y, false),
            AccountMeta::new_readonly(accounts.token_mint_wsol, false),
            AccountMeta::new(accounts.pool, false),
            AccountMeta::new_readonly(accounts.authority, false),
            AccountMeta::new(accounts.pool_token_reserve_x, false),
            AccountMeta::new(accounts.pool_token_reserve_y, false),
            AccountMeta::new(accounts.pool_wsol_reserve, false),
            AccountMeta::new_readonly(accounts.amm_config, false),
            AccountMeta::new(accounts.user_token_account_x, false),
            AccountMeta::new(accounts.user_token_account_y, false),
            AccountMeta::new(accounts.user_token_account_wsol, false),
            AccountMeta::new(accounts.caller_token_account_wsol, false),
            AccountMeta::new(accounts.order, false),
            AccountMeta::new(accounts.order_token_account_wsol, false),
            AccountMeta::new_readonly(accounts.system_program, false),
            AccountMeta::new_readonly(accounts.associated_token_program, false),
            AccountMeta::new_readonly(accounts.token_mint_x_program, false),
            AccountMeta::new_readonly(accounts.token_mint_y_program, false),
            AccountMeta::new_readonly(accounts.token_program, false),
        ]
    }
}

pub(crate) struct DarklakeAmmCancel {
    pub caller: Pubkey,
    pub order_owner: Pubkey,
    pub token_mint_x: Pubkey,
    pub token_mint_y: Pubkey,
    pub token_mint_wsol: Pubkey,
    pub pool: Pubkey,
    pub authority: Pubkey,
    pub pool_token_reserve_x: Pubkey,
    pub pool_token_reserve_y: Pubkey,
    pub pool_wsol_reserve: Pubkey,
    pub amm_config: Pubkey,
    pub user_token_account_x: Pubkey,
    pub user_token_account_y: Pubkey,
    pub user_token_account_wsol: Pubkey,
    pub caller_token_account_wsol: Pubkey,
    pub order: Pubkey,
    pub system_program: Pubkey,
    pub associated_token_program: Pubkey,
    pub token_mint_x_program: Pubkey,
    pub token_mint_y_program: Pubkey,
    pub token_program: Pubkey,
}

impl From<DarklakeAmmCancel> for Vec<AccountMeta> {
    fn from(accounts: DarklakeAmmCancel) -> Self {
        vec![
            AccountMeta::new(accounts.caller, true),
            AccountMeta::new(accounts.order_owner, false),
            AccountMeta::new_readonly(accounts.token_mint_x, false),
            AccountMeta::new_readonly(accounts.token_mint_y, false),
            AccountMeta::new_readonly(accounts.token_mint_wsol, false),
            AccountMeta::new(accounts.pool, false),
            AccountMeta::new_readonly(accounts.authority, false),
            AccountMeta::new(accounts.pool_token_reserve_x, false),
            AccountMeta::new(accounts.pool_token_reserve_y, false),
            AccountMeta::new(accounts.pool_wsol_reserve, false),
            AccountMeta::new_readonly(accounts.amm_config, false),
            AccountMeta::new(accounts.user_token_account_x, false),
            AccountMeta::new(accounts.user_token_account_y, false),
            AccountMeta::new(accounts.user_token_account_wsol, false),
            AccountMeta::new(accounts.caller_token_account_wsol, false),
            AccountMeta::new(accounts.order, false),
            AccountMeta::new_readonly(accounts.system_program, false),
            AccountMeta::new_readonly(accounts.associated_token_program, false),
            AccountMeta::new_readonly(accounts.token_mint_x_program, false),
            AccountMeta::new_readonly(accounts.token_mint_y_program, false),
            AccountMeta::new_readonly(accounts.token_program, false),
        ]
    }
}

pub(crate) struct DarklakeAmmSlash {
    pub caller: Pubkey,
    pub order_owner: Pubkey,
    pub token_mint_x: Pubkey,
    pub token_mint_y: Pubkey,
    pub token_mint_wsol: Pubkey,
    pub pool: Pubkey,
    pub authority: Pubkey,
    pub pool_token_reserve_x: Pubkey,
    pub pool_token_reserve_y: Pubkey,
    pub pool_wsol_reserve: Pubkey,
    pub amm_config: Pubkey,
    pub user_token_account_x: Pubkey,
    pub user_token_account_y: Pubkey,
    pub caller_token_account_wsol: Pubkey,
    pub order: Pubkey,
    pub system_program: Pubkey,
    pub associated_token_program: Pubkey,
    pub token_mint_x_program: Pubkey,
    pub token_mint_y_program: Pubkey,
    pub token_program: Pubkey,
}

impl From<DarklakeAmmSlash> for Vec<AccountMeta> {
    fn from(accounts: DarklakeAmmSlash) -> Self {
        vec![
            AccountMeta::new(accounts.caller, true),
            AccountMeta::new(accounts.order_owner, false),
            AccountMeta::new_readonly(accounts.token_mint_x, false),
            AccountMeta::new_readonly(accounts.token_mint_y, false),
            AccountMeta::new_readonly(accounts.token_mint_wsol, false),
            AccountMeta::new(accounts.pool, false),
            AccountMeta::new_readonly(accounts.authority, false),
            AccountMeta::new(accounts.pool_token_reserve_x, false),
            AccountMeta::new(accounts.pool_token_reserve_y, false),
            AccountMeta::new(accounts.pool_wsol_reserve, false),
            AccountMeta::new_readonly(accounts.amm_config, false),
            AccountMeta::new(accounts.user_token_account_x, false),
            AccountMeta::new(accounts.user_token_account_y, false),
            AccountMeta::new(accounts.caller_token_account_wsol, false),
            AccountMeta::new(accounts.order, false),
            AccountMeta::new_readonly(accounts.system_program, false),
            AccountMeta::new_readonly(accounts.associated_token_program, false),
            AccountMeta::new_readonly(accounts.token_mint_x_program, false),
            AccountMeta::new_readonly(accounts.token_mint_y_program, false),
            AccountMeta::new_readonly(accounts.token_program, false),
        ]
    }
}

pub(crate) struct DarklakeAmmAddLiquidity {
    pub user: Pubkey,
    pub token_mint_x: Pubkey,
    pub token_mint_y: Pubkey,
    pub token_mint_lp: Pubkey,
    pub pool: Pubkey,
    pub authority: Pubkey,
    pub amm_config: Pubkey,
    pub user_token_account_x: Pubkey,
    pub user_token_account_y: Pubkey,
    pub user_token_account_lp: Pubkey,
    pub pool_token_reserve_x: Pubkey,
    pub pool_token_reserve_y: Pubkey,
    pub associated_token_program: Pubkey,
    pub system_program: Pubkey,
    pub token_mint_x_program: Pubkey,
    pub token_mint_y_program: Pubkey,
    pub token_program: Pubkey,
}

impl From<DarklakeAmmAddLiquidity> for Vec<AccountMeta> {
    fn from(accounts: DarklakeAmmAddLiquidity) -> Self {
        vec![
            AccountMeta::new(accounts.user, true),
            AccountMeta::new_readonly(accounts.token_mint_x, false),
            AccountMeta::new_readonly(accounts.token_mint_y, false),
            AccountMeta::new(accounts.token_mint_lp, false),
            AccountMeta::new(accounts.pool, false),
            AccountMeta::new_readonly(accounts.amm_config, false),
            AccountMeta::new_readonly(accounts.authority, false),
            AccountMeta::new(accounts.user_token_account_x, false),
            AccountMeta::new(accounts.user_token_account_y, false),
            AccountMeta::new(accounts.user_token_account_lp, false),
            AccountMeta::new(accounts.pool_token_reserve_x, false),
            AccountMeta::new(accounts.pool_token_reserve_y, false),
            AccountMeta::new_readonly(accounts.associated_token_program, false),
            AccountMeta::new_readonly(accounts.system_program, false),
            AccountMeta::new_readonly(accounts.token_mint_x_program, false),
            AccountMeta::new_readonly(accounts.token_mint_y_program, false),
            AccountMeta::new_readonly(accounts.token_program, false),
        ]
    }
}

pub(crate) struct DarklakeAmmRemoveLiquidity {
    pub user: Pubkey,
    pub token_mint_x: Pubkey,
    pub token_mint_y: Pubkey,
    pub token_mint_lp: Pubkey,
    pub pool: Pubkey,
    pub authority: Pubkey,
    pub amm_config: Pubkey,
    pub user_token_account_x: Pubkey,
    pub user_token_account_y: Pubkey,
    pub user_token_account_lp: Pubkey,
    pub pool_token_reserve_x: Pubkey,
    pub pool_token_reserve_y: Pubkey,
    pub associated_token_program: Pubkey,
    pub system_program: Pubkey,
    pub token_mint_x_program: Pubkey,
    pub token_mint_y_program: Pubkey,
    pub token_program: Pubkey,
}

impl From<DarklakeAmmRemoveLiquidity> for Vec<AccountMeta> {
    fn from(accounts: DarklakeAmmRemoveLiquidity) -> Self {
        vec![
            AccountMeta::new(accounts.user, true),
            AccountMeta::new_readonly(accounts.token_mint_x, false),
            AccountMeta::new_readonly(accounts.token_mint_y, false),
            AccountMeta::new_readonly(accounts.amm_config, false),
            AccountMeta::new(accounts.token_mint_lp, false),
            AccountMeta::new(accounts.pool, false),
            AccountMeta::new_readonly(accounts.authority, false),
            AccountMeta::new(accounts.user_token_account_x, false),
            AccountMeta::new(accounts.user_token_account_y, false),
            AccountMeta::new(accounts.user_token_account_lp, false),
            AccountMeta::new(accounts.pool_token_reserve_x, false),
            AccountMeta::new(accounts.pool_token_reserve_y, false),
            AccountMeta::new_readonly(accounts.associated_token_program, false),
            AccountMeta::new_readonly(accounts.system_program, false),
            AccountMeta::new_readonly(accounts.token_mint_x_program, false),
            AccountMeta::new_readonly(accounts.token_mint_y_program, false),
            AccountMeta::new_readonly(accounts.token_program, false),
        ]
    }
}
