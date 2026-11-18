//! Bounded event ring for the Logs page.
//!
//! Server lifecycle lines, blackout transitions and errors all land here. The
//! ring is capped: a vLLM start prints hundreds of lines, and an unbounded
//! buffer would grow for as long as the window stays open. Identical
//! consecutive lines collapse into a repeat count for the same reason.

use std::collections::VecDeque;

use serde::Serialize;

const CAPACITY: usize = 400;

#[derive(Serialize, Clone, Debug)]
pub struct Event {
    pub id: u64,
    pub at: u64,
    pub kind: String,
    pub text: String,
    /// How many identical consecutive lines collapsed into this one.
    pub repeat: u32,
}

pub struct EventRing {
    next: u64,
    items: VecDeque<Event>,
}

impl Default for EventRing {
    fn default() -> Self {
        Self::new()
    }
}

impl EventRing {
    pub fn new() -> Self {
        Self {
            next: 1,
            items: VecDeque::with_capacity(CAPACITY),
        }
    }

    pub fn push(&mut self, kind: &str, text: impl AsRef<str>) {
        let text = sanitise(text.as_ref());
        if text.is_empty() {
            return;
        }
        if let Some(last) = self.items.back_mut() {
            // Only collapse within a short window: a line that reappears an
            // hour later is a new event, not a repeat.
            let fresh = now_ms().saturating_sub(last.at) < 5_000;
            if last.text == text && last.kind == kind && fresh {
                last.repeat = last.repeat.saturating_add(1);
                return;
            }
        }
        let event = Event {
            id: self.next,
            at: now_ms(),
            kind: kind.to_string(),
            text,
            repeat: 1,
        };
        self.next += 1;
        self.items.push_back(event);
        while self.items.len() > CAPACITY {
            self.items.pop_front();
        }
    }

    /// Events newer than `after`, oldest first. A cursor of 0 returns the tail
    /// below rather than the whole history, so a fresh window is not flooded.
    pub fn since(&self, after: u64) -> Vec<Event> {
        if after == 0 {
            return self.items.iter().rev().take(60).rev().cloned().collect();
        }
        self.items
            .iter()
            .filter(|e| e.id > after)
            .cloned()
            .collect()
    }
}

fn sanitise(text: &str) -> String {
    let trimmed = text.trim();
    // Cap a single line: one runaway log line must not become the whole ring.
    let limited: String = trimmed.chars().take(320).collect();
    limited
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
