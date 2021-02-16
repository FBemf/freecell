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

        // no no-ops part two: if the new state has been previously visited,
        // and all the intervening states are states with floating cards,
        // jump back to the original instance of the state instead of adding a new one
        if old_state.has_floating() {
            let mut new_length = None;
            for (n, (_, prev_state)) in self.history.iter().enumerate().rev() {
                if !prev_state.has_floating() {
                    if prev_state == &new_state {
                        new_length = Some(n);
                    }
                    break;
                }
            }
            if let Some(l) = new_length {
                self.history.truncate(l);
                return new_state;
            }
        }

        self.history.push((false, old_state));

        // if we're manually re-doing a move, pop it off the redo stack
        if let Some(undone_state) = self.undo_history.last() {
            if &new_state == undone_state {
                self.undo_history.pop();
            } else {
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
        self.undo_history.push(state);
        while let Some((sneak, previous_state)) = self.history.pop() {
            if !sneak && !previous_state.has_floating() {
                return previous_state;
            } else {
                self.undo_history.push(previous_state);
            }
        }
        self.undo_history.pop().unwrap()
    }

    pub fn redo(&mut self, state: Game) -> Game {
        self.history.push((false, state));
        while let Some(undone_state) = self.undo_history.pop() {
            if !undone_state.has_floating() {
                return undone_state;
            } else {
                self.history.push((false, undone_state));
            }
        }
        self.history.pop().unwrap().1
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
    use super::super::game::*;
    use super::*;

    #[test]
    fn undo_test() {
        let mut game = _game_from_columns(vec![
            vec![
                Card::new(5, Suit::Clubs),
                Card::new(4, Suit::Diamonds),
                Card::new(3, Suit::Clubs),
                Card::new(2, Suit::Diamonds),
                Card::new(1, Suit::Clubs),
            ],
            Vec::new(),
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
        let undo_stack_state_1 = undo_stack.clone();
        game = undo_stack.undo(game);
        assert_eq!(game, game_state_1);

        // automatic redo
        game = undo_stack.redo(game);
        assert_eq!(game, game_state_2);

        // manual redo
        game = undo_stack.undo(game);
        game = undo_stack.update(
            game.clone(),
            game.pick_up_card(CardAddress::Column(0)).unwrap(),
        );
        game = undo_stack.update(game.clone(), game.place(CardAddress::Column(1)).unwrap());
        assert_eq!(game, game_state_2);
        assert_eq!(undo_stack, undo_stack_state_1);

        // nop skipping during update
        game = undo_stack.update(
            game.clone(),
            game.pick_up_card(CardAddress::Column(0)).unwrap(),
        );
        game = undo_stack.update(game.clone(), game.place(CardAddress::Column(0)).unwrap());
        assert_eq!(game, game_state_2);
        assert_eq!(undo_stack, undo_stack_state_1);

        // sneak skipping during undo
        game = undo_stack.update(
            game.clone(),
            game.pick_up_card(CardAddress::Column(0)).unwrap(),
        );
        game = undo_stack.update(game.clone(), game.place(CardAddress::Column(2)).unwrap());
        game = undo_stack.sneak_update(game.clone(), game.auto_move_to_foundations().unwrap());
        game = undo_stack.undo(game);
        assert_eq!(game, game_state_2);
    }
}
