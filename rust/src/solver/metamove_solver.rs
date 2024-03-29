use super::{
    metamoves::{combine_metamoves, discover_metamoves, MetaMove},
    ScrambleSolver,
};
use crate::{
    traverse_combinations::{traverse_combinations, TraverseResult},
    twisty_puzzle::{PuzzleState, TwistyPuzzle},
};
use std::{
    collections::{hash_map::Entry, HashMap, VecDeque},
    rc::Rc,
};
use wasm_bindgen::throw_str;

pub struct MetaMoveSolver {
    puzzle: Rc<TwistyPuzzle>,
    state: PuzzleState,
    phase: SolvePhase,
    depth: usize,
    metamoves: Vec<MetaMove>,
    buffered_turns: VecDeque<usize>,
}

#[derive(PartialEq, Eq)]
enum SolvePhase {
    Search,
    Metamoves,
}

macro_rules! console_log {
    ($($t:tt)*) => {
        #[cfg(target_arch = "wasm32")] {
            web_sys::console::log_1(&format!($($t)*).into());
        }
        #[cfg(not(target_arch = "wasm32"))] {
            println!($($t)*);
        }
    };
}

impl ScrambleSolver for MetaMoveSolver {
    type Opts = ();

    fn new(puzzle: Rc<TwistyPuzzle>, initial_state: PuzzleState, _opts: Self::Opts) -> Self {
        // let max_discover_metamoves_depth =
        //     (2_000_000f64.ln() / (puzzle.turns.len() as f64).ln()) as usize;
        // For now using a hardcoded tree depth,
        // but in the future might switch to dynamic depth based on puzzle complexity
        let max_discover_metamoves_depth = 5;
        // Count the number of pieces affected by an individual turn
        let turn_num_affected_pieces =
            MetaMove::new_infer_face_map(Rc::clone(&puzzle), vec![0]).num_affected_pieces;
        // Discover sets of moves that affect fewer pieces than an individual turn
        let metamoves = discover_metamoves(
            Rc::clone(&puzzle),
            |mm| mm.num_affected_pieces < turn_num_affected_pieces,
            max_discover_metamoves_depth,
        );

        console_log!("num metamoves: {}", metamoves.len());
        let best = metamoves.iter().min().unwrap();
        console_log!(
            "best metamove: {} turns affecting {} pieces",
            best.turns.len(),
            best.num_affected_pieces
        );

        // Smoosh together pairs of sets of moves
        let metamoves: Vec<_> = combine_metamoves(Rc::clone(&puzzle), |_mm| true, &metamoves, 2);
        console_log!("num metamoves: {}", metamoves.len());
        let best = metamoves.iter().min().unwrap();
        console_log!(
            "best metamove: {} turns affecting {} pieces",
            best.turns.len(),
            best.num_affected_pieces
        );

        // Take out metamoves that have the same effect as others (keep ones with fewest # moves)
        let metamoves = filter_duplicates(metamoves);

        let metamoves: Vec<_> = metamoves
            .into_iter()
            .flat_map(|mm| {
                // Repeat each metamove multiple times so that some of the face-cycles within the
                // metamove cancel out
                mm.discover_repeat_metamoves()
                    .into_iter()
                    .chain(std::iter::once(mm))
            })
            // Filter out metamoves based on how many pieces they affect
            .filter(|mm| mm.num_affected_pieces <= 3)
            .collect();

        console_log!("all mm {}", metamoves.len());

        let mut metamoves = filter_duplicates(metamoves);

        console_log!("reduced mm {}", metamoves.len());

        console_log!("num metamoves: {}", metamoves.len());
        let best = metamoves.iter().min().unwrap();
        console_log!(
            "best metamove: {} turns affecting {} pieces",
            best.turns.len(),
            best.num_affected_pieces
        );

        metamoves.sort();

        if metamoves.is_empty() {
            throw_str("no metamoves");
        }
        console_log!("done scanning");

        Self {
            // depth: (500_000f64.ln() / (metamoves.len() as f64).ln()) as usize,
            // For now, using a static depth, but in the future, consider doing a dynamic depth
            // based on the number of metamoves available at this point
            depth: 2,
            phase: SolvePhase::Search,
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
        if !self.buffered_turns.is_empty() {
            let next_turn = self.buffered_turns.pop_front().unwrap();
            self.state = self
                .puzzle
                .get_derived_state_turn_index(&self.state, next_turn);
            return Some(next_turn);
        }

        // First phase: do a shallow search to make it more solved
        if self.phase == SolvePhase::Search {
            let mut best_metamove = MetaMove::empty(Rc::clone(&self.puzzle));
            let mut best_score = self.puzzle.get_num_solved_pieces(&self.state);
            let individual_turns_metamoves: Vec<MetaMove> = self
                .puzzle
                .turns
                .iter()
                .enumerate()
                .map(|(turn_index, turn)| {
                    MetaMove::new(
                        Rc::clone(&self.puzzle),
                        vec![turn_index],
                        turn.face_map.clone(),
                    )
                })
                .collect();

            for depth in 4..=5 {
                traverse_combinations(
                    &individual_turns_metamoves,
                    depth,
                    MetaMove::empty(Rc::clone(&self.puzzle)),
                    |previous_metamove: &MetaMove, new_metamove: &MetaMove| {
                        previous_metamove.apply(new_metamove)
                    },
                    &mut |mm| {
                        let next_state = self.puzzle.get_derived_state(&self.state, &mm.face_map);
                        let next_state_score = self.puzzle.get_num_solved_pieces(&next_state);
                        if next_state_score > best_score
                            || (next_state_score == best_score
                                && mm.turns.len() < best_metamove.turns.len())
                        {
                            best_metamove = mm.clone();
                            best_score = next_state_score;
                        }
                        TraverseResult::Continue
                    },
                );

                if best_metamove.num_affected_pieces != 0 {
                    let &first_turn = best_metamove.turns.first()?;
                    self.state = self
                        .puzzle
                        .get_derived_state_turn_index(&self.state, first_turn);
                    if best_metamove.turns.len() > 1 {
                        self.buffered_turns.clear();
                        for turn in &best_metamove.turns[1..] {
                            self.buffered_turns.push_back(*turn)
                        }
                    }
                    return Some(first_turn);
                }
            }
            self.phase = SolvePhase::Metamoves;
        }

        // Second phase: Use metamoves to finish solving

        let options = self.metamoves.clone();

        let best_metamove =
            find_best_metamove(Rc::clone(&self.puzzle), &self.state, &options, self.depth);
        let &first_turn = best_metamove.turns.first()?;
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
    puzzle: Rc<TwistyPuzzle>,
    state: &PuzzleState,
    metamoves: &[MetaMove],
    depth: usize,
) -> MetaMove {
    let mut best_metamove = MetaMove::empty(Rc::clone(&puzzle));
    let mut best_score = puzzle.get_num_solved_pieces(state);

    traverse_combinations(
        metamoves,
        depth,
        MetaMove::empty(Rc::clone(&puzzle)),
        |previous_metamove: &MetaMove, new_metamove: &MetaMove| {
            previous_metamove.apply(new_metamove)
        },
        &mut |mm| {
            let next_state = puzzle.get_derived_state(state, &mm.face_map);
            let next_state_score = puzzle.get_num_solved_pieces(&next_state);
            if next_state_score > best_score {
                best_metamove = mm.clone();
                best_score = next_state_score;
                // Uncomment the following line to stop once we find _anything_ better,
                // not necessarily the best one
                // return TraverseResult::Break;
            }
            if next_state_score == puzzle.get_num_pieces() {
                return TraverseResult::Break;
            }
            TraverseResult::Continue
        },
    );

    best_metamove
}

fn filter_duplicates(metamoves: Vec<MetaMove>) -> Vec<MetaMove> {
    let mut metamoves_reduced = HashMap::new();
    for mm in metamoves {
        let entry = metamoves_reduced.entry(mm.face_map.clone());

        match entry {
            Entry::Vacant(entry) => {
                entry.insert(mm);
            }
            Entry::Occupied(mut entry) => {
                if entry.get().turns.len() > mm.turns.len() {
                    entry.insert(mm);
                }
            }
        }
    }

    metamoves_reduced.into_values().collect()
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    use super::*;
    use crate::puzzles;

    #[test]
    fn solve_rubiks_3x3() {
        let puzzle = Rc::new(puzzles::rubiks_cube_3x3());

        let mut rng = ChaCha8Rng::seed_from_u64(1);

        let mut sum = 0;
        let mut num_solves = 0;
        let num_scrambles = 50;
        // Baseline: 30/50, took 1m 57s to run:
        //
        // 3x3 solution length: 260 turns
        // avg 3x3 solution length: 384.6 turns, (30 / 50)
        for _ in 0..num_scrambles {
            let scrambled_state = puzzle.scramble(&puzzle.get_initial_state(), 20, &mut rng);
            let solution: Vec<_> =
                MetaMoveSolver::new(Rc::clone(&puzzle), scrambled_state.clone(), ()).collect();

            let out = puzzle
                .get_derived_state_from_turn_sequence(&scrambled_state, solution.iter().cloned());

            if out != puzzle.get_initial_state() {
                println!("It did not solve it")
            } else {
                sum += solution.len();
                num_solves += 1;
            }

            println!("3x3 solution length: {} turns", solution.len());
        }

        println!(
            "avg 3x3 solution length: {} turns, ({} / {})",
            sum as f64 / num_solves as f64,
            num_solves,
            num_scrambles
        );
    }
}
