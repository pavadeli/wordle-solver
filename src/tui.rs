use color_eyre::eyre::Result;
use crossterm::{
    cursor,
    event::{Event as CrosstermEvent, KeyEvent, KeyEventKind},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::{FutureExt, StreamExt};
use ratatui::backend::CrosstermBackend as Backend;
use std::ops::{Deref, DerefMut};
use tokio::{
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug)]
pub enum Event {
    Key(KeyEvent),
    Resize,
}

pub struct Tui {
    terminal: ratatui::Terminal<Backend<std::io::Stderr>>,
    cancellation_token: CancellationToken,
    event_rx: UnboundedReceiver<Event>,
}

impl Tui {
    pub fn start() -> Result<Self> {
        crossterm::terminal::enable_raw_mode()?;
        crossterm::execute!(std::io::stderr(), EnterAlternateScreen, cursor::Hide)?;

        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let cancellation_token = CancellationToken::new();
        start_events(cancellation_token.clone(), event_tx);
        Ok(Self {
            terminal: ratatui::Terminal::new(Backend::new(std::io::stderr()))?,
            cancellation_token,
            event_rx,
        })
    }

    pub async fn next(&mut self) -> Option<Event> {
        self.event_rx.recv().await
    }
}

fn start_events(
    cancellation_token: CancellationToken,
    event_tx: UnboundedSender<Event>,
) -> JoinHandle<()> {
    let task = tokio::spawn(async move {
        let mut reader = crossterm::event::EventStream::new();
        loop {
            let crossterm_event = reader.next().fuse();
            tokio::select! {
                _ = cancellation_token.cancelled() => {
                    break;
                }
                maybe_event = crossterm_event => {
                    match maybe_event {
                        Some(Ok(evt)) => match evt {
                            CrosstermEvent::Key(key) => {
                                if key.kind == KeyEventKind::Press && event_tx.send(Event::Key(key)).is_err() {
                                    break;
                                }
                            }
                            CrosstermEvent::Resize(_, _) => {
                                if event_tx.send(Event::Resize).is_err() {
                                    break;
                                }
                            }
                            CrosstermEvent::Mouse(_)
                            | CrosstermEvent::FocusLost
                            | CrosstermEvent::FocusGained
                            | CrosstermEvent::Paste(_) => {}
                        },
                        Some(Err(e)) => {
                            panic!("{e}");
                        }
                        None => {}
                    }
                },
            }
        }
    });
    task
}

impl Deref for Tui {
    type Target = ratatui::Terminal<Backend<std::io::Stderr>>;

    fn deref(&self) -> &Self::Target {
        &self.terminal
    }
}

impl DerefMut for Tui {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.terminal
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        if crossterm::terminal::is_raw_mode_enabled().unwrap_or_default() {
            let _unused = self.terminal.flush();
        }
        restore();
        self.cancellation_token.cancel();
    }
}

pub fn restore() {
    if crossterm::terminal::is_raw_mode_enabled().unwrap_or_default() {
        let _unused = crossterm::execute!(std::io::stderr(), LeaveAlternateScreen, cursor::Show);
        let _unused = crossterm::terminal::disable_raw_mode();
    }
}
