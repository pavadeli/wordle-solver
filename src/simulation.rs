use crate::{
    game::Game,
    words::{Feedback, LetterMap, Word},
};
use color_eyre::eyre::{eyre, Result};
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

    pub fn run(&mut self) -> impl Iterator<Item = Result<(Word, [Feedback; 5])>> + '_ {
        iter::from_fn(|| {
            let guess = match self.game.suggested_word() {
                Some(word) => word,
                None => return Some(Err(eyre!("unknown word \"{}\"", self.word))),
            };
            let feedback = self.get_feedback(guess);
            self.game.apply_feedback(guess, feedback);
            Some(Ok((guess, feedback)))
        })
        .take_while_inclusive(|i| match i {
            Ok((_, f)) => f != &[Feedback::Green; 5],
            _ => false,
        })
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
