use super::*;

#[test]
fn test_moves() {
    let mut spread = _game_from_columns(vec![
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
    ]);

    // move 1-4 onto second column
    assert_eq!(
        spread.view().columns[0].last().unwrap(),
        &Card::new(1, Suit::Spades)
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
        spread.view().columns[0].last().unwrap(),
        &Card::new(5, Suit::Spades)
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
        spread.view().columns[1].last().unwrap(),
        &Card::new(1, Suit::Spades)
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
        spread.view().foundations[usize::from(Suit::Spades)],
        Card::new(1, Suit::Spades)
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
    let mut game = _game_from_columns(vec![
        (1..=5)
            .rev()
            .map(|n| Card::new(n, Suit::Spades))
            .collect::<Vec<Card>>(),
        vec![
            Card::new(3, Suit::Clubs),
            Card::new(4, Suit::Clubs),
            Card::new(2, Suit::Clubs),
            Card::new(1, Suit::Clubs),
        ],
        vec![
            Card::new(3, Suit::Diamonds),
            Card::new(2, Suit::Diamonds),
            Card::new(1, Suit::Diamonds),
            Card::new(2, Suit::Hearts),
            Card::new(1, Suit::Hearts),
        ],
    ]);
    while let Some(new_state) = game.auto_move_to_foundations() {
        game = new_state;
    }
    assert_eq!(
        game.view().foundations,
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
    assert!(!game.view().is_won());
    game.state.foundations = vec![
        Card::new(13, Suit::Clubs),
        Card::new(13, Suit::Diamonds),
        Card::new(13, Suit::Hearts),
        Card::new(12, Suit::Spades),
    ];
    game = game.state.into(); // update view
    assert!(!game.view().is_won());
    game.state.foundations[3] = Card::new(13, Suit::Spades);
    game = game.state.into(); // update view
    assert!(game.view().is_won());
}
