use std::{
    fs::File,
    io::{self, BufRead, Seek, SeekFrom, Write},
    thread,
    time::Duration,
};

use celeste_autosplit_tracer::Celeste;

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

    const OUTPUT_FILE: &str = "autosplitterinfo";
    let mut output = File::create(OUTPUT_FILE).expect("Could not create output file");

    let celeste = Celeste::new(pid);

    loop {
        let dump = celeste.get_data();

        output
            .seek(SeekFrom::Start(0))
            .expect("Unable to overwrite file");

        let data = dump.as_bytes();
        output.write_all(&data).expect("Unable to overwrite file");

        thread::sleep(Duration::from_millis(12));
    }
}
