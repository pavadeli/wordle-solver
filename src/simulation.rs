use crate::{
    game::Game,
    words::{Feedback, LetterMap, Word},
};
use itertools::Itertools;
use std::iter;

pub struct Simulation {
    word: Word,
    letter_counts: LetterMap<u8>,
    game: Game,
}

impl Simulation {
    pub fn new(word: Word) -> Self {
        let game = Game::default();
        let mut letter_counts = LetterMap::default();
        for letter in word.iter() {
            letter_counts[letter] += 1;
        }
        Self {
            word,
            letter_counts,
            game,
        }
    }

    pub fn run(&mut self) -> impl Iterator<Item = (Word, [Feedback; 5])> + '_ {
        iter::from_fn(|| {
            let guess = self.game.suggested_word().expect("unknown word");
            let feedback = self.get_feedback(guess);
            self.game.apply_feedback(guess, feedback);
            Some((guess, feedback))
        })
        .take_while_inclusive(|(_, feedback)| feedback != &[Feedback::Green; 5])
    }

    fn get_feedback(&self, guess: Word) -> [Feedback; 5] {
        let mut missing_letters = self.letter_counts.clone();
        let mut feedback = guess
            .iter()
            .zip(self.word.iter())
            .map(|(guess, letter)| {
                if guess == letter {
                    missing_letters[letter] -= 1;
                    Feedback::Green
                } else {
                    Feedback::Black
                }
            })
            .collect_vec();
        feedback
            .iter_mut()
            .zip(guess.iter())
            .for_each(|(feedback, letter)| {
                if *feedback == Feedback::Black && missing_letters[letter] > 0 {
                    missing_letters[letter] -= 1;
                    *feedback = Feedback::Yellow;
                }
            });
        feedback.try_into().unwrap()
    }
}
