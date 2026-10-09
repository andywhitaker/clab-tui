use crate::clab::model::ContainerInspectInfo;
use crossterm::event::{Event as CrosstermEvent, KeyEvent, MouseEvent};
use std::time::Duration;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

#[derive(Debug, Clone)]
pub enum AppEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    Tick,
    Log(String),
    InspectUpdated(Vec<ContainerInspectInfo>),
    DeployFinished(bool),
    DestroyFinished(bool),
    CliCommandFinished { cmd_name: String, success: bool },
    CliLog(String),
}

pub struct EventHandler {
    pub tx: UnboundedSender<AppEvent>,
    pub rx: UnboundedReceiver<AppEvent>,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = unbounded_channel();
        let event_tx = tx.clone();

        tokio::spawn(async move {
            let mut reader = crossterm::event::EventStream::new();
            let mut tick_interval = tokio::time::interval(tick_rate);

            loop {
                let tick_delay = tick_interval.tick();
                let crossterm_event = futures_util::StreamExt::next(&mut reader);

                tokio::select! {
                    _ = tick_delay => {
                        if event_tx.send(AppEvent::Tick).is_err() {
                            break;
                        }
                    }
                    maybe_event = crossterm_event => {
                        match maybe_event {
                            Some(Ok(evt)) => {
                                match evt {
                                    CrosstermEvent::Key(key) => {
                                        if key.kind == crossterm::event::KeyEventKind::Press
                                            || key.kind == crossterm::event::KeyEventKind::Repeat
                                        {
                                            let _ = event_tx.send(AppEvent::Key(key));
                                        }
                                    }
                                    CrosstermEvent::Mouse(mouse) => {
                                        let _ = event_tx.send(AppEvent::Mouse(mouse));
                                    }
                                    CrosstermEvent::Resize(w, h) => {
                                        let _ = event_tx.send(AppEvent::Resize(w, h));
                                    }
                                    _ => {}
                                }
                            }
                            Some(Err(_)) => {}
                            None => break,
                        }
                    }
                }
            }
        });

        Self { tx, rx }
    }
}
