use std::convert::TryInto;
use std::fmt;

use rand::seq::SliceRandom;
use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaCha12Rng;
use serde::{Deserialize, Serialize};

use super::card::*;
use super::error::*;

#[cfg(test)]
mod test;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Game {
    columns: Vec<CardColumn>,
    foundations: Vec<Card>, // when empty, a foundation holds a card with rank 0
    free_cells: Vec<Option<Card>>,
    floating: Option<Card>,
    floating_stack: Option<Vec<Card>>,
}

impl Game {
    fn empty() -> Self {
        Game {
            columns: Vec::new(),
            foundations: (0..4)
                .map(|n: usize| Card::new(0, n.try_into().unwrap()))
                .collect(),
            free_cells: vec![None; 4],
            floating: None,
            floating_stack: None,
        }
    }

    // shuffle & create a new game
    pub fn new_game(seed: u64) -> Self {
        let mut spread = Game::empty();
        let mut deck = Vec::with_capacity(52);
        for &suit in &[Suit::Clubs, Suit::Diamonds, Suit::Spades, Suit::Hearts] {
            for rank in 1..=13 {
                deck.push(Card::new(rank, suit));
            }
        }
        let mut rng = ChaCha12Rng::seed_from_u64(seed);
        deck.shuffle(&mut rng);
        for &n in &[7, 7, 7, 7, 6, 6, 6] {
            let (new, remainder) = deck.split_at(n);
            spread.columns.push(Vec::from(new));
            deck = Vec::from(remainder);
        }
        spread.columns.push(deck);
        spread
    }

    // pick up a card from a position
    pub fn pick_up_card(&self, address: CardAddress) -> Result<Self> {
        if self.floating != None || self.floating_stack != None {
            return Err(MoveError::CannotPickUp {
                from: address,
                reason: REASON_ALREADY_HOLDING.to_string(),
            });
        }
        match address {
            CardAddress::Column(i) => {
                let mut result = self.clone();
                if let Some(column) = &mut result.columns.get_mut(i) {
                    if let Some(card) = column.pop() {
                        result.floating = Some(card);
                        Ok(result)
                    } else {
                        Err(MoveError::CannotPickUp {
                            from: address,
                            reason: REASON_EMPTY_ADDRESS.to_string(),
                        })
                    }
                } else {
                    Err(MoveError::IllegalAddress { address })
                }
            }

            CardAddress::Foundation(s) => Err(MoveError::CannotPickUp {
                from: CardAddress::Foundation(s),
                reason: REASON_MOVE_FOUNDATION.to_string(),
            }),

            CardAddress::FreeCell(i) => {
                let mut result = self.clone();
                if let Some(free_cell) = result.free_cells.get_mut(i) {
                    if let Some(card) = free_cell.clone() {
                        *free_cell = None;
                        result.floating = Some(card);
                        Ok(result)
                    } else {
                        Err(MoveError::CannotPickUp {
                            from: address,
                            reason: REASON_EMPTY_ADDRESS.to_string(),
                        })
                    }
                } else {
                    Err(MoveError::IllegalAddress { address })
                }
            }
        }
    }

    // try to pick up a stack of cards from a position
    pub fn pick_up_stack(&self, address: CardAddress, number_of_cards: usize) -> Result<Self> {
        if let CardAddress::Column(column_number) = address {
            if self.floating != None || self.floating_stack != None {
                return Err(MoveError::CannotPickUp {
                    from: address,
                    reason: REASON_ALREADY_HOLDING.to_string(),
                });
            }
            let max_possible_stack_size = self.max_stack_size();
            match number_of_cards {
                0 => Err(MoveError::CannotPickUp {
                    from: address,
                    reason: REASON_EMPTY_STACK.to_string(),
                }),
                1 => self.pick_up_card(address),
                _ => {
                    let mut result = self.clone();
                    if let Some(column) = &mut result.columns.get_mut(column_number) {
                        if number_of_cards <= column.len() {
                            if number_of_cards <= max_possible_stack_size {
                                let it = column.iter().rev();
                                for pair in it.clone().take(number_of_cards - 1).zip(it.skip(1)) {
                                    if !pair.0.stacks_on(pair.1) {
                                        return Err(MoveError::CannotPickUp {
                                            from: address,
                                            reason: REASON_UNSOUND_STACK.to_string(),
                                        });
                                    }
                                }
                                let floating_stack =
                                    column.split_off(column.len() - number_of_cards);
                                result.floating_stack = Some(floating_stack);
                                Ok(result)
                            } else {
                                Err(MoveError::CannotPickUp {
                                    from: address,
                                    reason: REASON_STACK_TOO_LARGE.to_string(),
                                })
                            }
                        } else {
                            Err(MoveError::CannotPickUp {
                                from: address,
                                reason: REASON_STACK_LARGER_THAN_COLUMN.to_string(),
                            })
                        }
                    } else {
                        Err(MoveError::IllegalAddress { address })
                    }
                }
            }
        } else {
            Err(MoveError::CannotPickUp {
                from: address,
                reason: REASON_CAN_ONLY_GET_STACK_FROM_COLUMN.to_string(),
            })
        }
    }

    // place the held card at a position
    pub fn place(&self, address: CardAddress) -> Result<Self> {
        let mut result = self.clone();
        match address {
            CardAddress::Column(i) => {
                if let Some(column) = &mut result.columns.get_mut(i) {
                    if let Some(card) = result.floating {
                        if column.is_empty() || card.stacks_on(column.last().unwrap()) {
                            column.push(card);
                            result.floating = None;
                            Ok(result)
                        } else {
                            Err(MoveError::CannotPlace {
                                to: address,
                                reason: REASON_DOES_NOT_FIT.to_string(),
                            })
                        }
                    } else if let Some(cards) = &mut result.floating_stack {
                        if column.is_empty()
                            || cards.first().unwrap().stacks_on(column.last().unwrap())
                        {
                            column.append(cards);
                            result.floating_stack = None;
                            Ok(result)
                        } else {
                            Err(MoveError::CannotPlace {
                                to: address,
                                reason: REASON_DOES_NOT_FIT.to_string(),
                            })
                        }
                    } else {
                        Err(MoveError::CannotPlace {
                            to: address,
                            reason: REASON_NO_CARDS_HELD.to_string(),
                        })
                    }
                } else {
                    Err(MoveError::IllegalAddress { address })
                }
            }

            CardAddress::Foundation(s) => {
                if let Some(foundation) = result.foundations.get_mut(usize::from(s)) {
                    if let Some(card) = result.floating {
                        if card.fits_on_foundation(foundation) {
                            *foundation = card;
                            result.floating = None;
                            Ok(result)
                        } else {
                            Err(MoveError::CannotPlace {
                                to: CardAddress::Foundation(s),
                                reason: REASON_DOES_NOT_FIT.to_string(),
                            })
                        }
                    } else {
                        Err(MoveError::CannotPlace {
                            to: address,
                            reason: REASON_NO_CARDS_HELD.to_string(),
                        })
                    }
                } else {
                    Err(MoveError::IllegalAddress { address })
                }
            }

            CardAddress::FreeCell(i) => {
                if let Some(free_cell) = result.free_cells.get_mut(i) {
                    if *free_cell == None {
                        if let Some(card) = result.floating {
                            *free_cell = Some(card);
                            result.floating = None;
                            Ok(result)
                        } else {
                            Err(MoveError::CannotPlace {
                                to: CardAddress::FreeCell(i),
                                reason: REASON_DOES_NOT_FIT.to_string(),
                            })
                        }
                    } else {
                        Err(MoveError::CannotPlace {
                            to: address,
                            reason: REASON_DOES_NOT_FIT.to_string(),
                        })
                    }
                } else {
                    Err(MoveError::IllegalAddress { address })
                }
            }
        }
    }

    // get a look at the state of the board
    pub fn view(&self) -> GameView {
        let floating = if let Some(card) = self.floating {
            Some(vec![card])
        } else if let Some(cards) = self.floating_stack.clone() {
            Some(cards)
        } else {
            None
        };
        GameView {
            columns: self.columns.clone(),
            foundations: self.foundations.clone(),
            free_cells: self.free_cells.clone(),
            floating,
        }
    }

    pub fn has_floating(&self) -> bool {
        self.floating.is_some() || self.floating_stack.is_some()
    }

    fn max_stack_size(&self) -> usize {
        let num_empty_free_cells: usize = self
            .free_cells
            .iter()
            .map(|&c| if None == c { 1 } else { 0 })
            .sum();
        1 + num_empty_free_cells
    }

    // move a card to its foundation if possible. returns true if you moved any
    pub fn auto_move_to_foundations(&self) -> Option<Self> {
        if self.floating != None || self.floating_stack != None {
            return None;
        }
        let mut result = self.clone();
        for (index, column_card) in result
            .columns
            .iter()
            .map(|c| match c.last() {
                Some(&v) => Some(v.clone()),
                None => None,
            })
            .enumerate()
            .collect::<Vec<(usize, Option<Card>)>>()
        {
            if let Some(card) = column_card {
                if result.can_auto_move(card) {
                    result = result.pick_up_card(CardAddress::Column(index)).unwrap();
                    result = result.place(CardAddress::Foundation(card.suit)).unwrap();
                    return Some(result);
                }
            }
        }
        return None;
    }

    fn can_auto_move(&self, card: Card) -> bool {
        if self.foundations[usize::from(card.suit)].rank != card.rank - 1 {
            return false;
        }
        match card.suit.colour() {
            Colour::Red => {
                let clubs = self.foundations[usize::from(Suit::Clubs)].rank >= card.rank - 1
                    || self.can_auto_move(Card::new(card.rank - 1, Suit::Clubs));
                let spades = self.foundations[usize::from(Suit::Spades)].rank >= card.rank - 1
                    || self.can_auto_move(Card::new(card.rank - 1, Suit::Spades));
                clubs && spades
            }
            Colour::Black => {
                let diamonds = self.foundations[usize::from(Suit::Diamonds)].rank >= card.rank - 1
                    || self.can_auto_move(Card::new(card.rank - 1, Suit::Diamonds));
                let hearts = self.foundations[usize::from(Suit::Hearts)].rank >= card.rank - 1
                    || self.can_auto_move(Card::new(card.rank - 1, Suit::Hearts));
                hearts && diamonds
            }
        }
    }
}

pub fn _game_from_columns(columns: Vec<Vec<Card>>) -> Game {
    let mut game = Game::empty();
    game.columns = columns;
    game
}

#[derive(Debug, PartialEq)]
pub struct GameView {
    pub columns: Vec<Vec<Card>>,
    pub foundations: Vec<Card>,
    pub free_cells: Vec<Option<Card>>,
    pub floating: Option<Vec<Card>>,
}

impl GameView {
    pub fn is_won(&self) -> bool {
        self.foundations
            .iter()
            .map(|c| c.rank)
            .fold((true, 13), |(eq, val), next| (next == val && eq, val))
            .0
    }
}

impl fmt::Display for GameView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for cell in &self.free_cells {
            if let Some(card) = cell {
                write!(f, "{} ", card)?;
            } else {
                write!(f, "    ")?;
            }
        }
        for foundation in &self.foundations {
            write!(f, "{} ", foundation)?;
        }
        write!(f, "\n")?;
        for row in 0.. {
            write!(f, "\n")?;
            let mut printed_something = false;
            let mut print_string = String::new();
            for column in &self.columns {
                if let Some(card) = column.get(row) {
                    print_string += &format!("{} ", card).as_str();
                    printed_something = true;
                } else {
                    print_string += &format!("    ").as_str();
                }
            }
            if printed_something {
                write!(f, "{}", print_string)?;
            } else {
                break;
            }
        }
        if let Some(cards) = &self.floating {
            write!(f, "\n-> ")?;
            for card in cards {
                write!(f, "{},", card)?;
            }
        }
        Ok(())
    }
}
