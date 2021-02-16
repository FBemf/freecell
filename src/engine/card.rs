use std::convert::TryFrom;
use std::fmt;

pub type CardColumn = Vec<Card>;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Card {
    pub rank: u8,
    pub suit: Suit,
}

impl Card {
    pub fn stacks_on(&self, base: &Card) -> bool {
        self.suit.colour() != base.suit.colour() && base.rank == self.rank + 1
    }

    pub fn fits_on_foundation(&self, base: &Card) -> bool {
        self.suit == base.suit && self.rank == base.rank + 1
    }

    pub fn new(rank: u8, suit: Suit) -> Self {
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
        if self.rank == 0 {
            write!(f, "   ")
        } else {
            write!(f, "{:3}", rank + suit)
        }
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
pub enum Colour {
    Red,
    Black,
}

impl Suit {
    pub fn colour(&self) -> Colour {
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
