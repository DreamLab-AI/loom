//! loom-facade — the composition root + axum router (RUST-ARCHITECTURE §9).
//! STAGE-1 PLACEHOLDER: Stage 2 replaces this file with the `AppState` port
//! bundle, the fusion pipeline (§6), the router, and the two deploy profiles.

fn main() {
    // Reference the domain crate so the dependency edge is real and compiles;
    // Stage 2 wires the actual axum server here.
    let _ = std::any::type_name::<loom_domain::Iri>();
    println!("loom-facade: stage-1 placeholder (see RUST-ARCHITECTURE §9)");
}
