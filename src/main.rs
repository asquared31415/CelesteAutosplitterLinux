#![feature(const_evaluatable_checked, const_generics)]
#![allow(incomplete_features)]
#![deny(unsafe_op_in_unsafe_fn)]

use std::{
    fs::{self, File},
    io::{self, BufRead, Read, Seek, SeekFrom, Write},
    mem,
    path::{Path, PathBuf},
    thread,
    time::{self, Duration},
    usize,
};

struct MemPtr(usize);

impl MemPtr {
    pub fn new(addr: usize) -> Self {
        Self(addr)
    }

    // SAFETY: a T must be valid at the specified offset (basically ptr read)
    pub unsafe fn read<T>(&self, mem_file: &mut File) -> T
    where
        T: Copy,
        [(); mem::size_of::<T>()]: ,
    {
        mem_file
            .seek(SeekFrom::Start(self.0 as u64))
            .expect("Unable to read memory");
        let mut buf = [0_u8; mem::size_of::<T>()];
        mem_file
            .read_exact(&mut buf)
            .expect(&format!("Unable to read memory at {:#X}", self.0));
        unsafe { *(buf.as_ptr() as *const T) }
    }

    // SAFETY: a T must be valid at the specified offset (basically ptr read)
    // the provided pointer must be valid for writes for the specified number of writes of size T
    pub unsafe fn read_into<T>(&self, out: &mut [T], mem_file: &mut File)
    where
        T: Copy,
    {
        mem_file
            .seek(SeekFrom::Start(self.0 as u64))
            .expect("Unable to read memory");

        let count = out.len();
        let mut out = unsafe {
            std::slice::from_raw_parts_mut(out as *mut [T] as *mut u8, count * mem::size_of::<T>())
        };
        mem_file
            .read_exact(&mut out)
            .expect(&format!("Unable to read memory at {:#X}", self.0));
    }
}

unsafe fn read_u64(addr: usize, mem_file: &mut File) -> u64 {
    unsafe { MemPtr::new(addr).read::<u64>(mem_file) }
}

unsafe fn read_u32(addr: usize, mem_file: &mut File) -> u32 {
    unsafe { MemPtr::new(addr).read::<u32>(mem_file) }
}

unsafe fn read_u8(addr: usize, mem_file: &mut File) -> u8 {
    unsafe { MemPtr::new(addr).read::<u8>(mem_file) }
}

unsafe fn read_string(addr: usize, mem_file: &mut File) -> String {
    unsafe {
        let mut buf = vec![0_u8; 100];
        MemPtr::new(addr).read_into(&mut buf, mem_file);
        buf.set_len(100);
        let data = buf.into_iter().take_while(|&c| c != 0).collect::<Vec<_>>();
        String::from_utf8_unchecked(data)
    }
}

pub fn read_boxed_string(instance: usize, mem_file: &mut File) -> String {
    unsafe {
        let class = instance_class(instance, mem_file);
        let data_offset = class_field_offset(class, "m_firstChar", mem_file);
        let size_offset = class_field_offset(class, "m_stringLength", mem_file);
        let size = read_u32(instance + size_offset, mem_file) as usize;

        let mut utf16 = vec![0_u16; size];
        MemPtr::new(instance + data_offset).read_into(&mut utf16, mem_file);
        utf16.set_len(size);
        String::from_utf16_lossy(&utf16)
    }
}

unsafe fn class_name(class: usize, mem_file: &mut File) -> String {
    unsafe {
        let name_ptr = read_u64(class + 0x40, mem_file) as usize;
        read_string(name_ptr as usize, mem_file)
    }
}

pub unsafe fn lookup_class<S: AsRef<str>>(
    class_cache: usize,
    name: S,
    mem_file: &mut File,
) -> usize {
    let target_name = name.as_ref();
    unsafe {
        let cache_table = read_u64(class_cache + 0x20, mem_file) as usize;
        let hash_table_size = read_u32(cache_table + 0x18, mem_file) as usize;

        for bucket in 0..hash_table_size {
            let mut class = read_u64(cache_table + 8 * bucket, mem_file) as usize;
            while class != 0 {
                let class_name = class_name(class, mem_file);
                if class_name == target_name {
                    return class as usize;
                }

                class = read_u64(class + 0xF8, mem_file) as usize;
            }
        }

        panic!("Could not find class {}", target_name);
    }
}

unsafe fn instance_class(instance: usize, mem_file: &mut File) -> usize {
    unsafe {
        read_u64(
            read_u64(instance, mem_file) as usize & (!1_i32 as usize),
            mem_file,
        ) as usize
    }
}

unsafe fn class_static_fields(class: usize, mem_file: &mut File) -> u64 {
    unsafe {
        let vtable_size = read_u32(class + 0x54, mem_file);
        let runtime_info = read_u64(class + 0xC8, mem_file);
        let max_domains = read_u64(runtime_info as usize, mem_file) as usize;

        for i in 0..=max_domains {
            let vtable = read_u64(runtime_info as usize + 8 + 8 * i, mem_file);
            if vtable != 0 {
                return read_u64(vtable as usize + 64 + 8 * vtable_size as usize, mem_file);
            }
        }

        panic!("No domain has class {:#X} loaded", class);
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum MonoTypeKind {
    MonoClassDef = 1,
    MonoClassGTD = 2,
    MonoClassGInst = 3,
    MonoClassGParam = 4,
    MonoClassArray = 5,
    MonoClassPointer = 6,
}

impl MonoTypeKind {
    pub fn to_u8(&self) -> u8 {
        unsafe { mem::transmute(*self) }
    }

    pub fn from_u8(v: u8) -> Self {
        assert!(v >= 1 && v <= 6, "Value out of range");
        unsafe { mem::transmute(v) }
    }
}

fn class_kind(class: usize, mem_file: &mut File) -> MonoTypeKind {
    unsafe { MonoTypeKind::from_u8(read_u8(class + 0x24, mem_file) & 7) }
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct MonoClassField {
    t: u64,
    name: u64,
    parent: u64,
    offset: u32,
}

unsafe fn class_field_offset(class: usize, name: &str, mem_file: &mut File) -> usize {
    let kind = class_kind(class, mem_file);
    unsafe {
        match kind {
            MonoTypeKind::MonoClassGInst => class_field_offset(
                read_u64(read_u64(class + 0xE0, mem_file) as usize, mem_file) as usize,
                name,
                mem_file,
            ),
            MonoTypeKind::MonoClassDef | MonoTypeKind::MonoClassGTD => {
                let num_fields = read_u32(class + 0xF0, mem_file);
                let fields_ptr = read_u64(class + 0x90, mem_file);

                for i in 0..num_fields as usize {
                    let field: MonoClassField =
                        MemPtr::new(fields_ptr as usize + i * mem::size_of::<MonoClassField>())
                            .read(mem_file);
                    let nametest = read_string(field.name as usize, mem_file);
                    if name == nametest {
                        return field.offset as usize;
                    }
                }

                panic!("Failed to find name {}", name);
            }
            _ => {
                panic!("Something is wrong");
            }
        }
    }
}

pub unsafe fn static_field_u64<S: AsRef<str>>(class: usize, name: S, mem_file: &mut File) -> u64 {
    unsafe {
        let static_data = class_static_fields(class, mem_file);
        let field_offset = class_field_offset(class, name.as_ref(), mem_file);
        read_u64(static_data as usize + field_offset, mem_file)
    }
}

pub unsafe fn instance_field_u32<S: AsRef<str>>(
    instance: usize,
    name: S,
    mem_file: &mut File,
) -> u32 {
    unsafe {
        let class = instance_class(instance, mem_file);
        let field_offset = class_field_offset(class, name.as_ref(), mem_file);
        read_u32(instance + field_offset, mem_file)
    }
}

pub unsafe fn instance_field_u64<S: AsRef<str>>(
    instance: usize,
    name: S,
    mem_file: &mut File,
) -> u64 {
    unsafe {
        let class = instance_class(instance, mem_file);
        let field_offset = class_field_offset(class, name.as_ref(), mem_file);
        read_u64(instance + field_offset, mem_file)
    }
}

#[derive(Debug)]
struct Celeste {
    assembly: usize,
    class_cache: usize,
    celeste_class: usize,
    savedata_class: usize,
    engine_class: usize,
    level_class: usize,
    instance: usize,
}

impl Celeste {
    pub fn new(domain: usize, mem_file: &mut File) -> Self {
        unsafe {
            let assembly = read_u64(domain + 0xD0, mem_file) as usize;
            let image = read_u64(assembly + 0x60, mem_file) as usize;
            let class_cache = image + 1216;
            let celeste_class = lookup_class(class_cache, "Celeste", mem_file);
            let savedata_class = lookup_class(class_cache, "SaveData", mem_file);
            let engine_class = lookup_class(class_cache, "Engine", mem_file);
            let level_class = lookup_class(class_cache, "Level", mem_file);

            let instance = static_field_u64(celeste_class as usize, "Instance", mem_file) as usize;

            Celeste {
                assembly,
                class_cache,
                celeste_class,
                savedata_class,
                engine_class,
                level_class,
                instance,
            }
        }
    }
}

fn load_mem(pid: i32) -> File {
    let path = PathBuf::from(format!("/proc/{}/mem", pid));
    File::open(path).expect(&format!("Unable to open mem file for process {}", pid))
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct AutosplitterInfo {
    level: u64,
    chapter: i32,
    mode: i32,
    timer_active: bool,
    chapter_started: bool,
    chapter_complete: bool,
    chapter_time: u64,
    chapter_strawberries: i32,
    chapter_cassette: bool,
    chapter_heart: bool,
    file_time: u64,
    file_strawberries: i32,
    file_cassettes: i32,
    file_hearts: i32,
}

#[derive(Clone, Debug, Default)]
pub struct Dump {
    autosplitter_info: AutosplitterInfo,
    current_checkpoints: u32,

    in_cutscene: bool,
    pad: [u8; 3],

    death_count: u32,
    level_name: String,
}

impl Dump {
    pub fn as_bytes(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(176);
        let asi: [u8; mem::size_of::<AutosplitterInfo>()] =
            unsafe { mem::transmute(self.autosplitter_info) };
        data.extend(asi);
        data.extend(self.current_checkpoints.to_ne_bytes());
        data.push(self.in_cutscene as u8);
        data.extend(self.pad);
        data.extend(self.death_count.to_ne_bytes());

        data.extend(self.level_name.bytes());
        for _ in 0..(100 - self.level_name.len()) {
            data.push(0);
        }

        data
    }
}

fn locate_autosplitter_info(celeste: &Celeste, mem_file: &mut File) -> usize {
    unsafe { instance_field_u64(celeste.instance, "AutoSplitterInfo", mem_file) as usize + 0x10 }
}

fn dump_info_loop(mut mem_file: File) {
    const OUTPUT_FILE: &str = "autosplitterinfo";

    let mut output = File::create(OUTPUT_FILE).expect("Could not create output file");
    unsafe {
        let root_domain_ptr = read_u64(0xA17650, &mut mem_file) as usize;
        let domains_list = read_u64(0xA17698, &mut mem_file) as usize;

        let first_domain = read_u64(domains_list, &mut mem_file) as usize;
        let first_domain_name_ptr = read_u64(first_domain + 0xD8, &mut mem_file) as usize;
        let first_domain_name = read_string(first_domain_name_ptr as usize, &mut mem_file);

        if first_domain_name != "Celeste.exe" {
            panic!("This is not celeste!");
        }

        let second_domain = read_u64(domains_list + 8, &mut mem_file) as usize;
        let second_domain_name_ptr = read_u64(second_domain + 0xD8, &mut mem_file) as usize;
        // TODO: this could probably cause Bad Things and spicy UB if it doesn't exist
        // but it does so
        let second_domain_name = read_string(second_domain_name_ptr as usize, &mut mem_file);

        println!("Connected to {}", second_domain_name);
        let celeste_domain = second_domain;

        let celeste = Celeste::new(celeste_domain, &mut mem_file);

        let asi_ptr = locate_autosplitter_info(&celeste, &mut mem_file);
        let mut last_savedata_ptr = 0;
        loop {
            let start = time::Instant::now();

            let asi: AutosplitterInfo = MemPtr::new(asi_ptr).read(&mut mem_file);

            let mut dump = Dump {
                autosplitter_info: asi,
                ..Default::default()
            };

            if asi.level != 0 {
                dump.level_name = read_boxed_string(asi.level as usize, &mut mem_file);
            }

            let savedata_ptr =
                static_field_u64(celeste.savedata_class, "Instance", &mut mem_file) as usize;
            if savedata_ptr != 0 {
                if savedata_ptr != last_savedata_ptr {
                    // TODO: sleep here to give time to save?
                    last_savedata_ptr = savedata_ptr;
                    continue;
                }

                dump.death_count = instance_field_u32(savedata_ptr, "TotalDeaths", &mut mem_file);

                if asi.chapter == -1 {
                    // mode stats = 0?
                } else {
                    let areas = instance_field_u64(savedata_ptr, "Areas", &mut mem_file) as usize;
                    if instance_field_u32(areas, "_size", &mut mem_file) == 11 {
                        let areas_ptr = instance_field_u64(areas, "_items", &mut mem_file) as usize;
                        let area_stats =
                            read_u64(areas_ptr + 0x20 + 8 * asi.chapter as usize, &mut mem_file)
                                as usize;
                        let mode_arr =
                            instance_field_u64(area_stats, "Modes", &mut mem_file) as usize + 0x20;
                        let mode_stats =
                            read_u64(mode_arr + 8 * asi.mode as usize, &mut mem_file) as usize;
                        if mode_stats == 0 {
                            dump.current_checkpoints = 0;
                        } else {
                            let checkpoints =
                                instance_field_u64(mode_stats, "Checkpoints", &mut mem_file)
                                    as usize;
                            dump.current_checkpoints =
                                instance_field_u32(checkpoints, "_count", &mut mem_file);
                        }
                    } else {
                        eprintln!("Failed to get areas array");
                    }
                }
            }

            if asi.chapter == -1 {
                dump.in_cutscene = false;
            } else {
                if !asi.chapter_started || asi.chapter_complete {
                    dump.in_cutscene = false;
                } else {
                    let scene = read_u64(
                        celeste.instance
                            + class_field_offset(celeste.engine_class, "scene", &mut mem_file),
                        &mut mem_file,
                    ) as usize;
                    if instance_class(scene, &mut mem_file) == celeste.level_class {
                        dump.in_cutscene = read_u8(
                            scene
                                + class_field_offset(
                                    celeste.level_class,
                                    "InCutscene",
                                    &mut mem_file,
                                ),
                            &mut mem_file,
                        ) != 0;
                    } else {
                        dump.in_cutscene = false;
                    }
                }
            }

            //dbg!(&dump);
            output
                .seek(SeekFrom::Start(0))
                .expect("Unable to overwrite file");

            let data = dump.as_bytes();
            output.write_all(&data).expect("Unable to overwrite file");

            thread::sleep(Duration::from_millis(12));
        }
    }
}

fn find_celeste() -> i32 {
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
                                return str::parse(&name).unwrap();
                            }
                        }
                    }
                }
            }
        }
    }

    -1
}

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();

    let found_pid = find_celeste();

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

    let mem_file = load_mem(pid);

    dump_info_loop(mem_file);
}
