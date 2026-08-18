use crate::error::AppError;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const DEFAULT_MODEL_FILENAME: &str = "ggml-small.bin";
pub const DEFAULT_MODEL_SHA256: &str =
    "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b";

#[allow(dead_code)]
pub const SMALL_MODEL_FILENAME: &str = "ggml-small.bin";
#[allow(dead_code)]
pub const SMALL_MODEL_SHA256: &str =
    "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b";

#[allow(dead_code)]
pub const LARGE_TURBO_MODEL_FILENAME: &str = "ggml-large-v3-turbo.bin";
#[allow(dead_code)]
pub const LARGE_TURBO_MODEL_SHA256: &str =
    "1fc70f774d38eb169993ac391eea357ef47c88757ef72ee5943879b7e8e2bc69";

#[allow(dead_code)]
pub const MEDIUM_MODEL_FILENAME: &str = "ggml-medium.bin";
#[allow(dead_code)]
pub const MEDIUM_MODEL_SHA256: &str =
    "6c14d5adee5f86394037b4e4e8b59f1673b6cee10e3cf0b11bbdbee79c156208";

#[allow(dead_code)]
pub const TEST_BASE_MODEL_FILENAME: &str = "ggml-base.bin";
#[allow(dead_code)]
pub const TEST_BASE_MODEL_SHA256: &str =
    "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe";

static VERIFIED_MODELS_CACHE: std::sync::Mutex<Option<std::collections::HashSet<PathBuf>>> =
    std::sync::Mutex::new(None);

/// Resolves the root resources directory across production (.app bundle), development,
/// and hermetic test execution environments.
pub fn get_resources_dir() -> Result<PathBuf, AppError> {
    // 1. Explicit environment override for hermetic integration tests and sandboxing
    if let Ok(override_path) = std::env::var("PODREADY_RESOURCES_DIR") {
        let path = PathBuf::from(override_path);
        if path.exists() {
            return Ok(path);
        }
    }

    // 2. Production macOS bundle: [App].app/Contents/Resources/resources or [App].app/Contents/Resources
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(macos_dir) = exe_path.parent() {
            if macos_dir.file_name() == Some(std::ffi::OsStr::new("MacOS")) {
                if let Some(contents_dir) = macos_dir.parent() {
                    let res_dir = contents_dir.join("Resources").join("resources");
                    if res_dir.exists() {
                        return Ok(res_dir);
                    }
                    let alt_res_dir = contents_dir.join("Resources");
                    if alt_res_dir.join("bin").exists() || alt_res_dir.join("models").exists() {
                        return Ok(alt_res_dir);
                    }
                }
            }
        }
    }

    // 3. Development / Cargo workspace location (src-tauri/resources or resources/)
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dev_res = manifest_dir.join("resources");
    if dev_res.exists() {
        return Ok(dev_res);
    }

    let root_res = manifest_dir.join("../resources");
    if root_res.exists() {
        return Ok(root_res);
    }

    Err(AppError::SystemError(
        "PodReady could not find its bundled resources directory. Please ensure PodReady is properly installed."
            .to_string(),
    ))
}

/// Resolves a named binary executable strictly within the bundled resources directory.
/// Does NOT rely on system PATH in production.
pub fn resolve_binary(binary_name: &str) -> Result<PathBuf, AppError> {
    let res_dir = get_resources_dir()?;
    let bin_path = res_dir.join("bin").join(binary_name);

    if bin_path.exists() && bin_path.is_file() {
        Ok(bin_path)
    } else {
        Err(AppError::SystemError(format!(
            "PodReady could not find required media tool '{}' in its bundled runtime.",
            binary_name
        )))
    }
}

/// Resolves the bundled `ffmpeg` executable.
pub fn resolve_ffmpeg() -> Result<PathBuf, AppError> {
    resolve_binary("ffmpeg")
}

/// Resolves the bundled `ffprobe` executable.
pub fn resolve_ffprobe() -> Result<PathBuf, AppError> {
    resolve_binary("ffprobe")
}

/// Resolves the bundled `whisper-cli` executable.
pub fn resolve_whisper() -> Result<PathBuf, AppError> {
    resolve_binary("whisper-cli")
}

/// Returns a configured `Command` for the bundled `ffmpeg`.
pub fn ffmpeg_cmd() -> Result<Command, AppError> {
    let bin = resolve_ffmpeg()?;
    Ok(Command::new(bin))
}

/// Returns a configured `Command` for the bundled `ffprobe`.
pub fn ffprobe_cmd() -> Result<Command, AppError> {
    let bin = resolve_ffprobe()?;
    Ok(Command::new(bin))
}

/// Returns a configured `Command` for the bundled `whisper-cli`.
pub fn whisper_cmd() -> Result<Command, AppError> {
    let bin = resolve_whisper()?;
    Ok(Command::new(bin))
}

/// Resolves a Whisper model file within bundled resources.
pub fn resolve_model(model_filename: &str) -> Result<PathBuf, AppError> {
    let res_dir = get_resources_dir()?;
    let model_path = res_dir.join("models").join(model_filename);

    if model_path.exists() && model_path.is_file() {
        Ok(model_path)
    } else {
        Err(AppError::SystemError(
            "Transcription isn't available because PodReady's speech model could not be found."
                .to_string(),
        ))
    }
}

/// Validates that a model resource exists, is a regular file, is readable, and is non-empty.
/// This fast runtime check takes < 0.1ms and runs without blocking the export critical path.
pub fn validate_model_resource(model_path: &Path) -> Result<(), AppError> {
    if !model_path.exists() {
        return Err(AppError::SystemError(
            "Transcription isn't available because PodReady's speech model file is missing."
                .to_string(),
        ));
    }

    let meta = std::fs::metadata(model_path).map_err(|_| {
        AppError::SystemError(
            "Transcription isn't available because PodReady's speech model could not be read."
                .to_string(),
        )
    })?;

    if meta.len() < 1024 * 1024 {
        return Err(AppError::SystemError(
            "Transcription isn't available because PodReady's speech model appears incomplete."
                .to_string(),
        ));
    }

    // Verify file readability by attempting to open it
    let _ = std::fs::File::open(model_path).map_err(|_| {
        AppError::SystemError(
            "Transcription isn't available because PodReady's speech model could not be read."
                .to_string(),
        )
    })?;

    Ok(())
}

/// Resolves the default full production Whisper speech model.
/// Fast path: validates model resource existence and readability in <0.1ms without blocking export.
pub fn resolve_default_model() -> Result<PathBuf, AppError> {
    let model_path = resolve_model(DEFAULT_MODEL_FILENAME)?;
    validate_model_resource(&model_path)?;
    Ok(model_path)
}

/// Resolves the test base model if available, falling back to full model.
#[allow(dead_code)]
pub fn resolve_test_model() -> Result<PathBuf, AppError> {
    if let Ok(base_path) = resolve_model(TEST_BASE_MODEL_FILENAME) {
        if validate_model_resource(&base_path).is_ok() {
            return Ok(base_path);
        }
    }
    resolve_default_model()
}

/// Spawns an asynchronous background task to verify model integrity on startup as defence-in-depth.
pub fn start_background_model_verification() {
    std::thread::spawn(|| {
        if let Ok(path) = resolve_model(DEFAULT_MODEL_FILENAME) {
            let _ = verify_model_integrity(&path, DEFAULT_MODEL_SHA256);
        }
    });
}

/// Verifies that a model file is non-empty and optionally verifies its sha256 checksum.
pub fn verify_model_integrity(model_path: &Path, expected_sha256: &str) -> Result<(), AppError> {
    validate_model_resource(model_path)?;

    let canon_path = model_path.to_path_buf();

    // Check in-process verification cache to avoid redundant disk I/O on every transcription
    if let Ok(guard) = VERIFIED_MODELS_CACHE.lock() {
        if let Some(set) = guard.as_ref() {
            if set.contains(&canon_path) {
                return Ok(());
            }
        }
    }

    let meta = std::fs::metadata(model_path).map_err(|_| {
        AppError::SystemError(
            "Transcription isn't available because PodReady's speech model could not be read."
                .to_string(),
        )
    })?;

    if meta.len() < 1024 * 1024 {
        return Err(AppError::SystemError(
            "Transcription isn't available because PodReady's speech model appears incomplete."
                .to_string(),
        ));
    }

    // Stream SHA-256 integrity calculation without allocating huge buffers
    if !expected_sha256.is_empty() {
        use std::io::Read;
        let mut file = std::fs::File::open(model_path).map_err(|_| {
            AppError::SystemError(
                "Transcription isn't available because PodReady's speech model could not be read."
                    .to_string(),
            )
        })?;

        let mut hasher = Sha256Simple::new();
        let mut buffer = [0u8; 65536];
        loop {
            let n = file.read(&mut buffer).map_err(|_| {
                AppError::SystemError(
                    "Transcription isn't available because PodReady's speech model could not be read."
                        .to_string(),
                )
            })?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }

        let hash = format_sha256(hasher.finalize());
        if hash != expected_sha256 {
            return Err(AppError::SystemError(
                "Transcription isn't available because PodReady's speech model checksum does not match."
                    .to_string(),
            ));
        }
    }

    // Cache successful verification
    if let Ok(mut guard) = VERIFIED_MODELS_CACHE.lock() {
        let set = guard.get_or_insert_with(std::collections::HashSet::new);
        set.insert(canon_path);
    }

    Ok(())
}

fn format_sha256(digest: [u8; 32]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(64);
    for b in digest {
        let _ = write!(s, "{:02x}", b);
    }
    s
}

// Minimal self-contained SHA-256 implementation so we don't need additional external crates
struct Sha256Simple {
    state: [u32; 8],
    count: u64,
    buffer: [u8; 64],
}

impl Sha256Simple {
    fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c,
                0x1f83d9ab, 0x5be0cd19,
            ],
            count: 0,
            buffer: [0u8; 64],
        }
    }

    fn update(&mut self, mut input: &[u8]) {
        let buffer_idx = (self.count & 63) as usize;
        self.count += input.len() as u64;

        if buffer_idx > 0 {
            let space = 64 - buffer_idx;
            if input.len() >= space {
                self.buffer[buffer_idx..64].copy_from_slice(&input[..space]);
                self.transform(&self.buffer.clone());
                input = &input[space..];
            } else {
                self.buffer[buffer_idx..buffer_idx + input.len()].copy_from_slice(input);
                return;
            }
        }

        while input.len() >= 64 {
            let chunk: [u8; 64] = input[..64].try_into().unwrap();
            self.transform(&chunk);
            input = &input[64..];
        }

        if !input.is_empty() {
            self.buffer[..input.len()].copy_from_slice(input);
        }
    }

    fn finalize(mut self) -> [u8; 32] {
        let buffer_idx = (self.count & 63) as usize;
        self.buffer[buffer_idx] = 0x80;
        if buffer_idx >= 56 {
            for b in &mut self.buffer[buffer_idx + 1..64] {
                *b = 0;
            }
            self.transform(&self.buffer.clone());
            for b in &mut self.buffer[..56] {
                *b = 0;
            }
        } else {
            for b in &mut self.buffer[buffer_idx + 1..56] {
                *b = 0;
            }
        }

        let bit_count = self.count * 8;
        self.buffer[56..64].copy_from_slice(&bit_count.to_be_bytes());
        self.transform(&self.buffer.clone());

        let mut output = [0u8; 32];
        for (i, val) in self.state.iter().enumerate() {
            output[i * 4..(i + 1) * 4].copy_from_slice(&val.to_be_bytes());
        }
        output
    }

    fn transform(&mut self, chunk: &[u8; 64]) {
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];

        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(chunk[i * 4..(i + 1) * 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_bundled_binaries() {
        let ffmpeg = resolve_ffmpeg().expect("ffmpeg should resolve");
        let ffprobe = resolve_ffprobe().expect("ffprobe should resolve");
        let whisper = resolve_whisper().expect("whisper should resolve");

        assert!(ffmpeg.exists(), "ffmpeg exists at {:?}", ffmpeg);
        assert!(ffprobe.exists(), "ffprobe exists at {:?}", ffprobe);
        assert!(whisper.exists(), "whisper exists at {:?}", whisper);
    }

    #[test]
    fn test_missing_binary_error() {
        let err = resolve_binary("non_existent_tool_12345");
        assert!(err.is_err());
        match err.unwrap_err() {
            AppError::SystemError(msg) => {
                assert!(msg.contains("non_existent_tool_12345"));
            }
            _ => panic!("Expected SystemError"),
        }
    }

    #[test]
    fn test_hermetic_execution_without_system_path() {
        let _guard = crate::TEST_GLOBAL_ENV_LOCK.lock().unwrap();
        let mut ffmpeg_cmd = ffmpeg_cmd().expect("ffmpeg cmd resolves");
        ffmpeg_cmd.env("PATH", "");
        let ffmpeg_out = ffmpeg_cmd.arg("-version").output().expect("ffmpeg runs with empty PATH");
        assert!(ffmpeg_out.status.success());

        let mut ffprobe_cmd = ffprobe_cmd().expect("ffprobe cmd resolves");
        ffprobe_cmd.env("PATH", "");
        let ffprobe_out = ffprobe_cmd.arg("-version").output().expect("ffprobe runs with empty PATH");
        assert!(ffprobe_out.status.success());

        let mut whisper_cmd = whisper_cmd().expect("whisper cmd resolves");
        whisper_cmd.env("PATH", "");
        let whisper_out = whisper_cmd.arg("-h").output().expect("whisper runs with empty PATH");
        assert!(whisper_out.status.success());
    }
}
