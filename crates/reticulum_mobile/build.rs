fn main() {
    uniffi::generate_scaffolding("./src/reticulum_mobile.udl")
        .expect("failed to generate UniFFI scaffolding");
}
