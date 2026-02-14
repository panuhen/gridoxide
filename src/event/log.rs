use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::command::{Command, CommandSource};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: u64,
    pub timestamp: u64,
    pub source: CommandSource,
    pub command: Command,
}

/// Ring buffer of recent events for MCP "listening"
pub struct EventLog {
    events: VecDeque<Event>,
    next_id: u64,
    max_events: usize,
}

impl EventLog {
    pub fn new() -> Self {
        Self {
            events: VecDeque::new(),
            next_id: 1,
            max_events: 500,
        }
    }

    /// Log a command as an event
    pub fn log(&mut self, command: Command, source: CommandSource) {
        if !command.is_loggable() {
            return;
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let event = Event {
            id: self.next_id,
            timestamp,
            source,
            command,
        };

        self.next_id += 1;
        self.events.push_back(event);

        // Trim old events
        while self.events.len() > self.max_events {
            self.events.pop_front();
        }
    }

    /// Get all events since a given ID
    pub fn get_events_since(&self, since_id: u64) -> Vec<Event> {
        self.events
            .iter()
            .filter(|e| e.id > since_id)
            .cloned()
            .collect()
    }

    /// Get the latest event ID
    pub fn latest_id(&self) -> u64 {
        self.events.back().map(|e| e.id).unwrap_or(0)
    }

    /// Get total event count
    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

impl Default for EventLog {
    fn default() -> Self {
        Self::new()
    }
}
