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

    let pid = if found_pid != -1 {
        found_pid
    } else {
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
    };

    const OUTPUT_FILE: &str = "autosplitterinfo";
    let mut output = File::create(OUTPUT_FILE).expect("Could not create output file");

    let mut mem_file = celeste_autosplit_tracer::load_mem(pid);
    let celeste = Celeste::new(&mut mem_file);

    loop {
        let dump = celeste.get_data(&mut mem_file);

        output
            .seek(SeekFrom::Start(0))
            .expect("Unable to overwrite file");

        let data = dump.as_bytes();
        output.write_all(&data).expect("Unable to overwrite file");

        thread::sleep(Duration::from_millis(12));
    }
}