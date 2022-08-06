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
        console::time_with_label("discover_metamoves");
        let max_discover_metamoves_depth =
            (5_000_000f64.ln() / (puzzle.turns.len() as f64).ln()) as usize;
        console::log_1(&max_discover_metamoves_depth.into());
        let turn_num_affected_pieces =
            MetaMove::new_infer_face_map(&puzzle, vec![0]).num_affected_pieces;
        let metamoves = discover_metamoves(
            &puzzle,
            |mm| mm.num_affected_pieces < turn_num_affected_pieces,
            // |mm| true,
            max_discover_metamoves_depth,
        );
        console::time_end_with_label("discover_metamoves");
        console::log_1(&metamoves.len().into());
        // console::time_with_label("combine_metamoves");
        // let metamoves = combine_metamoves(&puzzle, &metamoves, 2);
        // console::time_end_with_label("combine_metamoves");
        console::log_1(&metamoves.len().into());
        // let metamoves: Vec<MetaMove> = metamoves.into_iter().take(4).collect();
        console::log_1(&"solve".into());

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
            depth: (1_000_000f64.ln() / (metamoves.len() as f64).ln()) as usize,
            metamoves,
            puzzle,
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
        console::log_1(&self.depth.into());
        if !self.buffered_turns.is_empty() {
            let next_turn = self.buffered_turns.pop_front().unwrap();
            self.state = self
                .puzzle
                .get_derived_state_turn_index(&self.state, next_turn);
            return Some(next_turn);
        }

        let options = self.metamoves.clone();

        let best_metamove = find_best_metamove(&self.puzzle, &self.state, &options, self.depth);
        // let mut best_metamove = find_best_metamove(&self.puzzle, &self.state, &options, self.depth);
        // if best_metamove.turns.is_empty() {
        //     best_metamove = find_best_metamove(&self.puzzle, &self.state, &options, self.depth + 1);
        // }

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
