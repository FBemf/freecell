use thiserror::Error;

use super::card::*;

pub type Result<T> = std::result::Result<T, MoveError>;

#[derive(Error, Debug, PartialEq)]
pub enum MoveError {
    #[error("cannot move current cards to {to}: {reason}")]
    CannotPlace { reason: String, to: CardAddress },
    #[error("cannot pick up cards from {from}: {reason}")]
    CannotPickUp { reason: String, from: CardAddress },
    #[error("address {address} does not exist on the board")]
    IllegalAddress { address: CardAddress },
}

pub const REASON_ALREADY_HOLDING: &str = "already holding cards";
pub const REASON_EMPTY_ADDRESS: &str = "empty address";
pub const REASON_MOVE_FOUNDATION: &str = "cannot move off foundation";
pub const REASON_EMPTY_STACK: &str = "cannot pick up zero-card stack";
pub const REASON_UNSOUND_STACK: &str = "cards in stack don't stack";
pub const REASON_STACK_TOO_LARGE: &str = "cannot pick up that many cards at once";
pub const REASON_STACK_LARGER_THAN_COLUMN: &str = "there are not that many cards in that column";
pub const REASON_DOES_NOT_FIT: &str = "those cards do not fit there";
pub const REASON_NO_CARDS_HELD: &str = "cannot place cards when not holding cards";
pub const REASON_CAN_ONLY_GET_STACK_FROM_COLUMN: &str =
    "cannot pick up a stack from anywhere except a column";
