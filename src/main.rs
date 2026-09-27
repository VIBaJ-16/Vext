use std::env;

mod app;
mod config;

use app::App;

fn main() {
    let args = env::args().collect::<Vec<_>>();
    ratatui::run(|terminal| App::new(&args[1]).unwrap().run(terminal)).unwrap();
}