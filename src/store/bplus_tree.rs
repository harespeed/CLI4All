use anyhow::{anyhow, bail, Context, Result};
use std::fs;
use std::io::Write;
use std::path::Path;

use super::formats::{
    read_u32, read_u64, write_u32, write_u64, DataPointer, IndexEntry, IndexHeader,
    DEFAULT_PAGE_SIZE, FORMAT_VERSION, INDEX_HEADER_LEN, INDEX_MAGIC,
};

const PAGE_KIND_LEAF: u8 = 1;
const PAGE_KIND_INTERNAL: u8 = 2;
const PAGE_HEADER_LEN: usize = 16;

#[derive(Debug, Clone)]
struct ChildRef {
    first_key: String,
    offset: u64,
}

#[derive(Debug, Clone)]
struct LeafPage {
    first_key: String,
    entries: Vec<IndexEntry>,
}

#[derive(Debug, Clone)]
struct InternalPage {
    first_key: String,
    first_child_offset: u64,
    entries: Vec<InternalEntry>,
}

#[derive(Debug, Clone)]
struct InternalEntry {
    key: String,
    child_offset: u64,
}

#[derive(Debug, Clone)]
pub struct ReadOnlyBPlusTree {
    bytes: Vec<u8>,
    header: IndexHeader,
}

impl ReadOnlyBPlusTree {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let bytes = fs::read(path.as_ref())
            .with_context(|| format!("failed to read index file at {}", path.as_ref().display()))?;
        let header = parse_header(&bytes)?;
        Ok(Self { bytes, header })
    }

    pub fn get(&self, key: &str) -> Result<Option<DataPointer>> {
        if self.header.key_count == 0 {
            return Ok(None);
        }

        let leaf_offset = self.find_leaf_offset(key)?;
        let page = self.page_slice(leaf_offset)?;
        let (entry_count, _) = parse_page_header(page)?;
        let mut cursor = PAGE_HEADER_LEN;

        for _ in 0..entry_count {
            let (entry_key, pointer, next) = parse_leaf_entry(page, cursor)?;
            if entry_key == key {
                return Ok(Some(pointer));
            }
            cursor = next;
        }

        Ok(None)
    }

    pub fn scan_prefix(&self, prefix: &str) -> Result<Vec<DataPointer>> {
        if self.header.key_count == 0 {
            return Ok(Vec::new());
        }

        let mut pointers = Vec::new();
        let mut leaf_offset = self.find_leaf_offset(prefix)?;
        let mut seen_prefix = false;

        loop {
            let page = self.page_slice(leaf_offset)?;
            let (entry_count, next_leaf) = parse_page_header(page)?;
            let mut cursor = PAGE_HEADER_LEN;

            for _ in 0..entry_count {
                let (entry_key, pointer, next) = parse_leaf_entry(page, cursor)?;

                if entry_key.starts_with(prefix) {
                    seen_prefix = true;
                    pointers.push(pointer);
                } else if seen_prefix || entry_key.as_str() > prefix {
                    return Ok(pointers);
                }

                cursor = next;
            }

            if next_leaf == 0 {
                return Ok(pointers);
            }

            leaf_offset = next_leaf;
        }
    }

    fn find_leaf_offset(&self, key: &str) -> Result<u64> {
        let mut page_offset = self.header.root_page_offset;

        loop {
            let page = self.page_slice(page_offset)?;
            let page_kind = page[0];
            let (entry_count, extra) = parse_page_header(page)?;

            match page_kind {
                PAGE_KIND_LEAF => return Ok(page_offset),
                PAGE_KIND_INTERNAL => {
                    let mut selected = extra;
                    let mut cursor = PAGE_HEADER_LEN;

                    for _ in 0..entry_count {
                        let (separator, child_offset, next) = parse_internal_entry(page, cursor)?;
                        if key < separator.as_str() {
                            break;
                        }
                        selected = child_offset;
                        cursor = next;
                    }

                    page_offset = selected;
                }
                other => bail!("unsupported page kind {other} in index"),
            }
        }
    }

    fn page_slice(&self, offset: u64) -> Result<&[u8]> {
        let start = usize::try_from(offset).context("page offset does not fit in usize")?;
        let end =
            start + usize::try_from(self.header.page_size).unwrap_or(DEFAULT_PAGE_SIZE as usize);

        self.bytes
            .get(start..end)
            .ok_or_else(|| anyhow!("index page offset {offset} is out of bounds"))
    }
}

pub fn write_index_file(path: impl AsRef<Path>, sorted_entries: &[IndexEntry]) -> Result<()> {
    validate_entries(sorted_entries)?;

    let leaf_pages = build_leaf_pages(sorted_entries, DEFAULT_PAGE_SIZE as usize)?;
    let mut serialized_pages = Vec::new();
    let mut level = leaf_pages
        .iter()
        .enumerate()
        .map(|(index, page)| ChildRef {
            first_key: page.first_key.clone(),
            offset: INDEX_HEADER_LEN as u64 + (index as u64 * DEFAULT_PAGE_SIZE as u64),
        })
        .collect::<Vec<_>>();

    for (index, page) in leaf_pages.iter().enumerate() {
        let next_leaf = if index + 1 < leaf_pages.len() {
            INDEX_HEADER_LEN as u64 + ((index + 1) as u64 * DEFAULT_PAGE_SIZE as u64)
        } else {
            0
        };
        serialized_pages.push(serialize_leaf_page(
            page,
            next_leaf,
            DEFAULT_PAGE_SIZE as usize,
        )?);
    }

    let mut page_count = serialized_pages.len() as u64;

    while level.len() > 1 {
        let pages = build_internal_pages(&level, DEFAULT_PAGE_SIZE as usize)?;
        let start_offset = INDEX_HEADER_LEN as u64 + (page_count * DEFAULT_PAGE_SIZE as u64);
        let mut next_level = Vec::with_capacity(pages.len());

        for (index, page) in pages.iter().enumerate() {
            let offset = start_offset + (index as u64 * DEFAULT_PAGE_SIZE as u64);
            next_level.push(ChildRef {
                first_key: page.first_key.clone(),
                offset,
            });
            serialized_pages.push(serialize_internal_page(page, DEFAULT_PAGE_SIZE as usize)?);
        }

        page_count += pages.len() as u64;
        level = next_level;
    }

    let root_page_offset = level
        .first()
        .map(|page| page.offset)
        .unwrap_or(INDEX_HEADER_LEN as u64);

    let mut file = fs::File::create(path.as_ref())
        .with_context(|| format!("failed to create {}", path.as_ref().display()))?;
    file.write_all(&INDEX_MAGIC)
        .context("failed to write index magic")?;
    write_u32(&mut file, FORMAT_VERSION)?;
    write_u32(&mut file, DEFAULT_PAGE_SIZE)?;
    write_u64(&mut file, root_page_offset)?;
    write_u64(&mut file, page_count)?;
    write_u64(&mut file, sorted_entries.len() as u64)?;

    for page in serialized_pages {
        file.write_all(&page)
            .context("failed to write index page")?;
    }

    Ok(())
}

fn parse_header(bytes: &[u8]) -> Result<IndexHeader> {
    if bytes.len() < INDEX_HEADER_LEN {
        bail!("index file is too small");
    }

    if bytes[..4] != INDEX_MAGIC {
        bail!("invalid index magic number");
    }

    let mut cursor = std::io::Cursor::new(&bytes[4..INDEX_HEADER_LEN]);
    let version = read_u32(&mut cursor)?;
    let page_size = read_u32(&mut cursor)?;
    let root_page_offset = read_u64(&mut cursor)?;
    let page_count = read_u64(&mut cursor)?;
    let key_count = read_u64(&mut cursor)?;

    if version != FORMAT_VERSION {
        bail!("unsupported index version {version}");
    }

    if page_size != DEFAULT_PAGE_SIZE {
        bail!("unsupported page size {page_size}");
    }

    Ok(IndexHeader {
        version,
        page_size,
        root_page_offset,
        page_count,
        key_count,
    })
}

fn validate_entries(entries: &[IndexEntry]) -> Result<()> {
    for window in entries.windows(2) {
        if window[0].key >= window[1].key {
            bail!("index entries must be strictly sorted and unique");
        }
    }
    Ok(())
}

fn build_leaf_pages(entries: &[IndexEntry], page_size: usize) -> Result<Vec<LeafPage>> {
    let mut pages = Vec::new();
    let mut current_entries: Vec<IndexEntry> = Vec::new();
    let mut used = PAGE_HEADER_LEN;

    for entry in entries {
        let entry_size = 2 + entry.key.len() + 8 + 4;
        if used + entry_size > page_size && !current_entries.is_empty() {
            pages.push(LeafPage {
                first_key: current_entries[0].key.clone(),
                entries: current_entries,
            });
            current_entries = Vec::new();
            used = PAGE_HEADER_LEN;
        }

        if entry_size + PAGE_HEADER_LEN > page_size {
            bail!("index key '{}' is too large for one page", entry.key);
        }

        current_entries.push(entry.clone());
        used += entry_size;
    }

    if !current_entries.is_empty() {
        pages.push(LeafPage {
            first_key: current_entries[0].key.clone(),
            entries: current_entries,
        });
    }

    Ok(pages)
}

fn build_internal_pages(children: &[ChildRef], page_size: usize) -> Result<Vec<InternalPage>> {
    let mut pages = Vec::new();
    let mut chunk: Vec<ChildRef> = Vec::new();
    let mut used = PAGE_HEADER_LEN;

    for child in children {
        let entry_size = if chunk.is_empty() {
            0
        } else {
            2 + child.first_key.len() + 8
        };

        if used + entry_size > page_size && !chunk.is_empty() {
            pages.push(chunk_to_internal_page(&chunk)?);
            chunk = Vec::new();
            used = PAGE_HEADER_LEN;
        }

        if !chunk.is_empty() && entry_size + PAGE_HEADER_LEN > page_size {
            bail!(
                "separator key '{}' is too large for one page",
                child.first_key
            );
        }

        chunk.push(child.clone());
        used += entry_size;
    }

    if !chunk.is_empty() {
        pages.push(chunk_to_internal_page(&chunk)?);
    }

    Ok(pages)
}

fn chunk_to_internal_page(children: &[ChildRef]) -> Result<InternalPage> {
    let first = children
        .first()
        .ok_or_else(|| anyhow!("internal page cannot be empty"))?;

    Ok(InternalPage {
        first_key: first.first_key.clone(),
        first_child_offset: first.offset,
        entries: children[1..]
            .iter()
            .map(|child| InternalEntry {
                key: child.first_key.clone(),
                child_offset: child.offset,
            })
            .collect(),
    })
}

fn serialize_leaf_page(page: &LeafPage, next_leaf: u64, page_size: usize) -> Result<Vec<u8>> {
    let mut buffer = vec![0_u8; page_size];
    buffer[0] = PAGE_KIND_LEAF;
    buffer[4..8].copy_from_slice(&(page.entries.len() as u32).to_le_bytes());
    buffer[8..16].copy_from_slice(&next_leaf.to_le_bytes());

    let mut cursor = PAGE_HEADER_LEN;
    for entry in &page.entries {
        let key_bytes = entry.key.as_bytes();
        let key_len = u16::try_from(key_bytes.len()).context("key is too long")?;
        buffer[cursor..cursor + 2].copy_from_slice(&key_len.to_le_bytes());
        cursor += 2;
        buffer[cursor..cursor + key_bytes.len()].copy_from_slice(key_bytes);
        cursor += key_bytes.len();
        buffer[cursor..cursor + 8].copy_from_slice(&entry.pointer.offset.to_le_bytes());
        cursor += 8;
        buffer[cursor..cursor + 4].copy_from_slice(&entry.pointer.len.to_le_bytes());
        cursor += 4;
    }

    Ok(buffer)
}

fn serialize_internal_page(page: &InternalPage, page_size: usize) -> Result<Vec<u8>> {
    let mut buffer = vec![0_u8; page_size];
    buffer[0] = PAGE_KIND_INTERNAL;
    buffer[4..8].copy_from_slice(&(page.entries.len() as u32).to_le_bytes());
    buffer[8..16].copy_from_slice(&page.first_child_offset.to_le_bytes());

    let mut cursor = PAGE_HEADER_LEN;
    for entry in &page.entries {
        let key_bytes = entry.key.as_bytes();
        let key_len = u16::try_from(key_bytes.len()).context("key is too long")?;
        buffer[cursor..cursor + 2].copy_from_slice(&key_len.to_le_bytes());
        cursor += 2;
        buffer[cursor..cursor + key_bytes.len()].copy_from_slice(key_bytes);
        cursor += key_bytes.len();
        buffer[cursor..cursor + 8].copy_from_slice(&entry.child_offset.to_le_bytes());
        cursor += 8;
    }

    Ok(buffer)
}

fn parse_page_header(page: &[u8]) -> Result<(u32, u64)> {
    let entry_count = u32::from_le_bytes(page[4..8].try_into().unwrap());
    let extra = u64::from_le_bytes(page[8..16].try_into().unwrap());
    Ok((entry_count, extra))
}

fn parse_leaf_entry(page: &[u8], offset: usize) -> Result<(String, DataPointer, usize)> {
    let key_len = u16::from_le_bytes(page[offset..offset + 2].try_into().unwrap()) as usize;
    let key_start = offset + 2;
    let key_end = key_start + key_len;
    let pointer_start = key_end;
    let pointer_end = pointer_start + 8;
    let len_end = pointer_end + 4;

    let key = std::str::from_utf8(&page[key_start..key_end])
        .context("index key is not valid UTF-8")?
        .to_string();
    let pointer = DataPointer {
        offset: u64::from_le_bytes(page[pointer_start..pointer_end].try_into().unwrap()),
        len: u32::from_le_bytes(page[pointer_end..len_end].try_into().unwrap()),
    };

    Ok((key, pointer, len_end))
}

fn parse_internal_entry(page: &[u8], offset: usize) -> Result<(String, u64, usize)> {
    let key_len = u16::from_le_bytes(page[offset..offset + 2].try_into().unwrap()) as usize;
    let key_start = offset + 2;
    let key_end = key_start + key_len;
    let child_end = key_end + 8;
    let key = std::str::from_utf8(&page[key_start..key_end])
        .context("separator key is not valid UTF-8")?
        .to_string();
    let child_offset = u64::from_le_bytes(page[key_end..child_end].try_into().unwrap());

    Ok((key, child_offset, child_end))
}
