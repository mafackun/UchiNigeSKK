extern crate termion;

pub mod state;
pub mod key;
pub mod buffer;
pub mod engine;
pub mod romaji;
pub mod tables;
pub mod frontend;
pub mod jisyo;

use std::{env, panic};
use std::io::{Result, Write, stdout};

fn install_panic_hook() {
    panic::set_hook(Box::new(|info| {
        let mut out = stdout();
        let _ = write!(out, "{}{}", termion::screen::ToMainScreen, termion::clear::All);
        let _ = out.flush();
        eprintln!("panic: {}", info);
    }));
}

fn main() -> Result<()> {
    install_panic_hook();
    let msg = |s| { let mut msg = String::from("no environment variable: "); msg.push_str(s); msg };
    let (ct, cf, j) = ("CPY_TO", "CPY_FROM", "JISYO_PATH");
    let (ct, cf, j) = (
        env::var(ct).expect(&msg(&ct)), env::var(cf).expect(&msg(&cf)), env::var(j).expect(&msg(&j)) 
    );
    let jisyo = crate::jisyo::Jisyo::load(&j);
    frontend::run(jisyo?, &ct, &cf)
}

