use anyhow::{anyhow, bail, Context, Result};
use regex::Regex;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use crate::rules::{build_command_regex, placeholder_name};

use super::bplus_tree::ReadOnlyBPlusTree;
use super::command_store::{CommandRecord, CommandStore};
use super::formats::{
    deserialize_command_record, read_u32, read_u64, DataHeader, DataPointer, DATA_HEADER_LEN,
    DATA_MAGIC, FORMAT_VERSION,
};
use super::normalize::{exact_command_key, intent_key, normalize_category};

#[derive(Debug, Clone)]
pub struct C4DbCommandStore {
    index: ReadOnlyBPlusTree,
    data_bytes: Vec<u8>,
    data_header: DataHeader,
}

impl C4DbCommandStore {
    pub fn open(index_path: impl AsRef<Path>, data_path: impl AsRef<Path>) -> Result<Self> {
        let index = ReadOnlyBPlusTree::open(index_path.as_ref())?;
        let data_bytes = fs::read(data_path.as_ref()).with_context(|| {
            format!(
                "failed to read data file at {}",
                data_path.as_ref().display()
            )
        })?;
        let data_header = parse_data_header(&data_bytes)?;

        Ok(Self {
            index,
            data_bytes,
            data_header,
        })
    }

    fn read_record(&self, pointer: DataPointer) -> Result<CommandRecord> {
        let start =
            usize::try_from(pointer.offset).context("record offset does not fit in usize")?;
        let end =
            start + usize::try_from(pointer.len).context("record length does not fit in usize")?;
        let bytes = self
            .data_bytes
            .get(start..end)
            .ok_or_else(|| anyhow!("record pointer is out of bounds"))?;
        deserialize_command_record(bytes)
    }

    fn iter_records(&self) -> Result<Vec<CommandRecord>> {
        let mut cursor = DATA_HEADER_LEN;
        let mut records = Vec::with_capacity(self.data_header.record_count as usize);

        for _ in 0..self.data_header.record_count {
            let len_bytes = self
                .data_bytes
                .get(cursor..cursor + 4)
                .ok_or_else(|| anyhow!("data file ended while reading record length"))?;
            let len = u32::from_le_bytes(len_bytes.try_into().unwrap()) as usize;
            cursor += 4;
            let record_bytes = self
                .data_bytes
                .get(cursor..cursor + len)
                .ok_or_else(|| anyhow!("data file ended while reading record blob"))?;
            records.push(deserialize_command_record(record_bytes)?);
            cursor += len;
        }

        Ok(records)
    }

    fn best_record_match(&self, input: &str, record: &CommandRecord) -> Option<RecordMatch> {
        let token = extract_command_token(input);
        let normalized_input = super::normalize::normalize_command(input);
        let mut best: Option<RecordMatch> = None;

        for (platform, commands) in record.iter_commands() {
            for example in commands {
                if match_example(input, example).is_some() {
                    let candidate = RecordMatch {
                        platform: platform.to_string(),
                        full_match: true,
                    };
                    if is_better_match(best.as_ref(), &candidate) {
                        best = Some(candidate);
                    }
                    continue;
                }

                if super::normalize::normalize_command(example) == normalized_input {
                    let candidate = RecordMatch {
                        platform: platform.to_string(),
                        full_match: true,
                    };
                    if is_better_match(best.as_ref(), &candidate) {
                        best = Some(candidate);
                    }
                    continue;
                }

                if let Some(token) = token.as_deref() {
                    if first_token(example).eq_ignore_ascii_case(token) {
                        let candidate = RecordMatch {
                            platform: platform.to_string(),
                            full_match: false,
                        };
                        if is_better_match(best.as_ref(), &candidate) {
                            best = Some(candidate);
                        }
                    }
                }
            }
        }

        best
    }
}

impl CommandStore for C4DbCommandStore {
    fn find_by_command(&self, command: &str) -> Result<Option<CommandRecord>> {
        if let Some(pointer) = self.index.get(&exact_command_key(command))? {
            return self.read_record(pointer).map(Some);
        }

        let mut best: Option<(u8, bool, CommandRecord)> = None;
        for record in self.iter_records()? {
            if let Some(record_match) = self.best_record_match(command, &record) {
                let rank = platform_rank(&record_match.platform);
                let candidate = (rank, record_match.full_match, record);
                let replace = match &best {
                    None => true,
                    Some((best_rank, best_full_match, _)) => {
                        (candidate.1 && !*best_full_match)
                            || (candidate.1 == *best_full_match && candidate.0 < *best_rank)
                    }
                };

                if replace {
                    best = Some(candidate);
                }
            }
        }

        Ok(best.map(|(_, _, record)| record))
    }

    fn find_by_intent(&self, intent: &str) -> Result<Option<CommandRecord>> {
        match self.index.get(&intent_key(intent))? {
            Some(pointer) => self.read_record(pointer).map(Some),
            None => Ok(None),
        }
    }

    fn list_by_category(&self, category: &str) -> Result<Vec<CommandRecord>> {
        let prefix = format!("category:{}:", normalize_category(category));
        let pointers = self.index.scan_prefix(&prefix)?;
        let mut records = Vec::with_capacity(pointers.len());
        for pointer in pointers {
            records.push(self.read_record(pointer)?);
        }
        Ok(records)
    }
}

pub fn load_command_store() -> Result<C4DbCommandStore> {
    let data_dir = crate::data_paths::find_data_dir(&["commands.c4idx", "commands.c4dat"])?;
    let index_path = data_dir.join("commands.c4idx");
    let data_path = data_dir.join("commands.c4dat");
    C4DbCommandStore::open(index_path, data_path)
}

fn parse_data_header(bytes: &[u8]) -> Result<DataHeader> {
    if bytes.len() < DATA_HEADER_LEN {
        bail!("data file is too small");
    }

    if bytes[..4] != DATA_MAGIC {
        bail!("invalid data magic number");
    }

    let mut cursor = std::io::Cursor::new(&bytes[4..DATA_HEADER_LEN]);
    let version = read_u32(&mut cursor)?;
    let record_count = read_u64(&mut cursor)?;

    if version != FORMAT_VERSION {
        bail!("unsupported data version {version}");
    }

    Ok(DataHeader {
        version,
        record_count,
    })
}

#[derive(Debug, Clone)]
struct RecordMatch {
    platform: String,
    full_match: bool,
}

fn extract_command_token(input: &str) -> Option<String> {
    if let Some(captures) = command_not_found_regex().captures(input) {
        return captures.get(1).map(|value| value.as_str().to_string());
    }

    leading_token_regex()
        .captures(input)
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().to_string())
}

fn command_not_found_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)command not found:\s*([A-Za-z0-9._-]+)").expect("valid regex")
    })
}

fn leading_token_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"^\s*([A-Za-z0-9._-]+)").expect("valid regex"))
}

fn match_example(input: &str, example: &str) -> Option<BTreeMap<String, String>> {
    let regex = build_command_regex(example).expect("validated command regex");
    let captures = regex.captures(input)?;
    let mut values = BTreeMap::new();

    for token in example.split_whitespace() {
        if let Some(name) = placeholder_name(token) {
            if let Some(value) = captures.name(name) {
                values.insert(name.to_string(), value.as_str().to_string());
            }
        }
    }

    Some(values)
}

fn first_token(command: &str) -> &str {
    command.split_whitespace().next().unwrap_or(command)
}

fn is_better_match(current: Option<&RecordMatch>, candidate: &RecordMatch) -> bool {
    match current {
        None => true,
        Some(current) => {
            (candidate.full_match && !current.full_match)
                || (candidate.full_match == current.full_match
                    && platform_rank(&candidate.platform) < platform_rank(&current.platform))
        }
    }
}

fn platform_rank(platform: &str) -> u8 {
    match platform {
        "ubuntu" => 0,
        "windows_cmd" => 1,
        "powershell" => 2,
        "macos" => 3,
        _ => 4,
    }
}
