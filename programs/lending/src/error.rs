use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Insufficient funds for this operation.")]
    InsufficientFunds,

    #[msg("Resuest Exceeds Over borrowable amount.")]
    OverBorrowableAmount,

    #[msg("Over repay amount.")]
    OverRepay,

    #[msg("Health factor is above 1.0, liquidation not required.")]
    HealthFactorAboveOne,

    
}