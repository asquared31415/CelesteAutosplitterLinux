use std::io::{self, BufRead, Write};

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

    let mem_file = celeste_autosplit_tracer::load_mem(pid);

    const OUTPUT_FILE: &str = "autosplitterinfo";

    celeste_autosplit_tracer::dump_info_loop(OUTPUT_FILE, mem_file);
}
