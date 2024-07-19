use crate::{
    stats::LetterStats,
    words::{Feedback, Filter, Word},
};
use itertools::Itertools;

#[derive(Clone, Debug)]
pub struct Game {
    list: Vec<Word>,
    filter: Filter,
    stats: LetterStats,
}

impl Game {
    pub fn suggested_word(&self) -> Option<Word> {
        self.suggested_words(1).next()
    }

    pub fn suggested_words(&self, n: usize) -> impl Iterator<Item = Word> + '_ {
        self.list
            .iter()
            .copied()
            .k_largest_by_key(n, |&w| self.stats.relevance(w))
    }

    pub fn apply_feedback(&mut self, word: Word, feedback: [Feedback; 5]) {
        self.filter.restrict(word, feedback);
        self.list.retain(|&w| {
            let retain = w.matches(&self.filter);
            if !retain {
                self.stats.remove_word(w)
            }
            retain
        });
    }

    pub fn words(&self) -> &[Word] {
        &self.list
    }
}

impl Default for Game {
    fn default() -> Self {
        let list = Word::list().to_vec();
        let stats = list.iter().copied().collect();
        Self {
            list,
            stats,
            filter: Default::default(),
        }
    }
}
