use std::fmt;

use serde::{Deserialize, Serialize};

use super::game::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GameUndoStack {
    history: Vec<(bool, Game)>,
    undo_history: Vec<Game>,
}

impl GameUndoStack {
    pub fn new() -> Self {
        GameUndoStack {
            history: Vec::new(),
            undo_history: Vec::new(),
        }
    }

    pub fn update(&mut self, old_state: Game, new_state: Game) -> Game {
        // no no-ops
        if old_state == new_state {
            return new_state;
        }

        // if we're manually undoing a move, don't destroy the redo stack.
        // just truncate the undo stack back to the point we're undoing it to
        let mut new_len = None;
        for (n, (_, prev_state)) in self.history.iter().enumerate().rev().take(2) {
            if prev_state == &new_state {
                new_len = Some(n);
            }
        }
        if let Some(n) = new_len {
            let mut truncated: Vec<Game> = self
                .history
                .split_off(n)
                .into_iter()
                .map(|(_, state)| state)
                .rev()
                .collect();
            self.undo_history.append(&mut truncated);
            return self.undo_history.pop().unwrap();
        }

        if !old_state.has_floating() {
            self.history.push((false, old_state));
        }

        // if we're manually re-doing a move, pop it off the redo stack.
        if let Some(undone_state) = self.undo_history.last() {
            if &new_state == undone_state {
                self.undo_history.pop();
            } else if !new_state.has_floating() {
                self.undo_history = Vec::new();
            }
        }

        new_state
    }

    // sneak updates will, upon being undone, immediately trigger another undo
    pub fn sneak_update(&mut self, old_state: Game, new_state: Game) -> Game {
        if let Some((_, last_state)) = self.history.last() {
            // don't push no-ops
            if last_state != &old_state {
                self.history.push((true, old_state));
            }
        } else {
            self.history.push((true, old_state));
        }
        if let Some(undone_state) = self.undo_history.last() {
            if &new_state == undone_state {
                self.undo_history.pop();
            } else {
                self.undo_history = Vec::new();
            }
        }
        new_state
    }

    pub fn undo(&mut self, state: Game) -> Game {
        if !state.has_floating() {
            self.undo_history.push(state);
        }
        // undo all sneak updates and floating states and then one more
        while let Some((sneak, previous_state)) = self.history.pop() {
            if sneak {
                self.undo_history.push(previous_state);
            } else {
                return previous_state;
            }
        }
        // if we reach here, we undid the whole undo stack, and we have to redo the last thing
        self.undo_history.pop().unwrap()
    }

    pub fn redo(&mut self, state: Game) -> Game {
        if let Some(undone_state) = self.undo_history.pop() {
            if !state.has_floating() {
                self.history.push((false, state));
            }
            undone_state
        } else {
            state
        }
    }
}

impl fmt::Display for GameUndoStack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UNDO:")?;
        for (sneak, state) in self.history.iter() {
            if *sneak {
                write!(f, "\n  sneak:")?;
            }
            write!(f, "\n{}", state.view())?;
        }
        write!(f, "\nREDO:")?;
        for state in self.undo_history.iter() {
            write!(f, "\n{}", state.view())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::super::card::*;
    use super::super::game::inspect::*;
    use super::*;

    #[test]
    fn undo_redo() {
        let mut game = game_from_columns(vec![
            vec![Card::new(2, Suit::Diamonds), Card::new(1, Suit::Clubs)],
            Vec::new(),
        ]);
        let mut undo_stack = GameUndoStack::new();

        // basic undo
        let game_state_1 = game.clone();
        game = undo_stack.update(
            game.clone(),
            game.pick_up_card(CardAddress::Column(0)).unwrap(),
        );
        game = undo_stack.update(game.clone(), game.place(CardAddress::Column(1)).unwrap());
        let game_state_2 = game.clone();
        game = undo_stack.undo(game);
        assert_eq!(game, game_state_1);

        // automatic redo
        game = undo_stack.redo(game);
        assert_eq!(game, game_state_2);
    }

    #[test]
    fn manual_undo() {
        let mut game = game_from_columns(vec![
            vec![Card::new(2, Suit::Clubs), Card::new(1, Suit::Diamonds)],
            Vec::new(),
            Vec::new(),
        ]);
        let mut undo_stack = GameUndoStack::new();

        let game_state_1 = game.clone();
        game = undo_stack.update(
            game.clone(),
            game.pick_up_card(CardAddress::Column(0)).unwrap(),
        );
        game = undo_stack.update(game.clone(), game.place(CardAddress::Column(1)).unwrap());
        game = undo_stack.update(
            game.clone(),
            game.pick_up_card(CardAddress::Column(0)).unwrap(),
        );
        game = undo_stack.update(game.clone(), game.place(CardAddress::Column(2)).unwrap());
        let game_state_2 = game.clone();

        // make sure manual undos don't mess up the redo stack
        game = undo_stack.undo(game);
        game = undo_stack.update(
            game.clone(),
            game.pick_up_card(CardAddress::Column(1)).unwrap(),
        );
        eprintln!("{}\n---\n{}\n---", game.view(), undo_stack);
        game = undo_stack.update(game.clone(), game.place(CardAddress::Column(0)).unwrap());
        assert_eq!(game, game_state_1);
        eprintln!("{}\n---\n{}\n---", game.view(), undo_stack);
        game = undo_stack.redo(game);
        eprintln!("{}\n---\n{}\n---", game.view(), undo_stack);
        assert_ne!(game, game_state_1);
        assert_ne!(game, game_state_2);
        game = undo_stack.redo(game);
        assert_eq!(game, game_state_2);
    }

    #[test]
    fn manual_redo() {
        let mut game = game_from_columns(vec![
            vec![Card::new(1, Suit::Clubs), Card::new(2, Suit::Diamonds)],
            Vec::new(),
        ]);
        let mut undo_stack = GameUndoStack::new();

        let game_state_1 = game.clone();
        game = undo_stack.update(
            game.clone(),
            game.pick_up_card(CardAddress::Column(0)).unwrap(),
        );
        game = undo_stack.update(game.clone(), game.place(CardAddress::Column(1)).unwrap());
        game = undo_stack.update(
            game.clone(),
            game.pick_up_card(CardAddress::Column(0)).unwrap(),
        );
        game = undo_stack.update(game.clone(), game.place(CardAddress::Column(1)).unwrap());
        let game_state_2 = game.clone();

        game = undo_stack.undo(game);
        game = undo_stack.undo(game);
        assert_eq!(game, game_state_1);

        // manual redo
        game = undo_stack.update(
            game.clone(),
            game.pick_up_card(CardAddress::Column(0)).unwrap(),
        );
        game = undo_stack.update(game.clone(), game.place(CardAddress::Column(1)).unwrap());

        // auto redo
        game = undo_stack.redo(game);
        assert_eq!(game, game_state_2);
    }

    #[test]
    fn nop_skipping() {
        let mut game = game_from_columns(vec![
            vec![Card::new(1, Suit::Clubs), Card::new(2, Suit::Diamonds)],
            Vec::new(),
        ]);
        let mut undo_stack = GameUndoStack::new();

        game = undo_stack.update(
            game.clone(),
            game.pick_up_card(CardAddress::Column(0)).unwrap(),
        );
        game = undo_stack.update(game.clone(), game.place(CardAddress::Column(1)).unwrap());
        let game_state_1 = game.clone();
        game = undo_stack.update(
            game.clone(),
            game.pick_up_card(CardAddress::Column(0)).unwrap(),
        );
        game = undo_stack.update(game.clone(), game.place(CardAddress::Column(1)).unwrap());
        let game_state_2 = game.clone();
        game = undo_stack.undo(game);

        // nop skipping during update
        game = undo_stack.update(
            game.clone(),
            game.pick_up_card(CardAddress::Column(0)).unwrap(),
        );
        game = undo_stack.update(game.clone(), game.place(CardAddress::Column(0)).unwrap());
        assert_eq!(game, game_state_1);

        game = undo_stack.redo(game);
        assert_eq!(game, game_state_2);
    }

    #[test]
    fn sneak_skipping() {
        let mut game = game_from_columns(vec![
            vec![Card::new(1, Suit::Clubs), Card::new(2, Suit::Diamonds)],
            Vec::new(),
        ]);
        let mut undo_stack = GameUndoStack::new();

        let game_state_1 = game.clone();
        game = undo_stack.update(
            game.clone(),
            game.pick_up_card(CardAddress::Column(0)).unwrap(),
        );
        game = undo_stack.update(game.clone(), game.place(CardAddress::Column(1)).unwrap());
        game = undo_stack.sneak_update(game.clone(), game.auto_move_to_foundations().unwrap());

        // sneak skipping during undo
        game = undo_stack.undo(game);
        assert_eq!(game, game_state_1);
    }

    #[test]
    fn no_ops() {
        let mut game = game_from_columns(vec![
            vec![Card::new(2, Suit::Clubs), Card::new(1, Suit::Diamonds)],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ]);
        let mut undo_stack = GameUndoStack::new();

        let game_state_1 = game.clone();
        game = undo_stack.update(
            game.clone(),
            game.pick_up_card(CardAddress::Column(0)).unwrap(),
        );
        game = undo_stack.update(game.clone(), game.place(CardAddress::Column(1)).unwrap());
        game = undo_stack.update(
            game.clone(),
            game.pick_up_card(CardAddress::Column(0)).unwrap(),
        );
        game = undo_stack.update(game.clone(), game.place(CardAddress::Column(2)).unwrap());
        let game_state_2 = game.clone();

        // make sure no-ops don't destroy redo stack
        game = undo_stack.undo(game);
        game = undo_stack.undo(game);
        assert_eq!(game, game_state_1);
        game = undo_stack.update(
            game.clone(),
            game.pick_up_card(CardAddress::Column(0)).unwrap(),
        );
        game = undo_stack.update(game.clone(), game.place(CardAddress::Column(0)).unwrap());
        game = undo_stack.redo(game);
        game = undo_stack.redo(game);
        assert_eq!(game, game_state_2);

        // make sure actual ops do
        game = undo_stack.undo(game);
        game = undo_stack.undo(game);
        assert_eq!(game, game_state_1);
        game = undo_stack.update(
            game.clone(),
            game.pick_up_card(CardAddress::Column(0)).unwrap(),
        );
        game = undo_stack.update(game.clone(), game.place(CardAddress::Column(3)).unwrap());
        assert_ne!(game, game_state_2);
        let game_state_3 = game.clone();
        game = undo_stack.redo(game);
        assert_eq!(game, game_state_3);
    }
}
