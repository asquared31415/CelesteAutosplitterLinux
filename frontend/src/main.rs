#![feature(try_blocks)]

use std::{
    fmt::Debug,
    io::{self, BufRead, Write},
    thread,
    time::Duration,
};

use celeste_autosplit_tracer as cat;
use serde::{Deserialize, Serialize};
mod term;

use crate::term::TermColor;
#[derive(Debug, Serialize, Deserialize)]
pub struct Split {
    pub chapter: i32,
    pub kind: SplitKind,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum SplitKind {
    Level(String),
    Heart,
    Casette,
    Berries(i32),
}

impl Split {
    fn is_accomplished(&self, info: &cat::Dump) -> bool {
        match info.autosplitter_info.chapter == self.chapter {
            true => match &self.kind {
                SplitKind::Level(lvl) => lvl == &info.level_name(),
                SplitKind::Heart => info.autosplitter_info.chapter_heart,
                SplitKind::Casette => info.autosplitter_info.chapter_cassette,
                &SplitKind::Berries(bewwy_count) => {
                    bewwy_count == info.autosplitter_info.chapter_strawberries
                }
            },
            false => false,
        }
    }
}

#[derive(Debug)]
struct CurrentSplits {
    completed_splits: Vec<(Split, u64)>,
    todo_splits: Vec<Split>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Splits {
    splits: Vec<Split>,
}

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();

    stdout.lock().write(b"Path to splits:\n").unwrap();
    stdout.lock().flush().unwrap();
    let mut splits_path = String::new();
    stdin.lock().read_line(&mut splits_path).unwrap();
    let mut here = std::env::current_dir().unwrap();
    here.push(splits_path.trim_end());
    let splits: Splits = toml::from_str(&std::fs::read_to_string(here).unwrap()).unwrap();

    let mut splits = CurrentSplits {
        completed_splits: vec![],
        todo_splits: splits.splits,
    };

    let found_pid = cat::find_celeste();

    let pid = found_pid.unwrap_or_else(|e| {
        dbg!(e);
        stdout
            .lock()
            .write(b"Unable to find Celeste, please enter its PID: ")
            .unwrap();
        stdout.lock().flush().unwrap();

        let mut line = String::new();
        stdin.lock().read_line(&mut line).unwrap();

        line.trim_end()
            .parse::<i32>()
            .expect("enter a number u dingus")
    });

    let celeste = cat::Celeste::new(pid);

    loop {
        let dump = celeste.get_data();

        while let Some(split) = splits.todo_splits.first() {
            match split.is_accomplished(&dump) {
                true => {
                    let removed = splits.todo_splits.remove(0);
                    splits
                        .completed_splits
                        .push((removed, dump.autosplitter_info.chapter_time()));
                }
                false => break,
            }
        }

        term::clear();
        term::write(
            format!(
                "Chapter {} room {}\n",
                dump.autosplitter_info.chapter,
                dump.level_name()
            ),
            TermColor::Yellow,
            None,
        );
        term::write(
            format!("Chapter time: {}\n", dump.autosplitter_info.chapter_time()),
            TermColor::Green,
            None,
        );
        term::write(
            format!("File time: {}\n", dump.autosplitter_info.file_time()),
            TermColor::BrightMagenta,
            None,
        );
        term::write(
            format!("Deaths: {}\n", dump.death_count),
            TermColor::Red,
            None,
        );

        term::write(format!("{:?}\n", &splits), TermColor::White, None);

        thread::sleep(Duration::from_millis(12));
    }
}
