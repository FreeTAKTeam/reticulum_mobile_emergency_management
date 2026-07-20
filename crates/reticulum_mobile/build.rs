fn main() -> Result<(), Box<dyn std::error::Error>> {
    uniffi::generate_scaffolding("./src/reticulum_mobile.udl")?;
    Ok(())
}
