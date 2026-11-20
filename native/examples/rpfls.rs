//! Lists an archive of the game's (to find a file in it), or prints a text file from one.
//! usage: rpfls <archive, from the game folder> [filter] [--print]
use gmsv_delirium::game::Game;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let g = Game::open(std::path::Path::new(r"J:\SteamLibrary\steamapps\common\Grand Theft Auto IV\GTAIV")).expect("open");
    let filter = a.get(2).cloned().unwrap_or_default().to_ascii_lowercase();
    if a.iter().any(|x| x == "--print") {
        let t = g.archive_text(&a[1], &filter).unwrap_or_else(|e| e);
        let grep = a.get(4).cloned().unwrap_or_default();
        let mut on = grep.is_empty();
        for l in t.lines() {
            if !grep.is_empty() && l.contains(&grep) { on = true; }
            if on { println!("{l}"); }
            if on && !grep.is_empty() && l.trim() == "}" { on = false; }
        }
        return;
    }
    match g.archive_files(&a[1]) {
        Ok(list) => list.iter().filter(|p| p.to_ascii_lowercase().contains(&filter)).for_each(|p| println!("{p}")),
        Err(e) => println!("{e}"),
    }
}
