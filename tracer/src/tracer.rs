use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    mem,
    path::PathBuf,
    process,
    sync::{Arc, Mutex},
};

use once_cell::sync::OnceCell;

pub fn load_mem(pid: u32) -> File {
    let path = PathBuf::from(format!("/proc/{}/mem", pid));
    File::open(path).unwrap_or_else(|e| {
        if let io::ErrorKind::PermissionDenied = e.kind() {
            eprintln!("Permission to access memory file for {} denied", pid);
            process::exit(1);
        } else {
            panic!("Unable to open mem file for process {}: {}", pid, e);
        }
    })
}

pub static MEM_FILE: OnceCell<Arc<Mutex<Option<File>>>> = OnceCell::new();

pub struct MemPtr(usize);

impl MemPtr {
    pub fn new(addr: usize) -> Self {
        Self(addr)
    }

    // SAFETY: a T must be valid at the specified offset (basically ptr read)
    pub unsafe fn read<T>(&self) -> T
    where
        T: Copy,
        [(); mem::size_of::<T>()]: ,
    {
        let lock = &mut MEM_FILE
            .get()
            .expect("Mem file not initialized")
            .lock()
            .expect("Unable to lock mem file");
        let mem_file: &mut File = lock.as_mut().expect("Mem file deinitialized");
        mem_file
            .seek(SeekFrom::Start(self.0 as u64))
            .expect("Unable to read memory");
        let mut buf = [0_u8; mem::size_of::<T>()];
        mem_file
            .read_exact(&mut buf)
            .unwrap_or_else(|_| panic!("Unable to read memory at {:#X}", self.0));
        unsafe { *(buf.as_ptr() as *const T) }
    }

    // SAFETY: a T must be valid at the specified offset (basically ptr read)
    // the provided pointer must be valid for writes for the specified number of writes of size T
    pub unsafe fn read_into<T>(&self, out: &mut [T])
    where
        T: Copy,
    {
        let lock = &mut MEM_FILE
            .get()
            .expect("Mem file not initialized")
            .lock()
            .expect("Unable to lock mem file");
        let mem_file: &mut File = lock.as_mut().expect("Mem file not initialized");
        mem_file
            .seek(SeekFrom::Start(self.0 as u64))
            .expect("Unable to read memory");

        let count = out.len();
        let mut out = unsafe {
            std::slice::from_raw_parts_mut(out as *mut [T] as *mut u8, count * mem::size_of::<T>())
        };
        mem_file
            .read_exact(&mut out)
            .unwrap_or_else(|_| panic!("Unable to read memory at {:#X}", self.0));
    }
}

pub unsafe fn read_u64(addr: usize) -> u64 {
    unsafe { MemPtr::new(addr).read::<u64>() }
}

pub unsafe fn read_u32(addr: usize) -> u32 {
    unsafe { MemPtr::new(addr).read::<u32>() }
}

pub unsafe fn read_u8(addr: usize) -> u8 {
    unsafe { MemPtr::new(addr).read::<u8>() }
}

pub unsafe fn read_string(addr: usize) -> String {
    unsafe {
        let mut buf = vec![0_u8; 100];
        MemPtr::new(addr).read_into(&mut buf);
        buf.set_len(100);
        let data = buf.into_iter().take_while(|&c| c != 0).collect::<Vec<_>>();
        String::from_utf8_unchecked(data)
    }
}

pub fn read_boxed_string(instance: usize) -> String {
    unsafe {
        let class = instance_class(instance);
        let data_offset = class_field_offset(class, "m_firstChar");
        let size_offset = class_field_offset(class, "m_stringLength");
        let size = read_u32(instance + size_offset) as usize;

        let mut utf16 = vec![0_u16; size];
        MemPtr::new(instance + data_offset).read_into(&mut utf16);
        utf16.set_len(size);
        String::from_utf16_lossy(&utf16)
    }
}

pub unsafe fn class_name(class: usize) -> String {
    unsafe {
        let name_ptr = read_u64(class + 0x40) as usize;
        read_string(name_ptr as usize)
    }
}

pub unsafe fn lookup_class<S: AsRef<str>>(class_cache: usize, name: S) -> usize {
    let target_name = name.as_ref();
    unsafe {
        let cache_table = read_u64(class_cache + 0x20) as usize;
        let hash_table_size = read_u32(cache_table + 0x18) as usize;

        for bucket in 0..hash_table_size {
            let mut class = read_u64(cache_table + 8 * bucket) as usize;
            while class != 0 {
                let class_name = class_name(class);
                if class_name == target_name {
                    return class as usize;
                }

                class = read_u64(class + 0xF8) as usize;
            }
        }

        panic!("Could not find class {}", target_name);
    }
}

pub unsafe fn instance_class(instance: usize) -> usize {
    unsafe { read_u64(read_u64(instance) as usize & (!1_i32 as usize)) as usize }
}

pub unsafe fn class_static_fields(class: usize) -> u64 {
    unsafe {
        let vtable_size = read_u32(class + 0x54);
        let runtime_info = read_u64(class + 0xC8);
        let max_domains = read_u64(runtime_info as usize) as usize;

        for i in 0..=max_domains {
            let vtable = read_u64(runtime_info as usize + 8 + 8 * i);
            if vtable != 0 {
                return read_u64(vtable as usize + 64 + 8 * vtable_size as usize);
            }
        }

        panic!("No domain has class {:#X} loaded", class);
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
#[allow(dead_code)] // the variants are transmuted from u8
enum MonoTypeKind {
    MonoClassDef = 1,
    MonoClassGTD = 2,
    MonoClassGInst = 3,
    MonoClassGParam = 4,
    MonoClassArray = 5,
    MonoClassPointer = 6,
}

impl MonoTypeKind {
    fn from_u8(v: u8) -> Self {
        assert!((1..=6).contains(&v), "Value out of range");
        unsafe { mem::transmute(v) }
    }
}

fn class_kind(class: usize) -> MonoTypeKind {
    unsafe { MonoTypeKind::from_u8(read_u8(class + 0x24) & 7) }
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
struct MonoClassField {
    t: u64,
    name: u64,
    parent: u64,
    offset: u32,
}

pub unsafe fn class_field_offset(class: usize, name: &str) -> usize {
    let kind = class_kind(class);
    unsafe {
        match kind {
            MonoTypeKind::MonoClassGInst => {
                class_field_offset(read_u64(read_u64(class + 0xE0) as usize) as usize, name)
            }
            MonoTypeKind::MonoClassDef | MonoTypeKind::MonoClassGTD => {
                let num_fields = read_u32(class + 0xF0);
                let fields_ptr = read_u64(class + 0x90);

                for i in 0..num_fields as usize {
                    let field: MonoClassField =
                        MemPtr::new(fields_ptr as usize + i * mem::size_of::<MonoClassField>())
                            .read();
                    let nametest = read_string(field.name as usize);
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

pub unsafe fn static_field_u64<S: AsRef<str>>(class: usize, name: S) -> u64 {
    unsafe {
        let static_data = class_static_fields(class);
        let field_offset = class_field_offset(class, name.as_ref());
        read_u64(static_data as usize + field_offset)
    }
}

pub unsafe fn instance_field_u32<S: AsRef<str>>(instance: usize, name: S) -> u32 {
    unsafe {
        let class = instance_class(instance);
        let field_offset = class_field_offset(class, name.as_ref());
        read_u32(instance + field_offset)
    }
}

pub unsafe fn instance_field_u64<S: AsRef<str>>(instance: usize, name: S) -> u64 {
    unsafe {
        let class = instance_class(instance);
        let field_offset = class_field_offset(class, name.as_ref());
        read_u64(instance + field_offset)
    }
}

pub fn locate_autosplitter_info(instance: usize) -> usize {
    unsafe { instance_field_u64(instance, "AutoSplitterInfo") as usize + 0x10 }
}
