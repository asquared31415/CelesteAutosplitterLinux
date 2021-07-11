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
    pub name: Option<String>,
    pub chapter: i32,
    pub split_kind: SplitKind,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "kind_data")]
pub enum SplitKind {
    Heart,
    Casette,
    Berries(i32),
    Level(String),
}

impl Split {
    fn is_accomplished(&self, info: &cat::Dump) -> bool {
        match info.autosplitter_info.chapter == self.chapter {
            true => match &self.split_kind {
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

    fn display_incomplete(&self, info: &cat::Dump) -> String {
        if let Some(name) = &self.name {
            return name.clone();
        }

        match &self.split_kind {
            SplitKind::Berries(num_berries) => {
                format!(
                    "{}/{} Berries",
                    info.autosplitter_info.file_strawberries, num_berries
                )
            }
            _ => {
                let ch = self.chapter.to_string();
                let split_kind = match &self.split_kind {
                    SplitKind::Level(level) => &level,
                    SplitKind::Heart => "Heart",
                    SplitKind::Casette => "Casette",
                    _ => unreachable!(),
                };
                format!("Ch.{}: {}", ch, split_kind,)
            }
        }
    }

    fn display_complete(&self, finish_time: u64) -> String {
        let finish_time = std::time::Duration::from_millis(finish_time);
        if let Some(name) = &self.name {
            return format!("{} = {:#?}", name.clone(), finish_time);
        }

        match &self.split_kind {
            SplitKind::Berries(num_berries) => {
                format!(
                    "{}/{} Berries = {:#?}",
                    num_berries, num_berries, finish_time
                )
            }
            _ => {
                let ch = self.chapter.to_string();
                let split_kind = match &self.split_kind {
                    SplitKind::Level(level) => &level,
                    SplitKind::Heart => "Heart",
                    SplitKind::Casette => "Casette",
                    _ => unreachable!(),
                };
                format!("Ch.{}: {} = {:#?}", ch, split_kind, finish_time)
            }
        }
    }
}

#[derive(Debug)]
struct CurrentSplits {
    completed_splits: Vec<(Split, u64)>,
    todo_splits: Vec<Split>,
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

    #[derive(Debug, Serialize, Deserialize)]
    struct Splits {
        split_mode: (String, i32),
        splits: Vec<Split>,
    }

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
        term::writeln(
            format!(
                "Chapter {} room {}",
                dump.autosplitter_info.chapter,
                dump.level_name()
            ),
            TermColor::Yellow,
            None,
        );
        term::writeln(
            format!("Chapter time: {}", dump.autosplitter_info.chapter_time()),
            TermColor::Green,
            None,
        );
        term::writeln(
            format!("File time: {}", dump.autosplitter_info.file_time()),
            TermColor::BrightMagenta,
            None,
        );
        term::writeln(
            format!("Deaths: {}", dump.death_count),
            TermColor::Red,
            None,
        );

        term::writeln(
            "\n################\nCompleted Splits\n################\n",
            TermColor::White,
            TermColor::Gray,
        );

        for split in splits.completed_splits.iter() {
            term::writeln(split.0.display_complete(split.1), TermColor::White, None);
        }

        term::writeln(
            "\n###########\nTODO Splits\n###########\n",
            TermColor::White,
            TermColor::Gray,
        );

        for split in splits.todo_splits.iter() {
            term::writeln(split.display_incomplete(&dump), TermColor::White, None);
        }

        thread::sleep(Duration::from_millis(12));
    }
}
