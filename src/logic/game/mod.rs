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
    // unstable internal state of the game
    state: State,
    // api-stable state of the game
    // pre-calculated, so that you can call view() multiple times and not copy data every time
    view: GameView,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct State {
    columns: Vec<CardColumn>,
    foundations: Vec<Card>, // when empty, a foundation holds a card with rank 0
    free_cells: Vec<Option<Card>>,
    floating: Option<Card>,
    floating_stack: Option<Vec<Card>>,
}

impl From<State> for Game {
    // we handle only States internally, and we return Games with state.into().
    // this way, we always correctly calculate the new view right when we return
    fn from(state: State) -> Self {
        let floating = if let Some(card) = state.floating {
            Some(vec![card])
        } else if let Some(cards) = state.floating_stack.clone() {
            Some(cards)
        } else {
            None
        };
        let view = GameView {
            columns: state.columns.clone(),
            foundations: state.foundations.clone(),
            free_cells: state.free_cells.clone(),
            floating,
        };
        Game { state, view }
    }
}

impl Game {
    fn empty() -> Self {
        State {
            columns: Vec::new(),
            foundations: (0..4)
                .map(|n: usize| Card::new(0, n.try_into().unwrap()))
                .collect(),
            free_cells: vec![None; 4],
            floating: None,
            floating_stack: None,
        }
        .into()
    }

    // shuffle & create a new game
    pub fn new_game(seed: u64) -> Self {
        let mut spread = Game::empty().state;
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
        spread.into()
    }

    // pick up a card from a position
    pub fn pick_up_card(&self, address: CardAddress) -> Result<Self> {
        // can't pick up a cards if you're already holding cards
        if self.state.floating != None || self.state.floating_stack != None {
            return Err(MoveError::CannotPickUp {
                from: address,
                reason: REASON_ALREADY_HOLDING.to_string(),
            });
        }
        match address {
            // Pick up one card from a column
            CardAddress::Column(i) => {
                let mut result = self.state.clone();
                if let Some(column) = &mut result.columns.get_mut(i) {
                    if let Some(card) = column.pop() {
                        result.floating = Some(card);
                        Ok(result.into())
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
            // it's illegal to pick up from a foundation
            CardAddress::Foundation(s) => Err(MoveError::CannotPickUp {
                from: CardAddress::Foundation(s),
                reason: REASON_MOVE_FOUNDATION.to_string(),
            }),
            // pick up a card from a free cell
            CardAddress::FreeCell(i) => {
                let mut result = self.state.clone();
                if let Some(free_cell) = result.free_cells.get_mut(i) {
                    if let Some(card) = *free_cell {
                        *free_cell = None;
                        result.floating = Some(card);
                        Ok(result.into())
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
            // can't pick up a cards if you're already holding cards
            if self.state.floating != None || self.state.floating_stack != None {
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
                    let mut result = self.state.clone();
                    // ensure we can legally pick up this many cards
                    if let Some(column) = &mut result.columns.get_mut(column_number) {
                        if number_of_cards <= column.len() {
                            if number_of_cards <= max_possible_stack_size {
                                // check if the cards in the column are legally allowed to stack
                                let it = column.iter().rev();
                                for pair in it.clone().take(number_of_cards - 1).zip(it.skip(1)) {
                                    if !pair.0.stacks_on(pair.1) {
                                        return Err(MoveError::CannotPickUp {
                                            from: address,
                                            reason: REASON_UNSOUND_STACK.to_string(),
                                        });
                                    }
                                }
                                // actually pick up the cards
                                let floating_stack =
                                    column.split_off(column.len() - number_of_cards);
                                result.floating_stack = Some(floating_stack);
                                Ok(result.into())
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
        let mut result = self.state.clone();
        match address {
            CardAddress::Column(i) => {
                if let Some(column) = &mut result.columns.get_mut(i) {
                    if let Some(card) = result.floating {
                        // try to place a single card onto a column
                        if column.is_empty() || card.stacks_on(column.last().unwrap()) {
                            column.push(card);
                            result.floating = None;
                            Ok(result.into())
                        } else {
                            Err(MoveError::CannotPlace {
                                to: address,
                                reason: REASON_DOES_NOT_FIT.to_string(),
                            })
                        }
                    } else if let Some(cards) = &mut result.floating_stack {
                        // try to place a stack of cards onto a column
                        if column.is_empty()
                            || cards.first().unwrap().stacks_on(column.last().unwrap())
                        {
                            column.append(cards);
                            result.floating_stack = None;
                            Ok(result.into())
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
                // try to move a card to a foundation
                if let Some(foundation) = result.foundations.get_mut(usize::from(s)) {
                    if let Some(card) = result.floating {
                        if card.fits_on_foundation(foundation) {
                            *foundation = card;
                            result.floating = None;
                            Ok(result.into())
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
                // try to move a card to a free cell
                if let Some(free_cell) = result.free_cells.get_mut(i) {
                    if *free_cell == None {
                        if let Some(card) = result.floating {
                            *free_cell = Some(card);
                            result.floating = None;
                            Ok(result.into())
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
    pub fn view(&self) -> &GameView {
        &self.view
    }

    // true if the player is holding cards
    pub fn has_floating(&self) -> bool {
        self.state.floating.is_some() || self.state.floating_stack.is_some()
    }

    // find the max number of cards the player can pick up as a stack.
    // not strictly the max cards the player can move at once, but the max
    // number they can pick up at once without restricting where they can place them.
    fn max_stack_size(&self) -> usize {
        let num_empty_free_cells: usize = self
            .state
            .free_cells
            .iter()
            .map(|&c| if None == c { 1 } else { 0 })
            .sum();
        1 + num_empty_free_cells
    }

    // move one arbitrary card to a foundation, if possible. returns true if it moved a card
    pub fn auto_move_to_foundations(&self) -> Option<Self> {
        if self.state.floating != None || self.state.floating_stack != None {
            return None;
        }
        for (index, column_card) in self
            .state
            .columns
            .iter()
            .map(|c| match c.last() {
                Some(&v) => Some(v),
                None => None,
            })
            .enumerate()
            .collect::<Vec<(usize, Option<Card>)>>()
        {
            let mut result = self.clone();
            if let Some(card) = column_card {
                if result.can_auto_move(card) {
                    result = result.pick_up_card(CardAddress::Column(index)).unwrap();
                    result = result.place(CardAddress::Foundation(card.suit)).unwrap();
                    return Some(result);
                }
            }
        }
        None
    }

    // true if a card can be auto-moved, i.e. it can move to a foundation and nothing else can stack on it
    fn can_auto_move(&self, card: Card) -> bool {
        if self.state.foundations[usize::from(card.suit)].rank != card.rank - 1 {
            return false;
        }
        match card.suit.colour() {
            Colour::Red => {
                // "clubs" is false if there is any club not yet in the foundations which can stack on this card
                let clubs = self.state.foundations[usize::from(Suit::Clubs)].rank >= card.rank - 1
                    || self.can_auto_move(Card::new(card.rank - 1, Suit::Clubs));
                // "spades" is false if there is any spade not yet in the foundations which can stack on this card
                let spades = self.state.foundations[usize::from(Suit::Spades)].rank
                    >= card.rank - 1
                    || self.can_auto_move(Card::new(card.rank - 1, Suit::Spades));
                clubs && spades
            }
            Colour::Black => {
                // "diamonds" is false if there is any diamond not yet in the foundations which can stack on this card
                let diamonds = self.state.foundations[usize::from(Suit::Diamonds)].rank
                    >= card.rank - 1
                    || self.can_auto_move(Card::new(card.rank - 1, Suit::Diamonds));
                // "hearts" is false if there is any heart not yet in the foundations which can stack on this card
                let hearts = self.state.foundations[usize::from(Suit::Hearts)].rank
                    >= card.rank - 1
                    || self.can_auto_move(Card::new(card.rank - 1, Suit::Hearts));
                hearts && diamonds
            }
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
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
        writeln!(f)?;
        for row in 0.. {
            writeln!(f)?;
            let mut printed_something = false;
            let mut print_string = String::new();
            for column in &self.columns {
                if let Some(card) = column.get(row) {
                    print_string += &format!("{} ", card).as_str();
                    printed_something = true;
                } else {
                    print_string += "    ";
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

// these things let other modules in logic affect the game state for testing & serialization.
// not exported by logic
pub mod inspect {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct StateContainer(State);

    pub fn game_get_state(game: &Game) -> StateContainer {
        StateContainer(game.state.clone())
    }

    pub fn game_from_state(state: StateContainer) -> Game {
        state.0.into()
    }

    #[cfg(test)]
    pub fn game_from_columns(columns: Vec<Vec<Card>>) -> Game {
        let mut game = Game::empty().state;
        game.columns = columns;
        game.into()
    }
}
