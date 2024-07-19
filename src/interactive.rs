use crate::{
    game::Game,
    tui::{Event, Tui},
    words::{Feedback, Letter, LetterSet},
};
use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
use itertools::Itertools;
use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Padding, Paragraph},
};
use std::{fmt::Debug, iter};
use text::ToSpan;

#[derive(Debug)]
pub struct App {
    rows: Vec<Row>,
    cursor: usize,
    feedback_mode: bool,
    game: Game,
}

enum Action {
    Draw,
    Exit,
}

impl App {
    pub fn new() -> Self {
        let mut app = App {
            rows: vec![Row::default()],
            cursor: 0,
            feedback_mode: false,
            game: Default::default(),
        };
        app.active_block_mut().selected = true;
        app.fill_suggested_word();
        app
    }

    /// runs the application's main loop until the user quits
    #[tokio::main(flavor = "current_thread")]
    pub async fn run(&mut self) -> Result<()> {
        let tui = &mut Tui::start()?;

        self.draw(tui)?;

        while let Some(evt) = tui.next().await {
            match self.handle_event(evt) {
                Some(Action::Draw) => self.draw(tui)?,
                Some(Action::Exit) => break,
                None => {}
            }
        }

        Ok(())
    }

    fn draw(&self, tui: &mut Tui) -> Result<()> {
        tui.draw(|f| self.render(f.size(), f.buffer_mut()))?;
        Ok(())
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        let [_, left, right, _] = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Max(42),
            Constraint::Length(9),
            Constraint::Fill(1),
        ])
        .spacing(1)
        .areas(area);

        // Render rows to the left
        let row_areas = Layout::vertical(
            iter::once(Constraint::Fill(1))
                .chain(iter::repeat(Constraint::Length(3)).take(self.rows.len()))
                .chain(iter::once(Constraint::Length(1)))
                .chain(iter::once(Constraint::Fill(1))),
        )
        .spacing(1)
        .split(left);

        let mut area_iter = row_areas.iter().skip(1);
        for (row, &area) in self.rows.iter().zip(&mut area_iter) {
            row.render(area, buf);
        }

        let mode_area = *area_iter.next().unwrap();
        Paragraph::new(if self.feedback_mode {
            "ENTER FEEDBACK"
        } else {
            "ENTER WORD"
        })
        .render(mode_area, buf);

        // Render word list to the right
        let [_, word_area, _] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Max(20),
            Constraint::Fill(1),
        ])
        .areas(right);
        Paragraph::new(
            self.game
                .suggested_words(word_area.height as usize - 2)
                .format("\n")
                .to_string(),
        )
        .block(
            const {
                Block::bordered()
                    .border_type(BorderType::Plain)
                    .padding(Padding::horizontal(1))
            }
            .title(Line::from(vec![
                "╢".into(),
                self.game.words().len().to_string().dark_gray(),
                "╟".into(),
            ])),
        )
        .render(word_area, buf);
    }

    /// updates the application's state based on user input
    fn handle_event(&mut self, evt: Event) -> Option<Action> {
        match evt {
            // it's important to check that the event is a key press event as
            // crossterm also emits key release and repeat events on Windows.
            Event::Key(key_event) => {
                if self.feedback_mode {
                    self.handle_key_in_feedback_mode(key_event.code)
                } else {
                    self.handle_key_in_word_mode(key_event.code)
                }
            }
            Event::Resize => Some(Action::Draw),
        }
    }

    fn handle_key_in_word_mode(&mut self, code: KeyCode) -> Option<Action> {
        match code {
            KeyCode::Backspace if self.cursor > 0 => {
                self.set_cursor(self.cursor - 1);
                self.active_block_mut().contents = None;
                Some(Action::Draw)
            }
            KeyCode::Char(ch @ ('a'..='z' | 'A'..='Z')) if self.cursor < 5 => {
                self.active_block_mut().contents = Some(Letter::new(ch.to_ascii_lowercase()));
                self.set_cursor(self.cursor + 1);
                Some(Action::Draw)
            }
            KeyCode::Enter if self.has_word() => {
                self.set_cursor(0);
                self.feedback_mode = true;
                self.apply_expected_feedback();
                Some(Action::Draw)
            }
            KeyCode::Esc => Some(Action::Exit),
            _ => None,
        }
    }

    fn handle_key_in_feedback_mode(&mut self, code: KeyCode) -> Option<Action> {
        match code {
            KeyCode::Enter => {
                self.active_block_mut().selected = false;
                let row = self.last_row();
                let word = row
                    .letters
                    .each_ref()
                    .map(|l| l.contents.expect("all letters should be set by now"))
                    .into();
                let feedback = row.letters.each_ref().map(|l| l.color);
                self.game.apply_feedback(word, feedback);
                self.rows.push(Row::default());
                self.set_cursor(0);
                self.feedback_mode = false;
                self.fill_suggested_word();
                Some(Action::Draw)
            }
            KeyCode::Right => {
                self.set_cursor((self.cursor + 1) % 5);
                Some(Action::Draw)
            }
            KeyCode::Left => {
                self.set_cursor((self.cursor + 4) % 5);
                Some(Action::Draw)
            }
            KeyCode::Up => {
                self.active_block_mut().cycle_color();
                Some(Action::Draw)
            }
            KeyCode::Down => {
                let active_block = self.active_block_mut();
                active_block.cycle_color();
                active_block.cycle_color();
                Some(Action::Draw)
            }
            KeyCode::Esc => Some(Action::Exit),
            _ => None,
        }
    }

    fn set_cursor(&mut self, new_cursor: usize) {
        let old_cursor = self.cursor;
        self.cursor = new_cursor;
        let row = self.last_row_mut();
        if let Some(block) = row.letters.get_mut(old_cursor) {
            block.selected = false;
        }
        if let Some(block) = row.letters.get_mut(new_cursor) {
            block.selected = true;
        }
    }

    fn last_row(&self) -> &Row {
        self.rows.last().expect("there is always at least one row")
    }

    fn last_row_mut(&mut self) -> &mut Row {
        self.rows
            .last_mut()
            .expect("there is always at least one row")
    }

    fn active_block_mut(&mut self) -> &mut LetterBlock {
        let cursor = self.cursor;
        &mut self.last_row_mut().letters[cursor]
    }

    fn fill_suggested_word(&mut self) {
        let Some(word) = self.game.suggested_word() else {
            return;
        };
        self.last_row_mut()
            .letters
            .iter_mut()
            .zip(word.iter())
            .for_each(|(block, letter)| block.contents = Some(letter));
    }

    fn has_word(&self) -> bool {
        self.last_row().letters.iter().all(|l| l.contents.is_some())
    }

    fn apply_expected_feedback(&mut self) {
        let mut remaining_letters = [LetterSet::EMPTY; 5];
        let mut known_mandatory_letters = LetterSet::FULL;
        for &word in self.game.words() {
            known_mandatory_letters = known_mandatory_letters.intersect(word.into());
            for (set, letter) in remaining_letters.iter_mut().zip(word.iter()) {
                set.insert(letter);
            }
        }
        let last_row = self.last_row_mut();
        for (block, letter) in last_row.letters.iter_mut().zip(remaining_letters) {
            if let Ok(letter) = letter.into_iter().exactly_one() {
                block.contents = Some(letter);
                block.color = Feedback::Green;
                known_mandatory_letters.remove(letter);
            }
        }
        for maybe_misplaced in known_mandatory_letters {
            for block in last_row.letters.iter_mut() {
                if block.contents == Some(maybe_misplaced) && block.color == Feedback::Black {
                    block.color = Feedback::Yellow;
                }
            }
        }
    }
}

#[derive(Debug, Default)]
struct Row {
    letters: [LetterBlock; 5],
}

impl Row {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let areas = Layout::horizontal([Constraint::Length(7); 5])
            .spacing(1)
            .split(area);
        for (block, &area) in self.letters.iter().zip(areas.iter()) {
            block.render(area, buf);
        }
    }
}

#[derive(Debug, Default, Clone)]
struct LetterBlock {
    contents: Option<Letter>,
    color: Feedback,
    selected: bool,
}

impl LetterBlock {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let ch = self
            .contents
            .map(|l| char::from(l).to_ascii_uppercase())
            .unwrap_or(' ');
        let paragraph = Paragraph::new(ch.to_span())
            .style(self.style())
            .alignment(Alignment::Center);
        if area.height < 3 || area.width < 7 {
            paragraph.render(area, buf);
            return;
        }
        paragraph.block(self.block()).render(area, buf);
    }

    fn block(&self) -> Block {
        if self.selected {
            const {
                Block::bordered()
                    .border_type(BorderType::Thick)
                    .padding(Padding::horizontal(2))
            }
        } else {
            const {
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .padding(Padding::horizontal(2))
            }
        }
    }

    fn style(&self) -> Style {
        if self.selected {
            match self.color {
                Feedback::Black => {
                    const {
                        Style::reset()
                            .fg(Color::Black)
                            .bg(Color::DarkGray)
                            .add_modifier(Modifier::BOLD)
                    }
                }
                Feedback::Yellow => {
                    const {
                        Style::reset()
                            .fg(Color::Black)
                            .bg(Color::LightYellow)
                            .add_modifier(Modifier::BOLD)
                    }
                }
                Feedback::Green => {
                    const {
                        Style::reset()
                            .fg(Color::Black)
                            .bg(Color::LightGreen)
                            .add_modifier(Modifier::BOLD)
                    }
                }
            }
        } else {
            match self.color {
                Feedback::Black => {
                    const {
                        Style::reset()
                            .fg(Color::White)
                            .bg(Color::Black)
                            .add_modifier(Modifier::BOLD)
                    }
                }
                Feedback::Yellow => {
                    const {
                        Style::reset()
                            .fg(Color::Black)
                            .bg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    }
                }
                Feedback::Green => {
                    const {
                        Style::reset()
                            .fg(Color::Black)
                            .bg(Color::Green)
                            .add_modifier(Modifier::BOLD)
                    }
                }
            }
        }
    }

    fn cycle_color(&mut self) {
        self.color = match self.color {
            Feedback::Black => Feedback::Yellow,
            Feedback::Yellow => Feedback::Green,
            Feedback::Green => Feedback::Black,
        }
    }
}
