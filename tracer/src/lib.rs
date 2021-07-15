#![feature(const_evaluatable_checked, const_generics)]
#![allow(incomplete_features)]
#![deny(unsafe_op_in_unsafe_fn)]

use std::{
    fs::{self, File},
    io::{Seek, SeekFrom, Write},
    mem,
    path::Path,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
    usize,
};

mod tracer;
use crate::tracer::*;

#[cfg(not(target_os = "linux"))]
compile_error!("This program does not support non-linux OSes, please use a Linux OS :)");

#[derive(Clone, Copy, Debug)]
pub enum PIDError {
    NotFound,
    IOError,
}

pub fn find_celeste() -> Result<i32, PIDError> {
    for dir in fs::read_dir(Path::new("/proc/")).unwrap() {
        if let Ok(dir) = dir {
            if let Ok(file_type) = dir.file_type() {
                if file_type.is_dir() {
                    let name = dir.file_name().into_string().unwrap();
                    if name.chars().all(|c| ('0'..='9').contains(&c)) {
                        if let Ok(path) = fs::read_link(&format!("/proc/{}/exe", name)) {
                            if path
                                .into_os_string()
                                .into_string()
                                .unwrap()
                                .contains("Celeste.bin.x86_64")
                            {
                                return Ok(str::parse(&name).unwrap());
                            }
                        }
                    }
                }
            } else {
                return Err(PIDError::IOError);
            }
        } else {
            return Err(PIDError::IOError);
        }
    }

    Err(PIDError::NotFound)
}

#[derive(Debug)]
pub struct Celeste {
    assembly: usize,
    class_cache: usize,
    celeste_class: usize,
    savedata_class: usize,
    engine_class: usize,
    level_class: usize,
    instance: usize,
    autosplitter_info: usize,
}

impl Celeste {
    fn init(pid: i32) -> usize {
        MEM_FILE.get_or_init(|| Arc::new(Mutex::new(Some(load_mem(pid)))));

        unsafe {
            //let root_domain_ptr = read_u64(0xA17650, &mut mem_file) as usize;
            let domains_list = read_u64(0xA17698) as usize;

            let first_domain = read_u64(domains_list) as usize;
            let first_domain_name_ptr = read_u64(first_domain + 0xD8) as usize;
            let first_domain_name = read_string(first_domain_name_ptr as usize);

            if first_domain_name != "Celeste.exe" {
                panic!("This is not celeste!");
            }

            let second_domain = read_u64(domains_list + 8) as usize;
            let second_domain_name_ptr = read_u64(second_domain + 0xD8) as usize;
            // TODO: this could probably cause Bad Things and spicy UB if it doesn't exist
            // but it does so
            let second_domain_name = read_string(second_domain_name_ptr as usize);

            println!("Connected to {}", second_domain_name);
            // TODO: fallback to first domain?
            second_domain
        }
    }

    pub fn new(pid: i32) -> Self {
        let domain = Self::init(pid);
        unsafe {
            let assembly = read_u64(domain + 0xD0) as usize;
            let image = read_u64(assembly + 0x60) as usize;
            let class_cache = image + 1216;
            let celeste_class = lookup_class(class_cache, "Celeste");
            let savedata_class = lookup_class(class_cache, "SaveData");
            let engine_class = lookup_class(class_cache, "Engine");
            let level_class = lookup_class(class_cache, "Level");

            let instance = static_field_u64(celeste_class as usize, "Instance") as usize;
            let autosplitter_info = locate_autosplitter_info(instance);

            Celeste {
                assembly,
                class_cache,
                celeste_class,
                savedata_class,
                engine_class,
                level_class,
                instance,
                autosplitter_info,
            }
        }
    }

    pub fn get_data(&self) -> Dump {
        unsafe {
            let asi: AutosplitterInfo = MemPtr::new(self.autosplitter_info).read();

            let mut dump = Dump {
                autosplitter_info: asi,
                ..Default::default()
            };

            let savedata_ptr = static_field_u64(self.savedata_class, "Instance") as usize;
            if savedata_ptr != 0 {
                // TODO: reimplmement this w/ result maybe?
                /*
                if savedata_ptr != last_savedata_ptr {
                    // TODO: sleep here to give time to save?
                    last_savedata_ptr = savedata_ptr;
                    continue;
                }
                */

                dump.death_count = instance_field_u32(savedata_ptr, "TotalDeaths");

                if asi.chapter == -1 {
                    // mode stats = 0?
                } else {
                    let areas = instance_field_u64(savedata_ptr, "Areas") as usize;
                    if instance_field_u32(areas, "_size") == 11 {
                        let areas_ptr = instance_field_u64(areas, "_items") as usize;
                        let area_stats =
                            read_u64(areas_ptr + 0x20 + 8 * asi.chapter as usize) as usize;
                        let mode_arr = instance_field_u64(area_stats, "Modes") as usize + 0x20;
                        let mode_stats = read_u64(mode_arr + 8 * asi.mode as usize) as usize;
                        if mode_stats == 0 {
                            dump.chapter_checkpoints = 0;
                        } else {
                            let checkpoints =
                                instance_field_u64(mode_stats, "Checkpoints") as usize;
                            dump.chapter_checkpoints = instance_field_u32(checkpoints, "_count");
                        }
                    } else {
                        eprintln!("Failed to get areas array");
                    }
                }
            }

            if asi.chapter == -1 || !asi.chapter_started || asi.chapter_complete {
                dump.in_cutscene = false;
            } else {
                let scene = read_u64(self.instance + class_field_offset(self.engine_class, "scene"))
                    as usize;
                if instance_class(scene) == self.level_class {
                    dump.in_cutscene =
                        read_u8(scene + class_field_offset(self.level_class, "InCutscene")) != 0;
                } else {
                    dump.in_cutscene = false;
                }
            }

            dump
        }
    }
}

impl Drop for Celeste {
    fn drop(&mut self) {
        if let Some(f) = MEM_FILE.get() {
            *f.lock().expect("Unable to lock mem file") = None;
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct AutosplitterInfo {
    // ptr to a boxed string of the level (room) name
    level: u64,

    pub chapter: i32,
    pub mode: i32,
    pub timer_active: bool,
    pub chapter_started: bool,
    pub chapter_complete: bool,

    // For some reason this is a u64 in miliseconds times 10_000 (sec * 10_000_000)
    chapter_time: u64,

    // These values correspond to the current play through of the chapter
    // (even if you have collected something here, it's not counted here if you restart)
    pub chapter_strawberries: i32,
    pub chapter_cassette: bool,
    pub chapter_heart: bool,

    // For some reason this is a u64 in miliseconds times 10_000 (sec * 10_000_000)
    file_time: u64,

    pub file_strawberries: i32,
    pub file_cassettes: i32,
    pub file_hearts: i32,
}

impl AutosplitterInfo {
    /// Returns the chapter time in milliseconds
    pub fn chapter_time(&self) -> u64 {
        self.chapter_time / 10_000
    }

    /// Returns the file time in milliseconds
    pub fn file_time(&self) -> u64 {
        self.file_time / 10_000
    }
}

#[derive(Clone, Debug, Default)]
pub struct Dump {
    pub autosplitter_info: AutosplitterInfo,

    // The number of checkpoints that have ever been collected in the current chapter
    pub chapter_checkpoints: u32,

    pub in_cutscene: bool,
    pub death_count: u32,
}

impl Dump {
    pub fn level_name(&self) -> String {
        if self.autosplitter_info.level == 0 {
            String::new()
        } else {
            read_boxed_string(self.autosplitter_info.level as usize)
        }
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(176);
        let asi: [u8; mem::size_of::<AutosplitterInfo>()] =
            unsafe { mem::transmute(self.autosplitter_info) };
        data.extend(asi);
        data.extend(self.chapter_checkpoints.to_ne_bytes());
        data.push(self.in_cutscene as u8);
        data.extend([0_u8; 3]);
        data.extend(self.death_count.to_ne_bytes());

        let level_name = self.level_name();
        data.extend(level_name.bytes());
        // padding
        data.extend(vec![0; 100 - level_name.len()]);

        data
    }
}

pub fn dump_info_loop(output_file: &str, pid: i32) {
    let mut output = File::create(output_file).expect("Could not create output file");
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
