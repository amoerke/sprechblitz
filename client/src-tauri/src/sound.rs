use rodio::source::{SineWave, Source};
use std::time::Duration;

pub enum Cue {
    Start,
    Stop,
    Error,
}

/// Plays a short generated cue; fire-and-forget on its own thread.
pub fn play(cue: Cue) {
    std::thread::spawn(move || {
        let Ok((_stream, handle)) = rodio::OutputStream::try_default() else {
            return;
        };
        let Ok(sink) = rodio::Sink::try_new(&handle) else {
            return;
        };
        let notes: &[(f32, u64)] = match cue {
            Cue::Start => &[(660.0, 90), (880.0, 130)],
            Cue::Stop => &[(880.0, 90), (660.0, 130)],
            Cue::Error => &[(233.0, 120), (196.0, 220)],
        };
        for &(freq, ms) in notes {
            sink.append(
                SineWave::new(freq)
                    .take_duration(Duration::from_millis(ms))
                    .amplify(0.15),
            );
        }
        sink.sleep_until_end();
    });
}
