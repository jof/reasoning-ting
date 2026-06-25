//! Dump the tray icon states as raw RGBA for preview:
//!   cargo run --example dump_icons -- /tmp/someting-icons
fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "/tmp/someting-icons".into());
    std::fs::create_dir_all(&dir).unwrap();
    some_ting::icon::dump_rgba(&dir).unwrap();
    println!("wrote {}x{} icons to {dir}", some_ting::icon::SIZE, some_ting::icon::SIZE);
}
