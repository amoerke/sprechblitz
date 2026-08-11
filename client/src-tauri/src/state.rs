use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use crate::audio::AudioHandle;
use crate::history::HistoryEntry;
use crate::server_api::ModelEntry;
use crate::settings::Settings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Session {
    Idle,
    Recording { generation: u64 },
    Transcribing,
}

/// A failed dictation kept in memory so it can be re-sent from the tray.
#[derive(Clone)]
pub struct PendingDictation {
    pub wav: Vec<u8>,
    pub model: String,
    pub language: String,
    pub commands: bool,
}

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub audio: AudioHandle,
    pub session: Mutex<Session>,
    pub last_failed: Mutex<Option<PendingDictation>>,
    pub models: Mutex<Vec<ModelEntry>>,
    pub history: Mutex<Vec<HistoryEntry>>,
    pub last_toggle: Mutex<Instant>,
    generation: AtomicU64,
}

impl AppState {
    pub fn new(settings: Settings, models: Vec<ModelEntry>, history: Vec<HistoryEntry>) -> Self {
        Self {
            settings: Mutex::new(settings),
            audio: AudioHandle::spawn(),
            session: Mutex::new(Session::Idle),
            last_failed: Mutex::new(None),
            models: Mutex::new(models),
            history: Mutex::new(history),
            last_toggle: Mutex::new(Instant::now()),
            generation: AtomicU64::new(0),
        }
    }

    pub fn next_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }
}
