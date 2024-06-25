use color_eyre::{
    eyre::{bail, eyre},
    Report, Result,
};
use std::{
    fmt::{self, Debug, Display, Write},
    ops::{Index, IndexMut},
};

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct Letter(u8);

impl Letter {
    pub const fn new(letter: char) -> Self {
        if !letter.is_ascii_lowercase() {
            panic!("letter out of range");
        }
        Self((letter as u8) - b'a')
    }
}

impl TryFrom<char> for Letter {
    type Error = Report;

    fn try_from(value: char) -> Result<Self> {
        if !value.is_ascii_lowercase() {
            bail!("invalid letter range: {value}")
        }
        Ok(Self(value as u8 - b'a'))
    }
}

impl Debug for Letter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Letter").field(&char::from(*self)).finish()
    }
}

impl Display for Letter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_char(char::from(*self))
    }
}

impl From<Letter> for char {
    fn from(value: Letter) -> Self {
        (value.0 + b'a') as char
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LetterMap<T>([T; 26]);

impl<T> IndexMut<Letter> for LetterMap<T> {
    fn index_mut(&mut self, index: Letter) -> &mut Self::Output {
        &mut self.0[index.0 as usize]
    }
}

impl<T> Index<Letter> for LetterMap<T> {
    type Output = T;

    fn index(&self, index: Letter) -> &Self::Output {
        &self.0[index.0 as usize]
    }
}

impl<T> LetterMap<T> {
    fn iter(&self) -> impl Iterator<Item = &T> {
        self.0.iter()
    }

    fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.0.iter_mut()
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct LetterSet(u32);

impl LetterSet {
    pub const FULL: LetterSet = LetterSet(0x3FFFFFF);

    pub fn contains(self, letter: Letter) -> bool {
        self.0 & (1 << letter.0) != 0
    }

    #[cfg(test)]
    pub fn inverse(self) -> Self {
        Self(!self.0 & Self::FULL.0)
    }

    pub fn insert(&mut self, letter: Letter) -> bool {
        let old = self.0;
        let new = old | (1 << letter.0);
        self.0 = new;
        old != new
    }

    pub fn remove(&mut self, letter: Letter) -> bool {
        let old = self.0;
        let new = old & !(1 << letter.0);
        self.0 = new;
        old != new
    }
}

impl Debug for LetterSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set()
            .entries(self.into_iter().map(char::from))
            .finish()
    }
}

impl<const N: usize> From<[Letter; N]> for LetterSet {
    fn from(values: [Letter; N]) -> Self {
        let mut value = 0;
        for letter in values {
            value |= 1 << letter.0;
        }
        Self(value)
    }
}

impl IntoIterator for LetterSet {
    type Item = Letter;

    type IntoIter = LetterSetIter;

    fn into_iter(self) -> Self::IntoIter {
        LetterSetIter(self.0)
    }
}

pub struct LetterSetIter(u32);

impl Iterator for LetterSetIter {
    type Item = Letter;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.0.trailing_zeros();
        if next < 26 {
            self.0 &= !(1 << next);
            Some(Letter(next as u8))
        } else {
            None
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Word([Letter; 5]);

impl Display for Word {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [a, b, c, d, e] = self.0;
        write!(f, "{a}{b}{c}{d}{e}")
    }
}

impl Word {
    pub fn list() -> Vec<Word> {
        include_str!("../words")
            .split_whitespace()
            .map(|w| w.try_into().expect("incorrect word in word list"))
            .collect()
    }

    #[inline]
    pub fn letter_count(self) -> LetterMap<u8> {
        let mut count = LetterMap::default();
        for letter in self.0 {
            count[letter] += 1;
        }
        count
    }

    pub fn matches(self, filter: &Filter) -> bool {
        if !self
            .0
            .iter()
            .zip(filter.mask.iter())
            .all(|(letter, set)| set.contains(*letter))
        {
            return false;
        }
        self.letter_count()
            .iter()
            .zip(filter.min_count.iter())
            .all(|(actual, minimum)| actual >= minimum)
    }

    fn iter(&self) -> impl Iterator<Item = Letter> + '_ {
        self.0.iter().copied()
    }

    pub fn stats(words: &[Word]) -> LetterMap<u32> {
        let mut map = LetterMap::default();
        for word in words {
            for letter in word.iter() {
                map[letter] += 1;
            }
        }
        map
    }

    pub fn relevance(self, stats: &LetterMap<u32>, total: u32) -> u32 {
        let target = total / 2;
        let mut seen = LetterSet::default();
        self.iter()
            .filter(|&letter| seen.insert(letter))
            .map(|letter| target.saturating_sub(stats[letter].abs_diff(target)))
            .sum()
    }
}

impl TryFrom<&str> for Word {
    type Error = Report;

    fn try_from(value: &str) -> Result<Self> {
        let letters = value
            .chars()
            .map(Letter::try_from)
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .map_err(|_| eyre!("words must have length 5"))?;

        Ok(Word(letters))
    }
}

impl From<[Letter; 5]> for Word {
    fn from(value: [Letter; 5]) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    pub mask: [LetterSet; 5],
    pub min_count: LetterMap<u8>,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            mask: [LetterSet::FULL; 5],
            min_count: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Feedback {
    #[default]
    Black,
    Yellow,
    Green,
}

impl Filter {
    pub fn restrict(&mut self, word: Word, feedback: [Feedback; 5]) {
        let mut min_count: LetterMap<u8> = Default::default();
        for (pos, (letter, feedback)) in word.iter().zip(feedback.iter()).enumerate() {
            match feedback {
                Feedback::Green => {
                    self.mask[pos] = LetterSet::from([letter]);
                    min_count[letter] += 1
                }
                Feedback::Yellow => {
                    self.mask[pos].remove(letter);
                    min_count[letter] += 1
                }
                Feedback::Black => {
                    if min_count[letter] > 0 {
                        self.mask[pos].remove(letter);
                    } else {
                        self.mask.iter_mut().for_each(|set| {
                            set.remove(letter);
                        });
                    }
                }
            }
        }
        self.min_count
            .iter_mut()
            .zip(min_count.iter())
            .for_each(|(cur, new)| *cur = (*cur).max(*new));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list() {
        let list = Word::list();

        assert_eq!(list.len(), 14855);
    }

    #[test]
    fn test_matches() -> Result<()> {
        let mut guess = Filter::default();
        use Feedback::*;
        guess.restrict(
            "ready".try_into().unwrap(),
            [Yellow, Black, Yellow, Green, Black],
        );

        assert_eq!(
            guess,
            Filter {
                mask: [
                    LetterSet::from([Letter::new('r'), Letter::new('e'), Letter::new('y')])
                        .inverse(),
                    LetterSet::from([Letter::new('e'), Letter::new('y')]).inverse(),
                    LetterSet::from([Letter::new('e'), Letter::new('a'), Letter::new('y')])
                        .inverse(),
                    LetterSet::from([Letter::new('d')]),
                    LetterSet::from([Letter::new('e'), Letter::new('y')]).inverse()
                ],
                min_count: {
                    let mut map = LetterMap::default();
                    map[Letter::new('r')] = 1;
                    map[Letter::new('a')] = 1;
                    map[Letter::new('d')] = 1;
                    map
                }
            }
        );

        for word in ["cardi", "bards"] {
            let word: Word = word.try_into().unwrap();
            assert!(word.matches(&guess));
        }
        for word in ["ready", "split", "bough"] {
            let word: Word = word.try_into().unwrap();
            assert!(!word.matches(&guess));
        }
        Ok(())
    }
}
