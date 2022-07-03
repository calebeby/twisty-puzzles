use std::rc::Rc;

use crate::twisty_puzzle::{PuzzleState, TwistyPuzzle};
mod full_search_solve;
mod lookahead;
mod metamove_solver;
mod neural_network_one_move;
mod simple_one_move;
pub use full_search_solve::{FullSearchSolver, FullSearchSolverOpts};
pub use lookahead::{LookaheadSolver, LookaheadSolverOpts};
pub use metamove_solver::MetaMoveSolver;
pub use neural_network_one_move::NNOneMoveSolver;
pub use simple_one_move::OneMoveSolver;

pub struct Solver<T: ScrambleSolver> {
    opts: T::Opts,
    puzzle: Rc<TwistyPuzzle>,
}

impl<T: ScrambleSolver> Solver<T> {
    pub fn new(puzzle: Rc<TwistyPuzzle>, opts: T::Opts) -> Self {
        Self { opts, puzzle }
    }
    pub fn solve(&self, initial_state: PuzzleState) -> T {
        T::new(self.puzzle.clone(), initial_state, self.opts.clone())
    }
}

pub trait ScrambleSolver: Iterator<Item = usize> {
    type Opts: Clone;
    fn new(puzzle: Rc<TwistyPuzzle>, initial_state: PuzzleState, opts: Self::Opts) -> Self;
    fn get_state(&self) -> &PuzzleState;
}
