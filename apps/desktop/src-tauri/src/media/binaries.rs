use std::process::Command;

/// Returns a `Command` configured to invoke `ffprobe`.
/// Centralizes executable resolution so bundled sidecars or custom paths can be supported easily.
pub fn ffprobe_cmd() -> Command {
    Command::new("ffprobe")
}

/// Returns a `Command` configured to invoke `ffmpeg`.
/// Centralizes executable resolution so bundled sidecars or custom paths can be supported easily.
pub fn ffmpeg_cmd() -> Command {
    Command::new("ffmpeg")
}
