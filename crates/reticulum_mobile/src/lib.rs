uniffi::setup_scaffolding!();

pub fn healthcheck() -> String {
    "reticulum-mobile-ready".to_string()
}
