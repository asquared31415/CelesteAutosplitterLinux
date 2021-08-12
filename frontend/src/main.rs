#![deny(unsafe_op_in_unsafe_fn)]

use std::{
    fs::File,
    io::{self, BufRead, Write},
    process, thread,
    time::Duration,
};

use crate::{splits::CurrentSplits, term::ColorName, util::format_time};
use celeste_autosplit_tracer as cat;
use clap::{crate_version, App, Arg};
use dialoguer::{Input, Select, Sort};
use splits::{Split, SplitKind, Splits};

mod splits;
mod term;
mod util;

fn main() {
    let arg_matches = App::new("CelesteAutosplitter")
        .version(crate_version!())
        .arg_from_usage("[splits] -s --splits [path] 'the path to the splits file'")
        // currently broken :(
        //.arg_from_usage("[celeste] -c --celeste [path] 'the path to the celeste binary to automatically launch and trace without needing root'")
        .arg(
            Arg::with_name("edit-splits")
                .help("iteractive editor for the splits file")
                .short("e")
                .long("edit-splits")
                .conflicts_with("celeste"),
        )
        .get_matches();

    let stdin = io::stdin();
    let stdout = io::stdout();

    let path = if let Some(splits_path) = arg_matches.value_of("splits") {
        println!("Using passed split path: `{}`", splits_path);
        splits_path.to_string()
    } else {
        stdout.lock().write_all(b"Path to splits:\n").unwrap();
        stdout.lock().flush().unwrap();
        let mut input_path = String::new();
        stdin.lock().read_line(&mut input_path).unwrap();
        input_path.trim().to_string()
    };

    if arg_matches.is_present("edit-splits") {
        splits_menu(&path);
    } else {
        display_timer(&path);
    }
}

fn write_splits(splits: &Splits, splits_path: &str) {
    let splits_str = toml::to_string_pretty(&splits).expect("Failed to serialize");
    // TODO: keep backup first?
    // Create or truncate the file
    let mut file = File::create(splits_path).expect("Unable to open file");
    file.write_all(splits_str.as_bytes())
        .expect("Failed to write to file");
}

fn splits_menu(splits_path: &str) {
    let mut splits: Splits = toml::from_str(
        &std::fs::read_to_string(splits_path)
            .unwrap_or_else(|_| panic!("Unable to read splits file at `{}`", splits_path)),
    )
    .unwrap_or_else(|_| panic!("Unable to parse splits file `{}`", splits_path));

    loop {
        let selection = Select::new()
            .default(0)
            .with_prompt("Do what (press `q` to exit)")
            .items(&["Show Current", "Edit", "Done"])
            .interact_opt()
            .expect("Unable to display options");

        match selection {
            Some(0) => {
                term::writeln("Current Splits", ColorName::Cyan, None);
                for split in splits.splits.iter() {
                    println!("{}\n", split.display_long());
                }
            }
            Some(1) => edit_menu(&mut splits, splits_path),
            None | Some(2) => {
                break;
            }
            _ => {
                unreachable!("encountered an invalid selection")
            }
        }
    }
}

fn edit_menu(splits: &mut Splits, splits_path: &str) {
    loop {
        let choice = Select::new()
            .with_prompt("What to edit (press `q` to cancel)")
            .items(&["Add", "Delete", "Edit", "Move"])
            .default(0)
            .interact_opt()
            .expect("Unable to display options");

        match choice {
            Some(0) => {
                let name: String = Input::new()
                    .allow_empty(true)
                    .with_prompt("Split name (press enter to leave empty)")
                    .interact_text()
                    .expect("Unable to display prompt");
                let name = if name.is_empty() { None } else { Some(name) };

                let chapter: i32 = Input::new()
                    .with_prompt("What chapter is this split for?")
                    .interact_text()
                    .expect("Unable to display prompt");

                let kind_idx = Select::new()
                    .with_prompt("What sort of split is this?")
                    .default(0)
                    .items(&["Chapter Complete", "Heart", "Casette", "Berries", "Room"])
                    .interact()
                    .expect("Unable to display prompt");
                let kind = match kind_idx {
                    0 => SplitKind::ChapterComplete,
                    1 => SplitKind::Heart,
                    2 => SplitKind::Casette,
                    3 => {
                        let berries: i32 = Input::new()
                            .with_prompt("How many berries")
                            .interact_text()
                            .expect("Unable to display prompt");
                        SplitKind::Berries(berries)
                    }
                    4 => {
                        let room: String = Input::new()
                            .with_prompt("Room name")
                            .interact_text()
                            .expect("Unable to display prompt");
                        SplitKind::Level(room)
                    }
                    _ => {
                        unreachable!("encountered an invalid selection")
                    }
                };

                let split = Split {
                    name,
                    chapter,
                    split_kind: kind,
                };

                // TODO: sort splits
                splits.splits.push(split);
                write_splits(splits, splits_path);
            }
            Some(1) => {
                term::writeln(
                    "Which split do you want to remove? (press `q` to cancel)",
                    ColorName::BrightRed,
                    None,
                );
                let idx = Select::new()
                    .default(0)
                    .items(
                        &splits
                            .splits
                            .iter()
                            .map(|s| s.display_short())
                            .collect::<Vec<String>>(),
                    )
                    .interact_opt()
                    .expect("Unable to display prompt");

                if let Some(idx) = idx {
                    splits.splits.remove(idx);

                    write_splits(splits, splits_path);
                }
            }
            Some(2) => {
                term::writeln(
                    "Which split do you want to edit? (press `q` to cancel)",
                    ColorName::Blue,
                    None,
                );
                let idx = Select::new()
                    .default(0)
                    .items(
                        &splits
                            .splits
                            .iter()
                            .map(|s| s.display_short())
                            .collect::<Vec<String>>(),
                    )
                    .interact_opt()
                    .expect("Unable to display prompt");
                if let Some(idx) = idx {
                    loop {
                        let choice = Select::new()
                            .with_prompt("Edit what (press `q` to cancel)")
                            .default(0)
                            .items(&["Edit title", "Edit chapter", "Edit split kind"])
                            .interact_opt()
                            .expect("Unable to display prompt");

                        match choice {
                            Some(0) => {
                                let name: String = Input::new()
                                    .allow_empty(true)
                                    .with_prompt("Split name (press enter to leave empty)")
                                    .interact_text()
                                    .expect("Unable to display prompt");

                                splits.splits[idx].name =
                                    if name.is_empty() { None } else { Some(name) };
                            }
                            Some(1) => {
                                let chapter: i32 = Input::new()
                                    .with_prompt("What chapter is this split for?")
                                    .interact_text()
                                    .expect("Unable to display prompt");

                                splits.splits[idx].chapter = chapter;
                            }
                            Some(2) => {
                                let kind_idx = Select::new()
                                    .with_prompt("What sort of split is this?")
                                    .default(0)
                                    .items(&[
                                        "Chapter Complete",
                                        "Heart",
                                        "Casette",
                                        "Berries",
                                        "Room",
                                    ])
                                    .interact()
                                    .expect("Unable to display prompt");
                                let kind = match kind_idx {
                                    0 => SplitKind::ChapterComplete,
                                    1 => SplitKind::Heart,
                                    2 => SplitKind::Casette,
                                    3 => {
                                        let berries: i32 = Input::new()
                                            .with_prompt("How many berries")
                                            .interact_text()
                                            .expect("Unable to display prompt");
                                        SplitKind::Berries(berries)
                                    }
                                    4 => {
                                        let room: String = Input::new()
                                            .with_prompt("Room name")
                                            .interact_text()
                                            .expect("Unable to display prompt");
                                        SplitKind::Level(room)
                                    }
                                    _ => {
                                        unreachable!("encountered an invalid selection")
                                    }
                                };

                                splits.splits[idx].split_kind = kind;
                            }
                            None => break,
                            _ => {
                                unreachable!("encountered an invalid selection")
                            }
                        }
                    }

                    write_splits(splits, splits_path);
                }
            }
            Some(3) => {
                let sorted = Sort::new()
                    .with_prompt(
                        "Use spacebar and arrow keys to reorder splits.  Press enter when finished",
                    )
                    .items(
                        &splits
                            .splits
                            .iter()
                            .map(|s| s.display_short())
                            .collect::<Vec<String>>(),
                    )
                    .interact()
                    .expect("Unable to display prompt");

                splits.splits = sorted
                    .into_iter()
                    .map(|idx| splits.splits[idx].clone())
                    .collect();

                write_splits(splits, splits_path);
            }
            None => break,
            _ => {
                unreachable!("encountered an invalid selection")
            }
        }
    }
}

fn display_timer(splits_path: &str) {
    let stdin = io::stdin();
    let stdout = io::stdout();

    let splits: Splits = toml::from_str(
        &std::fs::read_to_string(splits_path)
            .unwrap_or_else(|_| panic!("Unable to read splits file at `{}`", splits_path)),
    )
    .unwrap_or_else(|_| panic!("Unable to parse splits file `{}`", splits_path));

    let mut splits = CurrentSplits {
        completed_splits: vec![],
        todo_splits: splits.splits,
    };

    let celeste_pid = cat::find_celeste();

    let pid = celeste_pid.unwrap_or_else(|_| {
        stdout
            .lock()
            .write_all(b"Unable to find Celeste, please enter its PID: ")
            .unwrap();
        stdout.lock().flush().unwrap();

        loop {
            let mut line = String::new();
            stdin.lock().read_line(&mut line).unwrap();

            match line.trim_end().parse::<u32>() {
                Ok(pid) => return pid,
                Err(_) => {
                    stdout.lock().write_all(b"Please enter a number: ").unwrap();
                    stdout.lock().flush().unwrap();
                }
            }
        }
    });

    let celeste = cat::Celeste::new(pid);

    term::clear();
    term::writeln(
        "Starting autosplitter.  Press `q` at any time to exit.",
        None,
        None,
    );

    thread::sleep(Duration::from_secs(5));

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
            ColorName::Yellow,
            None,
        );

        term::writeln(
            format!(
                "Chapter time: {}",
                format_time(Duration::from_millis(dump.autosplitter_info.chapter_time()))
            ),
            ColorName::Green,
            None,
        );

        term::writeln(
            format!(
                "File time: {}",
                format_time(Duration::from_millis(dump.autosplitter_info.file_time()))
            ),
            ColorName::BrightMagenta,
            None,
        );
        term::writeln(
            format!("Deaths: {}", dump.death_count),
            ColorName::Red,
            None,
        );

        term::writeln(
            "\n################\nCompleted Splits\n################\n",
            ColorName::White,
            ColorName::Gray,
        );

        for split in splits.completed_splits.iter() {
            term::writeln(split.0.display_complete(split.1), ColorName::White, None);
        }

        term::writeln(
            "\n###########\nTODO Splits\n###########\n",
            ColorName::White,
            ColorName::Gray,
        );

        for split in splits.todo_splits.iter() {
            term::writeln(split.display_incomplete(&dump), ColorName::White, None);
        }

        thread::sleep(Duration::from_millis(12));
    }
}
