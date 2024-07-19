use color_eyre::eyre::Result;
use indicatif::ParallelProgressIterator;
use itertools::Itertools;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use simulation::Simulation;
use std::env::args;
use words::Word;

mod errors;
mod game;
mod interactive;
mod simulation;
mod stats;
mod tui;
mod words;

fn main() -> Result<()> {
    errors::install_hooks()?;
    match args().nth(1).as_deref() {
        Some("all") => {
            let results = Word::list()
                .par_iter()
                .map(|&word| (word, Simulation::new(word).run().count()))
                .progress()
                .collect::<Vec<_>>();
            let (min, max) = results.iter().minmax_by_key(|t| t.1).into_option().unwrap();
            println!("Min: {} in {} rounds", min.0, min.1);
            println!("Max: {} in {} rounds", max.0, max.1);
            println!(
                "Avg: {:.2}",
                results.iter().map(|t| t.1 as f64).sum::<f64>() / results.len() as f64
            );
            let failed = results.iter().filter(|t| t.1 > 6).count();
            let perc = failed as f64 / Word::list().len() as f64 * 100.0;
            println!("Failed words: {} ({perc:.2}%)", failed);
            Ok(())
        }
        Some(word) => {
            let word = Word::try_from(word)?;
            println!("Starting simulation with word \"{word}\"");
            for (guess, feedback) in Simulation::new(word).run() {
                println!("Guess: {guess}, feedback: {feedback:?}");
            }
            Ok(())
        }
        None => interactive::App::new().run(),
    }
}
