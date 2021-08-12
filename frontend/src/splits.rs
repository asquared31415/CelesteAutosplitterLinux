use celeste_autosplit_tracer as cat;
use serde::{Deserialize, Serialize};

use crate::util::format_time_with_units;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "kind_data")]
pub enum SplitKind {
    Heart,
    Casette,
    Berries(i32),
    Level(String),
    ChapterComplete,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Split {
    pub name: Option<String>,
    pub chapter: i32,
    pub split_kind: SplitKind,
}

impl Split {
    pub fn is_accomplished(&self, info: &cat::Dump) -> bool {
        match info.autosplitter_info.chapter == self.chapter {
            true => match &self.split_kind {
                SplitKind::Level(lvl) => lvl == &info.level_name(),
                SplitKind::Heart => info.autosplitter_info.chapter_heart,
                SplitKind::Casette => info.autosplitter_info.chapter_cassette,
                &SplitKind::Berries(bewwy_count) => {
                    bewwy_count == info.autosplitter_info.chapter_strawberries
                }
                SplitKind::ChapterComplete => info.autosplitter_info.chapter_complete,
            },
            false => false,
        }
    }

    pub fn display_long(&self) -> String {
        let name = if let Some(name) = &self.name {
            format!("{}\n  ", name)
        } else {
            String::new()
        };

        let split_info = match &self.split_kind {
            SplitKind::Heart => "Heart".to_string(),
            SplitKind::Casette => "Casette".to_string(),
            SplitKind::Berries(num) => format!("{} Berries", num),
            SplitKind::Level(room) => format!("Room {}", room),
            SplitKind::ChapterComplete => "Complete".to_string(),
        };
        format!("{}Cp. {} {}", name, self.chapter, split_info)
    }

    pub fn display_short(&self) -> String {
        if let Some(name) = &self.name {
            return name.to_string();
        }

        let split_info = match &self.split_kind {
            SplitKind::Heart => "Heart".to_string(),
            SplitKind::Casette => "Casette".to_string(),
            SplitKind::Berries(num) => format!("{} Berries", num),
            SplitKind::Level(room) => format!("Room {}", room),
            SplitKind::ChapterComplete => "Complete".to_string(),
        };
        format!("Cp. {} {}", self.chapter, split_info)
    }

    pub fn display_incomplete(&self, info: &cat::Dump) -> String {
        if let Some(name) = &self.name {
            return name.clone();
        }

        let split_kind = match &self.split_kind {
            SplitKind::Level(level) => level,
            SplitKind::Heart => "Heart",
            SplitKind::Casette => "Casette",
            SplitKind::ChapterComplete => "Complete",
            SplitKind::Berries(num_berries) => {
                return format!(
                    "{}/{} Berries",
                    info.autosplitter_info.file_strawberries, num_berries
                );
            }
        };
        format!("Ch.{}: {}", self.chapter, split_kind,)
    }

    pub fn display_complete(&self, finish_time: u64) -> String {
        let finish_time = std::time::Duration::from_millis(finish_time);

        if let Some(name) = &self.name {
            return format!("{} = {}", name.clone(), format_time_with_units(finish_time));
        }

        let split_kind = match &self.split_kind {
            SplitKind::Level(level) => level,
            SplitKind::Heart => "Heart",
            SplitKind::Casette => "Casette",
            SplitKind::ChapterComplete => "Complete",
            SplitKind::Berries(num_berries) => {
                return format!(
                    "{}/{} Berries = {:#?}",
                    num_berries, num_berries, finish_time
                );
            }
        };
        format!("Ch.{}: {} = {:#?}", self.chapter, split_kind, finish_time)
    }
}

#[derive(Debug)]
pub struct CurrentSplits {
    pub completed_splits: Vec<(Split, u64)>,
    pub todo_splits: Vec<Split>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Splits {
    pub split_mode: (String, i32),
    pub splits: Vec<Split>,
}
