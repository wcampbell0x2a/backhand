use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use test_assets_ureq::{TestAsset, dl_test_files_backoff};

static TEST_ASSETS: OnceLock<TestAsset> = OnceLock::new();

pub fn get_test_assets() -> &'static TestAsset {
    TEST_ASSETS.get_or_init(|| {
        let mut config_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        config_path.push("../test-assets.toml");
        let file_content = std::fs::read_to_string(config_path).unwrap();
        toml::from_str(&file_content).expect("Failed to parse test-assets.toml")
    })
}

pub fn download_asset(asset_key: &str) -> String {
    let assets = get_test_assets();
    let asset = assets
        .assets
        .get(asset_key)
        .unwrap_or_else(|| panic!("Asset '{}' not found in test-assets.toml", asset_key));

    let _ = dl_test_files_backoff(&[asset.clone()], ".", Duration::from_secs(60));

    asset.filepath.clone()
}
