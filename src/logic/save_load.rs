use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Result;

use super::game::inspect::*;
use super::*;

// load game
pub fn load(filename: &Path) -> Result<(u64, Game, GameUndoStack)> {
    let save = fs::read_to_string(filename)?;
    let (seed, state, undo): (u64, StateContainer, GameUndoStack) = serde_json::from_str(&save)?;
    Ok((seed, game_from_state(state), undo))
}

// save game
pub fn save(
    seed: u64,
    game: &Game,
    undo: &GameUndoStack,
    dir: PathBuf,
    name: &str,
) -> Result<PathBuf> {
    let save = serde_json::to_string(&(seed, game_get_state(game), undo))?;
    for n in 0.. {
        let mut filename = dir.clone();
        filename.push(name.to_string() + &n.to_string());
        if !filename.exists() {
            let mut file = fs::File::create(filename.clone())?;
            file.write_all(save.as_bytes())?;
            return Ok(filename);
        }
    }
    unreachable!();
}
