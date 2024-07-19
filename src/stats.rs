use crate::words::{LetterMap, LetterSet, Word};

#[derive(Default, Debug, Clone)]
pub struct LetterStats {
    total: u32,
    counts: LetterMap<LetterMap<u32>>,
}

impl LetterStats {
    pub fn remove_word(&mut self, word: Word) {
        let letters = LetterSet::from(word);
        self.total -= 1;
        for letter in letters {
            for other_letter in letters {
                self.counts[letter][other_letter] -= 1;
            }
        }
    }

    pub fn relevance(&self, word: Word) -> u32 {
        let Self { total, counts } = self;
        LetterSet::from(word)
            .into_iter()
            .map(|letter| total - counts[letter][letter].abs_diff(total / 2))
            .sum()
    }
}

impl FromIterator<Word> for LetterStats {
    fn from_iter<T: IntoIterator<Item = Word>>(iter: T) -> Self {
        let mut total = 0;
        let mut counts: LetterMap<LetterMap<u32>> = Default::default();
        for word in iter {
            total += 1;
            let letters = LetterSet::from(word);
            for letter in letters {
                for other_letter in letters {
                    counts[letter][other_letter] += 1;
                }
            }
        }
        Self { total, counts }
    }
}
