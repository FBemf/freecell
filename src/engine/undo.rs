use std::fmt;

use super::game::*;

#[derive(Debug)]
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
        if let Some((_, last_state)) = self.history.last() {
            // don't push no-ops
            if last_state != &old_state {
                self.history.push((false, old_state));
            }
        } else {
            self.history.push((false, old_state));
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
            if !sneak && previous_state.view().floating.is_none() {
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
            if undone_state.view().floating.is_none() {
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
