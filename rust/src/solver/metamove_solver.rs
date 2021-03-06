use super::{
    metamoves::{discover_metamoves, MetaMove},
    ScrambleSolver,
};
use crate::{
    traverse_combinations::{traverse_combinations, TraverseResult},
    twisty_puzzle::{PuzzleState, TwistyPuzzle},
};
use std::{collections::VecDeque, rc::Rc};
use wasm_bindgen::throw_str;
use web_sys::console;

pub struct MetaMoveSolver {
    puzzle: Rc<TwistyPuzzle>,
    state: PuzzleState,
    depth: usize,
    metamoves: Vec<MetaMove>,
    buffered_turns: VecDeque<usize>,
}

impl ScrambleSolver for MetaMoveSolver {
    type Opts = ();

    fn new(puzzle: Rc<TwistyPuzzle>, initial_state: PuzzleState, _opts: Self::Opts) -> Self {
        let metamoves = discover_metamoves(&puzzle, |mm| mm.num_affected_pieces <= 12, 4);
        let plugin: Vec<_> = metamoves.into_iter().take(2).collect();
        let metamoves = combine_metamoves(&puzzle, &plugin, 4);
        let metamoves: Vec<MetaMove> = metamoves.into_iter().take(4).collect();

        if metamoves.is_empty() {
            throw_str("no metamoves");
        }

        console::log_1(&format!("num metamoves: {}", metamoves.len()).into());
        console::log_1(
            &format!(
                "best metamove: {} turns affecting {} pieces",
                metamoves[0].turns.len(),
                metamoves[0].num_affected_pieces
            )
            .into(),
        );

        Self {
            metamoves,
            puzzle,
            depth: 3,
            state: initial_state,
            buffered_turns: VecDeque::new(),
        }
    }

    fn get_state(&self) -> &PuzzleState {
        &self.state
    }
}

impl Iterator for MetaMoveSolver {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.buffered_turns.is_empty() {
            let next_turn = self.buffered_turns.pop_front().unwrap();
            self.state = self
                .puzzle
                .get_derived_state_turn_index(&self.state, next_turn);
            return Some(next_turn);
        }

        let options: Vec<MetaMove> = self
            .metamoves
            .iter()
            .cloned()
            .chain(
                self.puzzle
                    .turns
                    .iter()
                    .enumerate()
                    .map(|(turn_index, turn)| {
                        MetaMove::new(&self.puzzle, vec![turn_index], turn.face_map.clone())
                    }),
            )
            .collect();

        let mut best_metamove = find_best_metamove(&self.puzzle, &self.state, &options, self.depth);
        if best_metamove.turns.is_empty() {
            best_metamove = find_best_metamove(&self.puzzle, &self.state, &options, self.depth + 1);
        }

        console::log_1(&format!("applying metamoves {} turns", best_metamove.turns.len()).into());
        let &first_turn = best_metamove.turns.get(0)?;
        self.state = self
            .puzzle
            .get_derived_state_turn_index(&self.state, first_turn);
        if best_metamove.turns.len() > 1 {
            self.buffered_turns.clear();
            for turn in &best_metamove.turns[1..] {
                self.buffered_turns.push_back(*turn)
            }
        }
        Some(first_turn)
    }
}

fn find_best_metamove(
    puzzle: &TwistyPuzzle,
    state: &PuzzleState,
    metamoves: &[MetaMove],
    depth: usize,
) -> MetaMove {
    let mut best_metamove = MetaMove::empty(puzzle);
    let mut best_score = puzzle.get_num_solved_pieces(state);

    traverse_combinations(
        metamoves,
        depth,
        MetaMove::empty(puzzle),
        &|previous_metamove: &MetaMove, new_metamove: &MetaMove| {
            previous_metamove.apply(puzzle, new_metamove)
        },
        &mut |mm| {
            let next_state = puzzle.get_derived_state(state, &mm.face_map);
            let next_state_score = puzzle.get_num_solved_pieces(&next_state);
            if next_state_score > best_score {
                best_metamove = mm.clone();
                best_score = next_state_score;
            }
            if next_state_score == puzzle.get_num_pieces() {
                return TraverseResult::Break;
            }
            TraverseResult::Continue
        },
    );

    best_metamove
}

fn combine_metamoves(puzzle: &TwistyPuzzle, metamoves: &[MetaMove], depth: usize) -> Vec<MetaMove> {
    let mut combined_metamoves = vec![];

    traverse_combinations(
        metamoves,
        depth,
        MetaMove::empty(puzzle),
        &|previous_metamove: &MetaMove, new_metamove: &MetaMove| {
            previous_metamove.apply(puzzle, new_metamove)
        },
        &mut |mm| {
            if mm.num_affected_pieces != 0 && mm.num_affected_pieces <= 8 {
                combined_metamoves.push(mm.clone());
            }
            TraverseResult::Continue
        },
    );

    combined_metamoves.sort();
    combined_metamoves
}
