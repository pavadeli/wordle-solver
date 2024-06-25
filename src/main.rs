use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
use itertools::Itertools;
use ratatui::{
    prelude::*,
    text::ToSpan,
    widgets::{Block, BorderType, Padding, Paragraph},
};
use std::{fmt::Debug, iter};
use tui::{Event, Tui};
use words::{Feedback, Filter, Letter, Word};

mod errors;
mod tui;
mod words;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    errors::install_hooks()?;
    App::new().run().await
}

#[derive(Debug)]
struct App {
    rows: Vec<Row>,
    cursor: usize,
    feedback_mode: bool,
    list: Vec<Word>,
    filter: Filter,
}

enum Action {
    Draw,
    Exit,
}

impl App {
    fn new() -> Self {
        let mut app = App {
            rows: vec![Row::default()],
            cursor: 0,
            feedback_mode: false,
            list: Word::list(),
            filter: Default::default(),
        };
        app.active_block_mut().selected = true;
        app.sort_list();
        app
    }

    /// runs the application's main loop until the user quits
    async fn run(&mut self) -> Result<()> {
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
                .chain(iter::once(Constraint::Fill(1))),
        )
        .spacing(1)
        .split(left);
        for (row, &area) in self.rows.iter().zip(row_areas.iter().skip(1)) {
            row.render(area, buf);
        }

        // Render word list to the right
        let [_, word_area, _] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Max(20),
            Constraint::Fill(1),
        ])
        .areas(right);
        Paragraph::new(
            self.list
                .iter()
                .take(word_area.height as usize - 2)
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
                self.list.len().to_string().dark_gray(),
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
            KeyCode::Enter if self.cursor == 5 => {
                self.set_cursor(0);
                self.feedback_mode = true;
                self.apply_previous_feedback();
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
                self.filter.restrict(
                    map_array(&row.letters, |l| {
                        l.contents.expect("all letters should be set by now")
                    })
                    .into(),
                    map_array(&row.letters, |l| l.color),
                );
                self.list.retain(|w| w.matches(&self.filter));
                self.sort_list();
                self.rows.push(Row::default());
                self.set_cursor(0);
                self.feedback_mode = false;
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

    fn sort_list(&mut self) {
        let stats = Word::stats(&self.list);
        let total = self.list.len() as u32;
        self.list
            .sort_unstable_by_key(|w| u32::MAX - w.relevance(&stats, total))
    }

    fn apply_previous_feedback(&mut self) {
        let mask = self.filter.mask;
        let row = self.last_row_mut();
        for (pos, &mask) in mask.iter().enumerate() {
            if let Ok(letter) = mask.into_iter().exactly_one() {
                if row.letters[pos].contents == Some(letter) {
                    row.letters[pos].color = Feedback::Green;
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

fn map_array<const N: usize, T, U: Debug>(arr: &[T; N], f: impl FnMut(&T) -> U) -> [U; N] {
    arr.iter().map(f).collect_vec().try_into().unwrap()
}
