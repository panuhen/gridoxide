use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};

use super::types::{Command, CommandSource};

/// Central command bus for dispatching commands from TUI or MCP
pub struct CommandBus {
    tx: Sender<(Command, CommandSource)>,
    rx: Receiver<(Command, CommandSource)>,
}

impl CommandBus {
    pub fn new() -> Self {
        let (tx, rx) = bounded(256);
        Self { tx, rx }
    }

    /// Get a sender that can be cloned and shared
    pub fn sender(&self) -> CommandSender {
        CommandSender {
            tx: self.tx.clone(),
        }
    }

    /// Get a receiver (typically for the audio thread)
    pub fn receiver(&self) -> CommandReceiver {
        CommandReceiver {
            rx: self.rx.clone(),
        }
    }

    /// Try to receive a command (non-blocking)
    pub fn try_recv(&self) -> Option<(Command, CommandSource)> {
        self.rx.try_recv().ok()
    }
}

impl Default for CommandBus {
    fn default() -> Self {
        Self::new()
    }
}

/// Cloneable sender for dispatching commands
#[derive(Clone)]
pub struct CommandSender {
    tx: Sender<(Command, CommandSource)>,
}

impl CommandSender {
    /// Send a command (non-blocking, drops if buffer full)
    pub fn send(&self, cmd: Command, source: CommandSource) -> bool {
        match self.tx.try_send((cmd, source)) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => {
                eprintln!("Warning: Command buffer full, dropping command");
                false
            }
            Err(TrySendError::Disconnected(_)) => false,
        }
    }
}

/// Receiver for consuming commands
#[derive(Clone)]
pub struct CommandReceiver {
    rx: Receiver<(Command, CommandSource)>,
}

impl CommandReceiver {
    /// Try to receive a command (non-blocking)
    pub fn try_recv(&self) -> Option<(Command, CommandSource)> {
        self.rx.try_recv().ok()
    }
}
