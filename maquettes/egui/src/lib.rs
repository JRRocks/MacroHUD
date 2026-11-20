//! Trial crate of the « visuel » session. The main crate does not exist yet: this crate compiles
//! the HUD of `src/hud/` as it is, and `cargo test` renders it to PNG without a screen
//! (views a–f of the mock-up, then a comparison sheet laid out like `maquettes/variante-2.png`).

#[path = "../../../src/hud/mod.rs"]
pub mod hud;

#[cfg(test)]
mod planche;
