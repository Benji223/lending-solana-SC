use std::f32::consts::E;

use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_interface::{ self, Mint, TokenAccount, TokenInterface, TransferChecked };

use pyth_solana_receiver_sdk::price_update::{get_feed_id_from_hex, PriceUpdateV2};

use crate::constants::{MAX_AGE, SOL_USD_FEED_ID, USDC_USD_FEED_ID};

use crate::state::{Bank, User};
use crate::error::ErrorCode;

#[derive(Accounts)]
pub struct Borrow<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        seeds = [mint.key().as_ref()],
        bump,
    )]
    pub bank: Account<'info, Bank>,

    #[account(
        mut,
        seeds = [b"treasury", mint.key().as_ref()],
        bump,
    )]
    pub bank_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [signer.key().as_ref()],
        bump,
    )]
    pub user_account: Account<'info, User>,

    #[account(
        init_if_needed,
        payer = signer,
        associated_token::mint = mint,
        associated_token::authority = signer,
        associated_token::token_program = token_program,
    )]
    pub user_token_account: InterfaceAccount<'info, TokenAccount>,

    // oracle to track price of our tokens
    pub price_update: Account<'info, PriceUpdateV2>, 
    

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    // The system program is used for account creation and other system-level operations
    // It is not directly involved in the borrow operation but is required for account initialization   
    pub system_program: Program<'info, System>,
    
    
}

pub fn process_borrow(
    ctx: Context<Borrow>,
    amount: u64
) -> Result<()> {

    // check the value the user has deposited throughout the lending protocol
    let bank= &mut ctx.accounts.bank;
    let user = &mut ctx.accounts.user_account;
    let price_update = &mut ctx.accounts.price_update;

    // check what is the collateral
    let total_collateral: u64;

    match ctx.accounts.mint.to_account_info().key()  {
        key if key == user.usdc_address => {
            let sol_feed_id = get_feed_id_from_hex(SOL_USD_FEED_ID)?;
            let sol_price = price_update.get_price_no_older_than(&Clock::get()?, MAX_AGE, &sol_feed_id)?;
            let new_value = calculate_accrued_interest(user.deposited_sol, bank.interest_rate, user.last_updated)?;
            total_collateral = sol_price.price as u64 * new_value ;
        }
        _ => {
            let usdc_feed_id = get_feed_id_from_hex(USDC_USD_FEED_ID)?;
            let usdc_price = price_update.get_price_no_older_than(&Clock::get()?, MAX_AGE, &usdc_feed_id)?;
            let new_value = calculate_accrued_interest(user.deposited_usdc, bank.interest_rate, user.last_updated)?;
            total_collateral = usdc_price.price as u64 * new_value;
        }
    }
    

    // Ensure the user has enough collateral to borrow
    let borrowable_amount = total_collateral.checked_mul(bank.liquidation_threshold).unwrap(); 

    if borrowable_amount < amount {
        return Err(ErrorCode::OverBorrowableAmount.into());
    }

    // Transfer tokens from the bank's treasury to the user's token account
    let transfer_cpi_accounts = TransferChecked {
        from: ctx.accounts.bank_token_account.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        to: ctx.accounts.user_token_account.to_account_info(),
        authority: ctx.accounts.bank_token_account.to_account_info(),
    };

    let cpi_program = ctx.accounts.token_program.to_account_info();

    let mint_key = ctx.accounts.mint.key();
    // Use the bank's token account bump to derive the signer seeds
    // This assumes that the bank_token_account is initialized with the correct seeds and bump
    let signer_seeds: &[&[&[u8]]] = &[
        &[
            b"treasury",
            mint_key.as_ref(),
            &[ctx.bumps.bank_token_account],
        ],
    ];

    let cpi_ctx = CpiContext::new(cpi_program, transfer_cpi_accounts).with_signer(signer_seeds);

    let decimals = ctx.accounts.mint.decimals;

    token_interface::transfer_checked(
        cpi_ctx,
        amount,
        decimals,
    )?;


    // update the state of the bank and user 

    if bank.total_borrowed == 0 {
        bank.total_borrowed = amount;
        bank.total_borrowed_shares = amount;
    }

    let borrowed_ratio = amount.checked_div(bank.total_borrowed).unwrap();
    let user_shares = bank.total_borrowed_shares.checked_mul(borrowed_ratio).unwrap();

    match ctx.accounts.mint.to_account_info().key() {
        key if key == user.usdc_address => {
            user.borrowed_usdc += amount;
            user.borrowed_usdc_shares += user_shares;
        }
        _ => {
            user.borrowed_sol += amount;
            user.borrowed_sol_shares += user_shares;
        }       
    }

    
    user.last_updated_borrowed = Clock::get()?.unix_timestamp;

    Ok(())
}

pub fn calculate_accrued_interest(
    deposited: u64,
    interest_rate: u64,
    last_updated: i64,
) -> Result<u64> {
    
    // Get the current time
    let current_time = Clock::get()?.unix_timestamp;

    // Calculate the time difference between the last update and the current time
    let time_diff = current_time - last_updated;
    
    // Calculate the accrued interest based on the bank's interest rate and time difference
    let new_value = (deposited as f64 * E.powf( interest_rate as f32 * time_diff as f32) as f64) as u64;
    
    Ok(new_value)
}