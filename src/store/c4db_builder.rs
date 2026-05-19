use anyhow::{anyhow, bail, Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::rules::build_command_regex;

use super::bplus_tree::write_index_file;
use super::command_store::CommandRecord;
use super::formats::{
    read_source_catalog, serialize_command_record, write_u32, write_u64, DataPointer, IndexEntry,
    DATA_MAGIC, FORMAT_VERSION,
};
use super::normalize::{category_key, exact_command_key, intent_key};

pub fn build_command_index(
    input_path: impl AsRef<Path>,
    index_path: impl AsRef<Path>,
    data_path: impl AsRef<Path>,
) -> Result<()> {
    let source_bytes = fs::read(input_path.as_ref()).with_context(|| {
        format!(
            "failed to read commands source JSON at {}",
            input_path.as_ref().display()
        )
    })?;
    let mut records = read_source_catalog(&source_bytes)?.commands;
    records.sort_by(|left, right| left.intent.cmp(&right.intent));

    for record in &records {
        validate_record(record)?;
    }

    let pointers = write_data_file(data_path.as_ref(), &records)?;
    let entries = build_index_entries(&records, &pointers)?;
    write_index_file(index_path.as_ref(), &entries)
}

fn write_data_file(path: &Path, records: &[CommandRecord]) -> Result<Vec<DataPointer>> {
    let mut file =
        fs::File::create(path).with_context(|| format!("failed to create {}", path.display()))?;
    use std::io::Write;

    file.write_all(&DATA_MAGIC)
        .context("failed to write data magic")?;
    write_u32(&mut file, FORMAT_VERSION)?;
    write_u64(&mut file, records.len() as u64)?;

    let mut pointers = Vec::with_capacity(records.len());
    let mut offset = 16_u64;

    for record in records {
        let blob = serialize_command_record(record)?;
        let len = u32::try_from(blob.len()).context("record blob is too large")?;
        write_u32(&mut file, len)?;
        offset += 4;
        file.write_all(&blob)
            .context("failed to write command blob")?;
        pointers.push(DataPointer { offset, len });
        offset += u64::from(len);
    }

    Ok(pointers)
}

fn build_index_entries(
    records: &[CommandRecord],
    pointers: &[DataPointer],
) -> Result<Vec<IndexEntry>> {
    let mut map = BTreeMap::new();

    for (record, pointer) in records.iter().zip(pointers.iter().copied()) {
        insert_entry(&mut map, intent_key(&record.intent), pointer)?;
        insert_entry(
            &mut map,
            category_key(&record.category, &record.intent),
            pointer,
        )?;

        for (_, commands) in record.iter_commands() {
            for command in commands {
                insert_entry(&mut map, exact_command_key(command), pointer)?;
            }
        }
    }

    Ok(map
        .into_iter()
        .map(|(key, pointer)| IndexEntry { key, pointer })
        .collect())
}

fn insert_entry(
    map: &mut BTreeMap<String, DataPointer>,
    key: String,
    pointer: DataPointer,
) -> Result<()> {
    if let Some(existing) = map.insert(key.clone(), pointer) {
        if existing != pointer {
            bail!("index key collision for '{key}'");
        }
    }
    Ok(())
}

fn validate_record(record: &CommandRecord) -> Result<()> {
    if record.intent.trim().is_empty() {
        bail!("command record has an empty intent");
    }

    if record.category.trim().is_empty() {
        bail!("command record '{}' has an empty category", record.intent);
    }

    if record.commands.is_empty() {
        bail!(
            "command record '{}' does not define any platform commands",
            record.intent
        );
    }

    for (platform, commands) in &record.commands {
        if commands.is_empty() {
            return Err(anyhow!(
                "command record '{}' platform '{}' does not define any examples",
                record.intent,
                platform
            ));
        }

        for command in commands {
            build_command_regex(command).with_context(|| {
                format!(
                    "invalid command example for intent '{}' platform '{}': {}",
                    record.intent, platform, command
                )
            })?;
        }
    }

    Ok(())
}
