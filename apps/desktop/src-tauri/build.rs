use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize)]
struct Manifest {
    models: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    #[allow(dead_code)]
    name: String,
    filename: String,
    sha256: String,
    #[serde(default)]
    required: bool,
}

fn main() {
    println!("cargo:rerun-if-changed=resources/models/manifest.json");

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let manifest_file_path = Path::new(&manifest_dir).join("resources/models/manifest.json");

    if !manifest_file_path.exists() {
        panic!(
            "\n\
            ================================================================================\n\
            BUILD ERROR: Model manifest is missing!\n\
            Expected file: {:?}\n\
            ================================================================================\n",
            manifest_file_path
        );
    }

    let manifest_content = std::fs::read_to_string(&manifest_file_path)
        .unwrap_or_else(|e| panic!("Failed to read {:?}: {}", manifest_file_path, e));
    let manifest: Manifest = serde_json::from_str(&manifest_content)
        .unwrap_or_else(|e| panic!("Failed to parse JSON in {:?}: {}", manifest_file_path, e));

    for model in manifest.models {
        println!("cargo:rerun-if-changed=resources/models/{}", model.filename);

        if model.required {
            let model_path = Path::new(&manifest_dir)
                .join("resources/models")
                .join(&model.filename);

            if !model_path.exists() {
                panic!(
                    "\n\
                    ================================================================================\n\
                    BUILD ERROR: Required Whisper speech model is missing!\n\
                    \n\
                    Expected file:    {:?}\n\
                    Expected SHA-256: {}\n\
                    \n\
                    PodReady bundles the full Whisper model in production releases so users never\n\
                    have to configure or download models separately.\n\
                    \n\
                    To provision the required runtime assets, please run:\n\
                        pnpm setup\n\
                    \n\
                    from the repository root.\n\
                    ================================================================================\n",
                    model_path, model.sha256
                );
            }

            verify_build_model(&model_path, &model.sha256);
        }
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
