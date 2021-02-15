use std::convert::{TryFrom, TryInto};
use std::fmt;

use rand::seq::SliceRandom;
use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaCha20Rng;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, MoveError>;

#[derive(Clone, Debug, PartialEq)]
pub struct Game {
    columns: Vec<CardColumn>,
    foundations: Vec<Card>, // when empty, a foundation holds a card with rank 0
    free_cells: Vec<Option<Card>>,
    floating: Option<Card>,
    floating_stack: Option<Vec<Card>>,
}

#[derive(Debug, PartialEq)]
pub struct GameView {
    pub columns: Vec<Vec<Card>>,
    pub foundations: Vec<Card>,
    pub free_cells: Vec<Option<Card>>,
    pub floating: Option<Vec<Card>>,
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
                write!(f, "{}\n", print_string)?;
            } else {
                break;
            }
        }
        if let Some(cards) = &self.floating {
            write!(f, "-> ")?;
            for card in cards {
                write!(f, "{},", card)?;
            }
            write!(f, "\n")?;
        }
        Ok(())
    }
}

type CardColumn = Vec<Card>;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Card {
    pub rank: u8,
    pub suit: Suit,
}

impl Card {
    fn stacks_on(&self, base: &Card) -> bool {
        self.suit.colour() != base.suit.colour() && base.rank == self.rank + 1
    }

    fn fits_on_foundation(&self, base: &Card) -> bool {
        self.suit == base.suit && self.rank == base.rank + 1
    }

    fn new(rank: u8, suit: Suit) -> Self {
        Card { rank, suit }
    }
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rank = match self.rank {
            0 => "_".to_string(),
            11 => "J".to_string(),
            12 => "Q".to_string(),
            13 => "K".to_string(),
            n @ (1..=10) => n.to_string(),
            n => panic!("bad card {}", n),
        };
        let suit = match self.suit {
            Suit::Clubs => "♣",
            Suit::Diamonds => "♦",
            Suit::Hearts => "♥",
            Suit::Spades => "♠",
        };
        write!(f, "{}", rank + suit)
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Suit {
    Clubs,
    Diamonds,
    Hearts,
    Spades,
}

#[derive(PartialEq)]
enum Colour {
    Red,
    Black,
}

impl Suit {
    fn colour(&self) -> Colour {
        match self {
            Suit::Clubs => Colour::Black,
            Suit::Diamonds => Colour::Red,
            Suit::Hearts => Colour::Red,
            Suit::Spades => Colour::Black,
        }
    }
}

impl From<Suit> for usize {
    fn from(suit: Suit) -> Self {
        match suit {
            Suit::Clubs => 0,
            Suit::Diamonds => 1,
            Suit::Hearts => 2,
            Suit::Spades => 3,
        }
    }
}

impl TryFrom<usize> for Suit {
    type Error = ();
    fn try_from(n: usize) -> std::result::Result<Self, Self::Error> {
        match n {
            0 => Ok(Suit::Clubs),
            1 => Ok(Suit::Diamonds),
            2 => Ok(Suit::Hearts),
            3 => Ok(Suit::Spades),
            _ => Err(()),
        }
    }
}

impl fmt::Display for Suit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let suit_str = match self {
            Suit::Clubs => "clubs",
            Suit::Diamonds => "diamonds",
            Suit::Hearts => "hearts",
            Suit::Spades => "spades",
        };
        write!(f, "{}", suit_str)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CardAddress {
    Column(usize),
    Foundation(Suit),
    FreeCell(usize),
}

impl fmt::Display for CardAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CardAddress::Column(a) => write!(f, "column {}", a),
            CardAddress::Foundation(s) => write!(f, "foundation {}", s),
            CardAddress::FreeCell(a) => write!(f, "free cell {}", a),
        }
    }
}

#[derive(Error, Debug, PartialEq)]
pub enum MoveError {
    #[error("cannot move current cards to {to}: {reason}")]
    CannotPlace { reason: String, to: CardAddress },
    #[error("cannot pick up cards from {from}: {reason}")]
    CannotPickUp { reason: String, from: CardAddress },
    #[error("address {address} does not exist on the board")]
    IllegalAddress { address: CardAddress },
}

const REASON_ALREADY_HOLDING: &str = "already holding cards";
const REASON_EMPTY_ADDRESS: &str = "empty address";
const REASON_MOVE_FOUNDATION: &str = "cannot move off foundation";
const REASON_EMPTY_STACK: &str = "cannot pick up zero-card stack";
const REASON_UNSOUND_STACK: &str = "cards in stack don't stack";
const REASON_STACK_TOO_LARGE: &str = "cannot pick up that many cards at once";
const REASON_STACK_LARGER_THAN_COLUMN: &str = "there are not that many cards in that column";
const REASON_DOES_NOT_FIT: &str = "those cards do not fit there";
const REASON_NO_CARDS_HELD: &str = "cannot place cards when not holding cards";
const REASON_CAN_ONLY_GET_STACK_FROM_COLUMN: &str =
    "cannot pick up a stack from anywhere except a column";

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
        let mut rng = ChaCha20Rng::seed_from_u64(seed);
        deck.shuffle(&mut rng);
        for &n in &[7, 7, 7, 7, 6, 6, 6] {
            let (new, remainder) = deck.split_at(n);
            spread.columns.push(Vec::from(new));
            deck = Vec::from(remainder);
        }
        spread.columns.push(deck);
        spread
    }

    // look at a card
    fn _get(&self, address: CardAddress) -> Result<Card> {
        match address {
            CardAddress::Column(i) => {
                if let Some(column) = self.columns.get(i) {
                    if let Some(&card) = column.last() {
                        Ok(card)
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
            CardAddress::Foundation(s) => match self.foundations.get(usize::from(s)) {
                Some(card) => Ok(*card),
                None => Err(MoveError::IllegalAddress { address }),
            },
            CardAddress::FreeCell(i) => match self.free_cells.get(i) {
                Some(Some(card)) => Ok(*card),
                Some(None) => Err(MoveError::CannotPickUp {
                    from: address,
                    reason: REASON_EMPTY_ADDRESS.to_string(),
                }),
                None => Err(MoveError::IllegalAddress { address }),
            },
        }
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

    pub fn is_won(&self) -> bool {
        self.foundations
            .iter()
            .map(|c| c.rank)
            .fold((true, 13), |(eq, val), next| (next == val && eq, val))
            .0
    }
}

#[test]
fn test_moves() {
    let mut spread = Game::empty();
    spread.columns = vec![
        vec![
            Card::new(6, Suit::Hearts),
            Card::new(5, Suit::Spades),
            Card::new(4, Suit::Hearts),
            Card::new(3, Suit::Spades),
            Card::new(2, Suit::Hearts),
            Card::new(1, Suit::Spades),
        ],
        vec![
            Card::new(7, Suit::Clubs),
            Card::new(6, Suit::Diamonds),
            Card::new(5, Suit::Clubs),
        ],
        Vec::new(),
        vec![
            Card::new(7, Suit::Hearts),
            Card::new(6, Suit::Diamonds),
            Card::new(5, Suit::Clubs),
        ],
    ];

    // move 1-4 onto second column
    assert_eq!(
        spread._get(CardAddress::Column(0)),
        Ok(Card::new(1, Suit::Spades))
    );
    assert_eq!(
        spread.pick_up_stack(CardAddress::Column(4), 3),
        Err(MoveError::IllegalAddress {
            address: CardAddress::Column(4)
        }),
    );
    assert_eq!(
        spread.pick_up_stack(CardAddress::Column(1), 4),
        Err(MoveError::CannotPickUp {
            from: CardAddress::Column(1),
            reason: REASON_STACK_LARGER_THAN_COLUMN.to_string(),
        }),
    );
    assert_eq!(
        spread.pick_up_stack(CardAddress::Column(3), 3),
        Err(MoveError::CannotPickUp {
            from: CardAddress::Column(3),
            reason: REASON_UNSOUND_STACK.to_string(),
        }),
    );
    spread = spread.pick_up_stack(CardAddress::Column(0), 4).unwrap();
    assert_eq!(
        spread.pick_up_stack(CardAddress::Column(0), 2),
        Err(MoveError::CannotPickUp {
            from: CardAddress::Column(0),
            reason: REASON_ALREADY_HOLDING.to_string(),
        })
    );
    assert_eq!(
        spread.pick_up_card(CardAddress::Column(0)),
        Err(MoveError::CannotPickUp {
            from: CardAddress::Column(0),
            reason: REASON_ALREADY_HOLDING.to_string(),
        })
    );
    assert_eq!(
        spread._get(CardAddress::Column(0)),
        Ok(Card::new(5, Suit::Spades))
    );
    assert_eq!(
        spread.place(CardAddress::FreeCell(0)),
        Err(MoveError::CannotPlace {
            to: CardAddress::FreeCell(0),
            reason: REASON_DOES_NOT_FIT.to_string(),
        })
    );
    assert_eq!(
        spread.place(CardAddress::Column(4)),
        Err(MoveError::IllegalAddress {
            address: CardAddress::Column(4)
        })
    );
    spread = spread.place(CardAddress::Column(1)).unwrap();
    assert_eq!(
        spread._get(CardAddress::Column(1)),
        Ok(Card::new(1, Suit::Spades))
    );
    assert_eq!(
        spread.pick_up_stack(CardAddress::Column(1), 6),
        Err(MoveError::CannotPickUp {
            from: CardAddress::Column(1),
            reason: REASON_STACK_TOO_LARGE.to_string(),
        })
    );
    spread = spread.pick_up_stack(CardAddress::Column(1), 5).unwrap();
    spread = spread.place(CardAddress::Column(1)).unwrap();

    // move ace onto foundation
    spread = spread.pick_up_card(CardAddress::Column(1)).unwrap();
    assert_eq!(
        spread.place(CardAddress::Column(0)),
        Err(MoveError::CannotPlace {
            to: CardAddress::Column(0),
            reason: REASON_DOES_NOT_FIT.to_string(),
        })
    );
    assert_eq!(
        spread.place(CardAddress::Foundation(Suit::Hearts)),
        Err(MoveError::CannotPlace {
            to: CardAddress::Foundation(Suit::Hearts),
            reason: REASON_DOES_NOT_FIT.to_string(),
        })
    );
    spread = spread.place(CardAddress::Foundation(Suit::Spades)).unwrap();
    assert_eq!(
        spread._get(CardAddress::Foundation(Suit::Spades)),
        Ok(Card::new(1, Suit::Spades))
    );

    // manually move cards up to 5 from 2nd column back to first; moved 5 from first to new column
    spread = spread.pick_up_card(CardAddress::Column(1)).unwrap();
    spread = spread.place(CardAddress::FreeCell(0)).unwrap();
    spread = spread.pick_up_card(CardAddress::Column(1)).unwrap();
    assert_eq!(
        spread.place(CardAddress::FreeCell(0)),
        Err(MoveError::CannotPlace {
            to: CardAddress::FreeCell(0),
            reason: REASON_DOES_NOT_FIT.to_string(),
        })
    );
    spread = spread.place(CardAddress::FreeCell(1)).unwrap();
    spread = spread.pick_up_card(CardAddress::Column(1)).unwrap();
    spread = spread.place(CardAddress::FreeCell(2)).unwrap();
    spread = spread.pick_up_card(CardAddress::Column(0)).unwrap();
    spread = spread.place(CardAddress::FreeCell(3)).unwrap();
    spread = spread.pick_up_card(CardAddress::Column(1)).unwrap();
    spread = spread.place(CardAddress::Column(0)).unwrap();
    spread = spread.pick_up_card(CardAddress::FreeCell(2)).unwrap();
    spread = spread.place(CardAddress::Column(0)).unwrap();
    assert_eq!(
        spread.pick_up_card(CardAddress::FreeCell(2)),
        Err(MoveError::CannotPickUp {
            from: CardAddress::FreeCell(2),
            reason: REASON_EMPTY_ADDRESS.to_string(),
        })
    );
    spread = spread.pick_up_card(CardAddress::FreeCell(1)).unwrap();
    spread = spread.place(CardAddress::Column(0)).unwrap();
    spread = spread.pick_up_card(CardAddress::FreeCell(0)).unwrap();
    spread = spread.place(CardAddress::Column(0)).unwrap();
    println!("{}", spread.view());
    spread = spread.pick_up_card(CardAddress::FreeCell(3)).unwrap();
    let _ = spread.place(CardAddress::Column(2)).unwrap();
}

#[test]
fn auto_move() {
    let mut game = Game::empty();
    game.columns.push(
        (1..=5)
            .rev()
            .map(|n| Card::new(n, Suit::Spades))
            .collect::<Vec<Card>>(),
    );
    game.columns.push(vec![
        Card::new(3, Suit::Clubs),
        Card::new(4, Suit::Clubs),
        Card::new(2, Suit::Clubs),
        Card::new(1, Suit::Clubs),
    ]);
    game.columns.push(vec![
        Card::new(3, Suit::Diamonds),
        Card::new(2, Suit::Diamonds),
        Card::new(1, Suit::Diamonds),
        Card::new(2, Suit::Hearts),
        Card::new(1, Suit::Hearts),
    ]);
    while let Some(new_state) = game.auto_move_to_foundations() {
        game = new_state;
    }
    assert_eq!(
        game.foundations,
        vec![
            Card::new(2, Suit::Clubs,),
            Card::new(3, Suit::Diamonds,),
            Card::new(2, Suit::Hearts,),
            Card::new(4, Suit::Spades,),
        ]
    )
}

#[test]
fn test_rng() {
    for seed in 0..10 {
        let a = Game::new_game(seed);
        let b = Game::new_game(seed);
        assert_eq!(a, b);
    }
}

#[test]
fn test_won() {
    let mut game = Game::empty();
    assert!(!game.is_won());
    game.foundations = vec![
        Card::new(13, Suit::Clubs),
        Card::new(13, Suit::Diamonds),
        Card::new(13, Suit::Hearts),
        Card::new(12, Suit::Spades),
    ];
    assert!(!game.is_won());
    game.foundations[3] = Card::new(13, Suit::Spades);
    assert!(game.is_won());
}
