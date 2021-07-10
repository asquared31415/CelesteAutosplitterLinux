use std::{
    fs::File,
    io::{self, BufRead, Seek, SeekFrom, Write},
    thread,
    time::Duration,
};

mod term;

use celeste_autosplit_tracer::Celeste;

use crate::term::TermColor;

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();

    let found_pid = celeste_autosplit_tracer::find_celeste();

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

    let celeste = Celeste::new(pid);

    loop {
        let dump = celeste.get_data();

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

        thread::sleep(Duration::from_millis(12));
    }
}
