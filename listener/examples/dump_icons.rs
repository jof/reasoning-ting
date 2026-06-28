//! Dump the tray icon states as raw RGBA for preview:
//!   cargo run --example dump_icons -- /tmp/reasoning-ting-icons
fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "/tmp/reasoning-ting-icons".into());
    std::fs::create_dir_all(&dir).unwrap();
    reasoning_ting::icon::dump_rgba(&dir).unwrap();
    println!("wrote {}x{} icons to {dir}", reasoning_ting::icon::SIZE, reasoning_ting::icon::SIZE);
}
