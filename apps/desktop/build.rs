//! Tauri's build step, only when the window itself is being built (the `app` feature).

fn main() {
    #[cfg(feature = "app")]
    tauri_build::build();
}
