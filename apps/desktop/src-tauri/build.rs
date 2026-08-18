use std::path::Path;

const EXPECTED_SMALL_MODEL_SHA256: &str =
    "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b";
const SMALL_MODEL_FILENAME: &str = "ggml-small.bin";

fn main() {
    println!("cargo:rerun-if-changed=resources/models");

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let model_path = Path::new(&manifest_dir).join("resources/models").join(SMALL_MODEL_FILENAME);

    if model_path.exists() {
        verify_build_model(&model_path, EXPECTED_SMALL_MODEL_SHA256);
    }

    tauri_build::build();
}

fn verify_build_model(path: &Path, expected_hash: &str) {
    use sha2::{Digest, Sha256};
    use std::io::Read;

    let mut file = std::fs::File::open(path)
        .unwrap_or_else(|e| panic!("Failed to open speech model {:?}: {}", path, e));
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];

    loop {
        let n = file
            .read(&mut buffer)
            .unwrap_or_else(|e| panic!("Failed to read speech model {:?}: {}", path, e));
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    let actual_hash = format!("{:x}", hasher.finalize());
    if actual_hash != expected_hash {
        panic!(
            "BUILD ERROR: Bundled Whisper speech model {:?} checksum mismatch!\nExpected: {}\nActual:   {}",
            path, expected_hash, actual_hash
        );
    }
}
