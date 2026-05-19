use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Read, Write};

use super::command_store::CommandRecord;

pub const INDEX_MAGIC: [u8; 4] = *b"C4I1";
pub const DATA_MAGIC: [u8; 4] = *b"C4D1";
pub const FORMAT_VERSION: u32 = 1;
pub const DEFAULT_PAGE_SIZE: u32 = 4096;
pub const INDEX_HEADER_LEN: usize = 36;
pub const DATA_HEADER_LEN: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataPointer {
    pub offset: u64,
    pub len: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct IndexHeader {
    pub version: u32,
    pub page_size: u32,
    pub root_page_offset: u64,
    pub page_count: u64,
    pub key_count: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct DataHeader {
    pub version: u32,
    pub record_count: u64,
}

#[derive(Debug, Clone)]
pub struct IndexEntry {
    pub key: String,
    pub pointer: DataPointer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceCommandCatalog {
    pub commands: Vec<CommandRecord>,
}

#[derive(Serialize)]
struct SerializableCommandRecord<'a> {
    intent: &'a str,
    description: &'a str,
    category: &'a str,
    risk_level: &'a str,
    commands: BTreeMap<&'a String, &'a Vec<String>>,
    notes: &'a [String],
}

pub fn serialize_command_record(record: &CommandRecord) -> Result<Vec<u8>> {
    let commands = record
        .commands
        .iter()
        .collect::<BTreeMap<&String, &Vec<String>>>();

    serde_json::to_vec(&SerializableCommandRecord {
        intent: &record.intent,
        description: &record.description,
        category: &record.category,
        risk_level: &record.risk_level,
        commands,
        notes: &record.notes,
    })
    .context("failed to serialize command record as JSON")
}

pub fn deserialize_command_record(bytes: &[u8]) -> Result<CommandRecord> {
    serde_json::from_slice(bytes).context("failed to deserialize command record JSON")
}

pub fn read_source_catalog(bytes: &[u8]) -> Result<SourceCommandCatalog> {
    serde_json::from_slice(bytes).context("failed to parse commands source JSON")
}

pub fn write_u32<W: Write>(writer: &mut W, value: u32) -> Result<()> {
    writer
        .write_all(&value.to_le_bytes())
        .context("failed to write u32")
}

pub fn write_u64<W: Write>(writer: &mut W, value: u64) -> Result<()> {
    writer
        .write_all(&value.to_le_bytes())
        .context("failed to write u64")
}

pub fn read_u32<R: Read>(reader: &mut R) -> Result<u32> {
    let mut buffer = [0_u8; 4];
    reader
        .read_exact(&mut buffer)
        .context("failed to read u32")?;
    Ok(u32::from_le_bytes(buffer))
}

pub fn read_u64<R: Read>(reader: &mut R) -> Result<u64> {
    let mut buffer = [0_u8; 8];
    reader
        .read_exact(&mut buffer)
        .context("failed to read u64")?;
    Ok(u64::from_le_bytes(buffer))
}
