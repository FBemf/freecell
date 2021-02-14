use std::convert::{TryFrom, TryInto};
use std::fmt;

use rand::seq::SliceRandom;
use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaCha20Rng;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, MoveError>;

#[derive(Debug, PartialEq)]
pub struct Game {
    columns: Vec<CardColumn>,
    foundations: Vec<Card>, // when empty, a foundation holds a card with rank 0
    free_cells: Vec<Option<Card>>,
    can_move_off_foundations: bool,
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
                write!(f, "   ")?;
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
                    print_string += &format!("   ").as_str();
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
                write!(f, "{},", card);
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
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rank_ch = match self.rank {
            0 => "_".to_string(),
            10 => "X".to_string(),
            11 => "J".to_string(),
            12 => "Q".to_string(),
            13 => "K".to_string(),
            n @ (1..=9) => n.to_string(),
            n => panic!("bad card {}", n),
        };
        let suit_ch = match self.suit {
            Suit::Clubs => "C",
            Suit::Diamonds => "D",
            Suit::Hearts => "H",
            Suit::Spades => "S",
        };
        write!(f, "{}{}", rank_ch, suit_ch)
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

#[derive(Debug, PartialEq)]
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
    #[error("cannot move current cards to {to}")]
    CannotPlace { to: CardAddress },
    #[error("cannot place cards when no cards are floating")]
    NoCardsHeld,
    #[error("cannot pick up card: already holding cards")]
    AlreadyHoldingCards,
    #[error("cannot pick up {cards} cards from {column}")]
    ImpossibleStack { column: usize, cards: usize },
    #[error("address {address} is empty")]
    EmptyAddress { address: CardAddress },
    #[error("address {address} does not exist on the board")]
    IllegalAddress { address: CardAddress },
    #[error("cannot pick up zero cards")]
    ZeroCardStack,
    #[error("{cards} cards are not available in column {column}")]
    TooManyCardsStack { cards: usize, column: usize },
    #[error("a stack of cards cannot be placed at {to}")]
    CannotPlaceStack { to: CardAddress },
    #[error("cannot move off of a foundation pile")]
    CannotMoveOffFoundation { foundation: Suit },
}

impl Game {
    fn empty() -> Self {
        Game {
            columns: Vec::new(),
            foundations: (0..4)
                .map(|n: usize| Card {
                    rank: 0,
                    suit: n.try_into().unwrap(),
                })
                .collect(),
            free_cells: vec![None; 4],
            can_move_off_foundations: false,
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
                deck.push(Card { rank, suit });
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
    pub fn get(&self, address: CardAddress) -> Result<Card> {
        match address {
            CardAddress::Column(i) => {
                if let Some(column) = self.columns.get(i) {
                    if let Some(&card) = column.last() {
                        Ok(card)
                    } else {
                        Err(MoveError::EmptyAddress { address })
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
                Some(None) => Err(MoveError::EmptyAddress { address }),
                None => Err(MoveError::IllegalAddress { address }),
            },
        }
    }

    // pick up a card from a position
    pub fn pick_up_card(&mut self, address: CardAddress) -> Result<()> {
        if self.floating != None || self.floating_stack != None {
            return Err(MoveError::AlreadyHoldingCards);
        }
        match address {
            CardAddress::Column(i) => {
                if let Some(column) = &mut self.columns.get_mut(i) {
                    if let Some(card) = column.pop() {
                        self.floating = Some(card);
                        Ok(())
                    } else {
                        Err(MoveError::EmptyAddress { address })
                    }
                } else {
                    Err(MoveError::IllegalAddress { address })
                }
            }

            CardAddress::Foundation(s) => {
                if self.can_move_off_foundations {
                    if let Some(foundation) = self.foundations.get_mut(usize::from(s)) {
                        if !foundation.rank == 0 {
                            let card = *foundation;
                            *foundation = Card {
                                suit: card.suit,
                                rank: card.rank - 1,
                            };
                            self.floating = Some(card);
                            Ok(())
                        } else {
                            Err(MoveError::EmptyAddress { address })
                        }
                    } else {
                        Err(MoveError::IllegalAddress { address })
                    }
                } else {
                    Err(MoveError::CannotMoveOffFoundation { foundation: s })
                }
            }

            CardAddress::FreeCell(i) => {
                if let Some(free_cell) = self.free_cells.get_mut(i) {
                    if let Some(card) = free_cell.clone() {
                        *free_cell = if card.rank > 1 {
                            Some(Card {
                                suit: card.suit,
                                rank: card.rank - 1,
                            })
                        } else {
                            None
                        };
                        self.floating = Some(card);
                        Ok(())
                    } else {
                        Err(MoveError::EmptyAddress { address })
                    }
                } else {
                    Err(MoveError::IllegalAddress { address })
                }
            }
        }
    }

    // try to pick up a stack of cards from a position
    pub fn pick_up_stack(&mut self, address: usize, number_of_cards: usize) -> Result<()> {
        if self.floating != None || self.floating_stack != None {
            return Err(MoveError::AlreadyHoldingCards);
        }
        let max_possible_stack_size = self.max_stack_size();
        match number_of_cards {
            0 => Err(MoveError::ZeroCardStack),
            1 => self.pick_up_card(CardAddress::Column(address)),
            _ => {
                if let Some(column) = &mut self.columns.get_mut(address) {
                    if number_of_cards <= column.len() {
                        if number_of_cards <= max_possible_stack_size {
                            let it = column.iter().rev();
                            for pair in it.clone().take(number_of_cards - 1).zip(it.skip(1)) {
                                if !pair.0.stacks_on(pair.1) {
                                    return Err(MoveError::ImpossibleStack {
                                        cards: number_of_cards,
                                        column: address,
                                    });
                                }
                            }
                            let floating_stack = column.split_off(column.len() - number_of_cards);
                            self.floating_stack = Some(floating_stack);
                            Ok(())
                        } else {
                            Err(MoveError::TooManyCardsStack {
                                cards: number_of_cards,
                                column: address,
                            })
                        }
                    } else {
                        Err(MoveError::TooManyCardsStack {
                            cards: number_of_cards,
                            column: address,
                        })
                    }
                } else {
                    Err(MoveError::IllegalAddress {
                        address: CardAddress::Column(address),
                    })
                }
            }
        }
    }

    // place the held card at a position
    pub fn place(&mut self, address: CardAddress) -> Result<()> {
        match address {
            CardAddress::Column(i) => {
                if let Some(column) = &mut self.columns.get_mut(i) {
                    if let Some(card) = self.floating {
                        if column.is_empty() || card.stacks_on(column.last().unwrap()) {
                            column.push(card);
                            self.floating = None;
                            Ok(())
                        } else {
                            Err(MoveError::CannotPlace { to: address })
                        }
                    } else if let Some(cards) = &mut self.floating_stack {
                        if column.is_empty()
                            || cards.first().unwrap().stacks_on(column.last().unwrap())
                        {
                            column.append(cards);
                            self.floating_stack = None;
                            Ok(())
                        } else {
                            dbg!(
                                &cards,
                                &column,
                                &cards.first().unwrap().stacks_on(column.last().unwrap())
                            );
                            Err(MoveError::CannotPlace { to: address })
                        }
                    } else {
                        Err(MoveError::NoCardsHeld)
                    }
                } else {
                    Err(MoveError::IllegalAddress { address })
                }
            }

            CardAddress::Foundation(s) => {
                if let Some(foundation) = self.foundations.get_mut(usize::from(s)) {
                    if let Some(card) = self.floating {
                        if card.fits_on_foundation(foundation) {
                            *foundation = card;
                            self.floating = None;
                            Ok(())
                        } else {
                            Err(MoveError::CannotPlace {
                                to: CardAddress::Foundation(s),
                            })
                        }
                    } else {
                        Err(MoveError::NoCardsHeld)
                    }
                } else {
                    Err(MoveError::IllegalAddress { address })
                }
            }

            CardAddress::FreeCell(i) => {
                if let Some(free_cell) = self.free_cells.get_mut(i) {
                    if *free_cell == None {
                        if let Some(card) = self.floating {
                            *free_cell = Some(card);
                            self.floating = None;
                            Ok(())
                        } else {
                            Err(MoveError::CannotPlaceStack {
                                to: CardAddress::FreeCell(i),
                            })
                        }
                    } else {
                        Err(MoveError::CannotPlace { to: address })
                    }
                } else {
                    Err(MoveError::IllegalAddress { address })
                }
            }
        }
    }

    // get a look at the state of the board
    fn view(&self) -> GameView {
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

    // move all the cards you can to the foundations. returns true if you moved any
    pub fn auto_move_to_foundations(&mut self) -> bool {
        if self.floating != None || self.floating_stack != None {
            return false;
        }
        let mut moved_something = false;
        'loop_til_you_do_nothing: loop {
            for (index, maybe_card) in self
                .columns
                .iter()
                .map(|c| match c.last() {
                    Some(&v) => Some(v.clone()),
                    None => None,
                })
                .enumerate()
                .collect::<Vec<(usize, Option<Card>)>>()
            {
                if let Some(card) = maybe_card {
                    if self.can_auto_move(card) {
                        self.pick_up_card(CardAddress::Column(index)).unwrap();
                        self.place(CardAddress::Foundation(card.suit)).unwrap();
                        moved_something = true;
                        continue 'loop_til_you_do_nothing;
                    }
                }
            }
            break;
        }
        moved_something
    }

    fn can_auto_move(&self, card: Card) -> bool {
        if self.foundations[usize::from(card.suit)].rank != card.rank - 1 {
            return false;
        }
        match card.suit.colour() {
            Colour::Red => {
                let clubs = self.foundations[usize::from(Suit::Clubs)].rank >= card.rank - 1
                    || self.can_auto_move(Card {
                        rank: card.rank - 1,
                        suit: Suit::Clubs,
                    });
                let spades = self.foundations[usize::from(Suit::Spades)].rank >= card.rank - 1
                    || self.can_auto_move(Card {
                        rank: card.rank - 1,
                        suit: Suit::Spades,
                    });
                clubs && spades
            }
            Colour::Black => {
                let diamonds = self.foundations[usize::from(Suit::Diamonds)].rank >= card.rank - 1
                    || self.can_auto_move(Card {
                        rank: card.rank - 1,
                        suit: Suit::Diamonds,
                    });
                let hearts = self.foundations[usize::from(Suit::Hearts)].rank >= card.rank - 1
                    || self.can_auto_move(Card {
                        rank: card.rank - 1,
                        suit: Suit::Hearts,
                    });
                hearts && diamonds
            }
        }
    }
}

#[test]
fn test_moves() {
    // have to test
    // column moves work when they should and don't when they shouldn't
    // single card moves work when they should and don't when they shouldn't
    // can't pick up more than one thing at once
    // gets work okay
    let mut spread = Game::empty();
    spread.columns = vec![
        vec![
            Card {
                rank: 6,
                suit: Suit::Hearts,
            },
            Card {
                rank: 5,
                suit: Suit::Spades,
            },
            Card {
                rank: 4,
                suit: Suit::Hearts,
            },
            Card {
                rank: 3,
                suit: Suit::Spades,
            },
            Card {
                rank: 2,
                suit: Suit::Hearts,
            },
            Card {
                rank: 1,
                suit: Suit::Spades,
            },
        ],
        vec![
            Card {
                rank: 7,
                suit: Suit::Clubs,
            },
            Card {
                rank: 6,
                suit: Suit::Diamonds,
            },
            Card {
                rank: 5,
                suit: Suit::Clubs,
            },
        ],
        Vec::new(),
        vec![
            Card {
                rank: 7,
                suit: Suit::Hearts,
            },
            Card {
                rank: 6,
                suit: Suit::Diamonds,
            },
            Card {
                rank: 5,
                suit: Suit::Clubs,
            },
        ],
    ];

    // move 1-4 onto second column
    assert_eq!(
        spread.get(CardAddress::Column(0)),
        Ok(Card {
            rank: 1,
            suit: Suit::Spades
        })
    );
    assert_eq!(
        spread.pick_up_stack(4, 3),
        Err(MoveError::IllegalAddress {
            address: CardAddress::Column(4)
        }),
    );
    assert_eq!(
        spread.pick_up_stack(1, 4),
        Err(MoveError::TooManyCardsStack {
            column: 1,
            cards: 4,
        }),
    );
    assert_eq!(
        spread.pick_up_stack(3, 3),
        Err(MoveError::ImpossibleStack {
            column: 3,
            cards: 3,
        }),
    );
    assert_eq!(spread.pick_up_stack(0, 4), Ok(()));
    assert_eq!(
        spread.pick_up_stack(0, 2),
        Err(MoveError::AlreadyHoldingCards)
    );
    assert_eq!(
        spread.pick_up_card(CardAddress::Column(0)),
        Err(MoveError::AlreadyHoldingCards)
    );
    assert_eq!(
        spread.get(CardAddress::Column(0)),
        Ok(Card {
            rank: 5,
            suit: Suit::Spades
        })
    );
    assert_eq!(
        spread.place(CardAddress::FreeCell(0)),
        Err(MoveError::CannotPlaceStack {
            to: CardAddress::FreeCell(0)
        })
    );
    assert_eq!(
        spread.place(CardAddress::Column(4)),
        Err(MoveError::IllegalAddress {
            address: CardAddress::Column(4)
        })
    );
    assert_eq!(spread.place(CardAddress::Column(1)), Ok(()));
    assert_eq!(
        spread.get(CardAddress::Column(1)),
        Ok(Card {
            rank: 1,
            suit: Suit::Spades
        })
    );
    assert_eq!(
        spread.pick_up_stack(1, 6),
        Err(MoveError::TooManyCardsStack {
            cards: 6,
            column: 1
        })
    );
    assert_eq!(spread.pick_up_stack(1, 5), Ok(()));
    assert_eq!(spread.place(CardAddress::Column(1)), Ok(()));

    // move ace onto foundation
    assert_eq!(spread.pick_up_card(CardAddress::Column(1)), Ok(()));
    assert_eq!(
        spread.place(CardAddress::Column(0)),
        Err(MoveError::CannotPlace {
            to: CardAddress::Column(0)
        })
    );
    assert_eq!(
        spread.place(CardAddress::Foundation(Suit::Hearts)),
        Err(MoveError::CannotPlace {
            to: CardAddress::Foundation(Suit::Hearts)
        })
    );
    assert_eq!(spread.place(CardAddress::Foundation(Suit::Spades)), Ok(()));
    assert_eq!(
        spread.get(CardAddress::Foundation(Suit::Spades)),
        Ok(Card {
            rank: 1,
            suit: Suit::Spades
        })
    );

    // manually move cards up to 5 from 2nd column back to first; moved 5 from first to new column
    assert_eq!(spread.pick_up_card(CardAddress::Column(1)), Ok(()));
    assert_eq!(spread.place(CardAddress::FreeCell(0)), Ok(()));
    assert_eq!(spread.pick_up_card(CardAddress::Column(1)), Ok(()));
    assert_eq!(
        spread.place(CardAddress::FreeCell(0)),
        Err(MoveError::CannotPlace {
            to: CardAddress::FreeCell(0)
        })
    );
    assert_eq!(spread.place(CardAddress::FreeCell(1)), Ok(()));
    assert_eq!(spread.pick_up_card(CardAddress::Column(1)), Ok(()));
    assert_eq!(spread.place(CardAddress::FreeCell(2)), Ok(()));
    assert_eq!(spread.pick_up_card(CardAddress::Column(0)), Ok(()));
    assert_eq!(spread.place(CardAddress::FreeCell(3)), Ok(()));
    assert_eq!(spread.pick_up_card(CardAddress::Column(1)), Ok(()));
    assert_eq!(spread.place(CardAddress::Column(0)), Ok(()));
    assert_eq!(spread.pick_up_card(CardAddress::FreeCell(2)), Ok(()));
    assert_eq!(spread.place(CardAddress::Column(0)), Ok(()));
    assert_eq!(spread.pick_up_card(CardAddress::FreeCell(1)), Ok(()));
    assert_eq!(spread.place(CardAddress::Column(0)), Ok(()));
    assert_eq!(spread.pick_up_card(CardAddress::FreeCell(0)), Ok(()));
    assert_eq!(spread.place(CardAddress::Column(0)), Ok(()));
    assert_eq!(spread.pick_up_card(CardAddress::FreeCell(2)), Ok(()));
    assert_eq!(spread.place(CardAddress::Column(2)), Ok(()));
}

#[test]
fn auto_move() {
    let mut game = Game::empty();
    game.columns.push(
        (1..=5)
            .rev()
            .map(|n| Card {
                rank: n,
                suit: Suit::Spades,
            })
            .collect::<Vec<Card>>(),
    );
    game.columns.push(vec![
        Card {
            rank: 3,
            suit: Suit::Clubs,
        },
        Card {
            rank: 4,
            suit: Suit::Clubs,
        },
        Card {
            rank: 2,
            suit: Suit::Clubs,
        },
        Card {
            rank: 1,
            suit: Suit::Clubs,
        },
    ]);
    game.columns.push(vec![
        Card {
            rank: 3,
            suit: Suit::Diamonds,
        },
        Card {
            rank: 2,
            suit: Suit::Diamonds,
        },
        Card {
            rank: 1,
            suit: Suit::Diamonds,
        },
        Card {
            rank: 2,
            suit: Suit::Hearts,
        },
        Card {
            rank: 1,
            suit: Suit::Hearts,
        },
    ]);
    game.auto_move_to_foundations();
    assert_eq!(
        game.foundations,
        vec![
            Card {
                rank: 2,
                suit: Suit::Clubs,
            },
            Card {
                rank: 3,
                suit: Suit::Diamonds,
            },
            Card {
                rank: 2,
                suit: Suit::Hearts,
            },
            Card {
                rank: 4,
                suit: Suit::Spades,
            },
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
