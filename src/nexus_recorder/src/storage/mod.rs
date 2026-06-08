//! Storage module for event persistence.
//!
//! Provides JSONL-based event storage with timestamps synchronized to video.

mod jsonl;

pub use jsonl::{Event, EventType, EventWriter, MouseAction};
