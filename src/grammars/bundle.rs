//! Deterministic `MRKB` bundle reader/writer.
//!
//! The production highlighter resolves every grammar through this bundle. It
//! is a byte-level container with eager metadata and lazy per-grammar blob
//! access, produced by the `grammar-compile` build step.

use std::{collections::BTreeMap, path::Path};

use crate::engine::{
    grammar::{CompiledGrammar, load_dev_grammar_from_str},
    state::GrammarId,
};

pub const MAGIC: &[u8; 4] = b"MRKB";
pub const FORMAT_VERSION: u16 = 1;
pub const CODEC_NONE: u32 = 0;
pub const CODEC_DEFLATE_ZLIB: u32 = 1;
pub const SECTION_STRINGS: u32 = 3;
pub const SECTION_SCOPES: u32 = 4;
pub const SECTION_LANGUAGES: u32 = 5;
pub const SECTION_GRAMMAR_BLOBS: u32 = 6;
pub const SECTION_LICENSES: u32 = 7;

const HEADER_LEN: usize = 32;
const SECTION_ENTRY_LEN: usize = 24;
const NO_STRING: u32 = u32::MAX;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleHeader {
    pub format_version: u16,
    pub section_count: u16,
    pub source_hash: u64,
    pub bundle_hash: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionEntry {
    pub id: u32,
    pub offset: u64,
    pub len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Bundle {
    pub source_hash: u64,
    pub bundle_hash: u64,
    pub strings: Vec<String>,
    pub scopes: Vec<String>,
    pub languages: Vec<LanguageEntry>,
    pub grammar_blobs: Vec<GrammarBlob>,
    pub licenses: Vec<LicenseEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageEntry {
    pub canonical: String,
    pub scope_name: String,
    pub aliases: Vec<String>,
    pub extensions: Vec<String>,
    pub basenames: Vec<String>,
    pub first_line_pattern: Option<String>,
    pub grammar_blob: u32,
    pub license: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrammarBlob {
    pub language: String,
    pub scope_name: String,
    pub codec: u32,
    pub flags: u32,
    pub raw_len: u32,
    pub bytes: Vec<u8>,
    pub pattern_count: u32,
    pub dfa_count: u32,
    pub fallback_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LicenseEntry {
    pub language: String,
    pub source_path: String,
    pub upstream_url: String,
    pub spdx_id: String,
    pub license_text: String,
    pub source_revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BundleError {
    TooShort,
    BadMagic,
    UnsupportedVersion(u16),
    SectionTableOutOfBounds,
    SectionOutOfBounds { id: u32 },
    MissingSection(u32),
    BadUtf8,
    UnknownLanguage(String),
    BadStringId(u32),
    BadGrammarBlobId(u32),
    BadLicenseId(u32),
    BadCodec(u32),
    Inflate { language: String },
    Truncated(&'static str),
    TrailingBytes(&'static str),
    GrammarParse { language: String, message: String },
}

#[derive(Debug, Clone)]
pub struct BundleGrammarRegistry {
    bundle: Bundle,
    cache: BTreeMap<String, CompiledGrammar>,
}

impl BundleGrammarRegistry {
    pub fn new(bundle: Bundle) -> Self {
        Self {
            bundle,
            cache: BTreeMap::new(),
        }
    }

    pub fn bundle(&self) -> &Bundle {
        &self.bundle
    }

    pub fn cached_grammar_count(&self) -> usize {
        self.cache.len()
    }

    pub fn grammar(&mut self, language: &str) -> Result<&CompiledGrammar, BundleError> {
        let canonical = self
            .bundle
            .canonical_language(language)
            .ok_or_else(|| BundleError::UnknownLanguage(language.to_owned()))?
            .to_owned();
        if !self.cache.contains_key(&canonical) {
            let blob = self
                .bundle
                .grammar_blob_for_language(&canonical)
                .ok_or(BundleError::BadGrammarBlobId(u32::MAX))?;
            let bytes = blob.decoded_bytes()?;
            let source = std::str::from_utf8(&bytes).map_err(|_| BundleError::BadUtf8)?;
            let id = GrammarId(self.cache.len() as u16);
            let grammar = load_dev_grammar_from_str(id, source).map_err(|error| {
                BundleError::GrammarParse {
                    language: canonical.clone(),
                    message: error.to_string(),
                }
            })?;
            self.cache.insert(canonical.clone(), grammar);
        }
        Ok(self
            .cache
            .get(&canonical)
            .expect("grammar inserted before lookup"))
    }

    pub fn grammar_by_scope(&mut self, scope_name: &str) -> Result<&CompiledGrammar, BundleError> {
        let canonical = self
            .bundle
            .languages
            .iter()
            .find(|entry| entry.scope_name == scope_name)
            .map(|entry| entry.canonical.clone())
            .ok_or_else(|| BundleError::UnknownLanguage(scope_name.to_owned()))?;
        self.grammar(&canonical)
    }
}

impl GrammarBlob {
    pub fn decoded_bytes(&self) -> Result<Vec<u8>, BundleError> {
        match self.codec {
            CODEC_NONE => Ok(self.bytes.clone()),
            CODEC_DEFLATE_ZLIB => {
                let bytes =
                    miniz_oxide::inflate::decompress_to_vec_zlib(&self.bytes).map_err(|_| {
                        BundleError::Inflate {
                            language: self.language.clone(),
                        }
                    })?;
                if bytes.len() != self.raw_len as usize {
                    return Err(BundleError::Inflate {
                        language: self.language.clone(),
                    });
                }
                Ok(bytes)
            }
            other => Err(BundleError::BadCodec(other)),
        }
    }
}

impl Bundle {
    pub fn parse(bytes: &[u8]) -> Result<Self, BundleError> {
        let (header, sections) = read_header_and_sections(bytes)?;
        if header.format_version != FORMAT_VERSION {
            return Err(BundleError::UnsupportedVersion(header.format_version));
        }
        let strings = decode_string_table(section(bytes, &sections, SECTION_STRINGS)?)?;
        let scopes = decode_scope_table(section(bytes, &sections, SECTION_SCOPES)?, &strings)?;
        let languages =
            decode_language_table(section(bytes, &sections, SECTION_LANGUAGES)?, &strings)?;
        let grammar_blobs =
            decode_grammar_blobs(section(bytes, &sections, SECTION_GRAMMAR_BLOBS)?, &strings)?;
        let licenses =
            decode_license_table(section(bytes, &sections, SECTION_LICENSES)?, &strings)?;
        Ok(Self {
            source_hash: header.source_hash,
            bundle_hash: header.bundle_hash,
            strings,
            scopes,
            languages,
            grammar_blobs,
            licenses,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let strings = interned_strings(self);
        let sections = vec![
            (SECTION_STRINGS, encode_string_table(&strings)),
            (SECTION_SCOPES, encode_scope_table(&self.scopes, &strings)),
            (
                SECTION_LANGUAGES,
                encode_language_table(&self.languages, &strings),
            ),
            (
                SECTION_GRAMMAR_BLOBS,
                encode_grammar_blobs(&self.grammar_blobs, &strings),
            ),
            (
                SECTION_LICENSES,
                encode_license_table(&self.licenses, &strings),
            ),
        ];
        let bundle_hash = hash_sections(&sections);
        write_container(self.source_hash, bundle_hash, sections)
    }

    pub fn version_stamp(&self) -> String {
        format!("{:016x}", self.bundle_hash)
    }

    pub fn available_languages(&self) -> Vec<&str> {
        self.languages
            .iter()
            .map(|language| language.canonical.as_str())
            .collect()
    }

    pub fn canonical_language(&self, language: &str) -> Option<&str> {
        let language = normalize_token(language);
        if let Some(entry) = self
            .languages
            .iter()
            .find(|entry| entry.canonical == language)
        {
            return Some(entry.canonical.as_str());
        }
        self.languages.iter().find_map(|entry| {
            entry
                .aliases
                .iter()
                .any(|alias| alias == &language)
                .then_some(entry.canonical.as_str())
        })
    }

    pub fn has_language(&self, language: &str) -> bool {
        self.canonical_language(language).is_some()
    }

    pub fn detect_language_from_path(&self, path: &str) -> Option<&str> {
        let name = Path::new(path).file_name()?.to_str()?;
        let lower_name = name.to_ascii_lowercase();
        if let Some(language) = self.languages.iter().find_map(|entry| {
            entry
                .basenames
                .iter()
                .any(|basename| basename.eq_ignore_ascii_case(name))
                .then_some(entry.canonical.as_str())
        }) {
            return Some(language);
        }
        // Compound fileTypes (`blade.php`, `html.erb`, `adoc.txt`, …) are
        // registered as basenames but follow TextMate suffix semantics, so
        // they also compete with plain extensions by match length.
        self.languages
            .iter()
            .filter_map(|entry| {
                entry
                    .extensions
                    .iter()
                    .chain(entry.basenames.iter().filter(|name| name.contains('.')))
                    .filter_map(|extension| extension_match_len(&lower_name, extension))
                    .max()
                    .map(|len| (len, entry.canonical.as_str()))
            })
            .max_by_key(|(len, _)| *len)
            .map(|(_, language)| language)
    }

    pub fn grammar_blob_for_language(&self, language: &str) -> Option<&GrammarBlob> {
        let canonical = self.canonical_language(language)?;
        let entry = self
            .languages
            .iter()
            .find(|entry| entry.canonical == canonical)?;
        self.grammar_blobs.get(entry.grammar_blob as usize)
    }

    pub fn grammar_blob_for_scope(&self, scope_name: &str) -> Option<&GrammarBlob> {
        self.grammar_blobs
            .iter()
            .find(|blob| blob.scope_name == scope_name)
    }
}

pub fn read_header(bytes: &[u8]) -> Result<BundleHeader, BundleError> {
    read_header_and_sections(bytes).map(|(header, _)| header)
}

pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn read_header_and_sections(
    bytes: &[u8],
) -> Result<(BundleHeader, Vec<SectionEntry>), BundleError> {
    if bytes.len() < HEADER_LEN {
        return Err(BundleError::TooShort);
    }
    if &bytes[..4] != MAGIC {
        return Err(BundleError::BadMagic);
    }
    let format_version = read_u16_at(bytes, 4)?;
    let section_count = read_u16_at(bytes, 6)?;
    let source_hash = read_u64_at(bytes, 8)?;
    let bundle_hash = read_u64_at(bytes, 16)?;
    let table_len = usize::from(section_count)
        .checked_mul(SECTION_ENTRY_LEN)
        .ok_or(BundleError::SectionTableOutOfBounds)?;
    let table_end = HEADER_LEN
        .checked_add(table_len)
        .ok_or(BundleError::SectionTableOutOfBounds)?;
    if table_end > bytes.len() {
        return Err(BundleError::SectionTableOutOfBounds);
    }
    let mut sections = Vec::with_capacity(usize::from(section_count));
    let mut cursor = HEADER_LEN;
    for _ in 0..section_count {
        let id = read_u32_at(bytes, cursor)?;
        let offset = read_u64_at(bytes, cursor + 8)?;
        let len = read_u64_at(bytes, cursor + 16)?;
        let end = offset
            .checked_add(len)
            .ok_or(BundleError::SectionOutOfBounds { id })?;
        if end as usize > bytes.len() {
            return Err(BundleError::SectionOutOfBounds { id });
        }
        sections.push(SectionEntry { id, offset, len });
        cursor += SECTION_ENTRY_LEN;
    }
    Ok((
        BundleHeader {
            format_version,
            section_count,
            source_hash,
            bundle_hash,
        },
        sections,
    ))
}

fn section<'a>(
    bytes: &'a [u8],
    sections: &[SectionEntry],
    id: u32,
) -> Result<&'a [u8], BundleError> {
    let section = sections
        .iter()
        .find(|section| section.id == id)
        .ok_or(BundleError::MissingSection(id))?;
    let start = section.offset as usize;
    let end = start + section.len as usize;
    Ok(&bytes[start..end])
}

fn write_container(source_hash: u64, bundle_hash: u64, sections: Vec<(u32, Vec<u8>)>) -> Vec<u8> {
    let table_len = sections.len() * SECTION_ENTRY_LEN;
    let mut offset = align_to(HEADER_LEN + table_len, 8);
    let mut entries = Vec::with_capacity(sections.len());
    for (id, bytes) in &sections {
        entries.push(SectionEntry {
            id: *id,
            offset: offset as u64,
            len: bytes.len() as u64,
        });
        offset = align_to(offset + bytes.len(), 8);
    }

    let mut out = Vec::with_capacity(offset);
    out.extend_from_slice(MAGIC);
    write_u16(&mut out, FORMAT_VERSION);
    write_u16(&mut out, sections.len() as u16);
    write_u64(&mut out, source_hash);
    write_u64(&mut out, bundle_hash);
    write_u64(&mut out, 0);
    for entry in &entries {
        write_u32(&mut out, entry.id);
        write_u32(&mut out, 0);
        write_u64(&mut out, entry.offset);
        write_u64(&mut out, entry.len);
    }
    for ((_, bytes), entry) in sections.into_iter().zip(entries) {
        while out.len() < entry.offset as usize {
            out.push(0);
        }
        out.extend_from_slice(&bytes);
    }
    out
}

fn hash_sections(sections: &[(u32, Vec<u8>)]) -> u64 {
    let mut bytes = Vec::new();
    for (id, section) in sections {
        write_u32(&mut bytes, *id);
        write_u64(&mut bytes, section.len() as u64);
        bytes.extend_from_slice(section);
    }
    fnv1a64(&bytes)
}

fn interned_strings(bundle: &Bundle) -> Vec<String> {
    let mut strings = BTreeMap::<String, ()>::new();
    let mut insert = |value: &str| {
        strings.insert(value.to_owned(), ());
    };
    for scope in &bundle.scopes {
        insert(scope);
    }
    for language in &bundle.languages {
        insert(&language.canonical);
        insert(&language.scope_name);
        if let Some(first_line) = &language.first_line_pattern {
            insert(first_line);
        }
        for value in language
            .aliases
            .iter()
            .chain(language.extensions.iter())
            .chain(language.basenames.iter())
        {
            insert(value);
        }
    }
    for blob in &bundle.grammar_blobs {
        insert(&blob.language);
        insert(&blob.scope_name);
    }
    for license in &bundle.licenses {
        insert(&license.language);
        insert(&license.source_path);
        insert(&license.upstream_url);
        insert(&license.spdx_id);
        insert(&license.license_text);
        insert(&license.source_revision);
    }
    strings.into_keys().collect()
}

fn string_id(strings: &[String], value: &str) -> u32 {
    strings
        .binary_search_by(|candidate| candidate.as_str().cmp(value))
        .expect("bundle string should be interned") as u32
}

fn optional_string_id(strings: &[String], value: Option<&str>) -> u32 {
    value.map_or(NO_STRING, |value| string_id(strings, value))
}

fn string_by_id(strings: &[String], id: u32) -> Result<String, BundleError> {
    strings
        .get(id as usize)
        .cloned()
        .ok_or(BundleError::BadStringId(id))
}

fn optional_string_by_id(strings: &[String], id: u32) -> Result<Option<String>, BundleError> {
    if id == NO_STRING {
        Ok(None)
    } else {
        string_by_id(strings, id).map(Some)
    }
}

fn encode_string_table(strings: &[String]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut payload = Vec::new();
    let mut offsets = Vec::with_capacity(strings.len());
    for string in strings {
        offsets.push(payload.len() as u32);
        payload.extend_from_slice(string.as_bytes());
    }
    write_u32(&mut bytes, strings.len() as u32);
    write_u32(&mut bytes, payload.len() as u32);
    for offset in offsets {
        write_u32(&mut bytes, offset);
    }
    bytes.extend_from_slice(&payload);
    bytes
}

fn decode_string_table(bytes: &[u8]) -> Result<Vec<String>, BundleError> {
    let mut cursor = Cursor::new(bytes, "string table");
    let count = cursor.u32()? as usize;
    let payload_len = cursor.u32()? as usize;
    let mut offsets = Vec::with_capacity(count);
    for _ in 0..count {
        offsets.push(cursor.u32()? as usize);
    }
    let payload = cursor.bytes(payload_len)?;
    cursor.finish()?;
    let mut strings = Vec::with_capacity(count);
    for index in 0..count {
        let start = offsets[index];
        let end = offsets.get(index + 1).copied().unwrap_or(payload_len);
        if start > end || end > payload.len() {
            return Err(BundleError::Truncated("string table payload"));
        }
        let string = std::str::from_utf8(&payload[start..end])
            .map_err(|_| BundleError::BadUtf8)?
            .to_owned();
        strings.push(string);
    }
    Ok(strings)
}

fn encode_scope_table(scopes: &[String], strings: &[String]) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_u32(&mut bytes, scopes.len() as u32);
    for scope in scopes {
        write_u32(&mut bytes, string_id(strings, scope));
    }
    bytes
}

fn decode_scope_table(bytes: &[u8], strings: &[String]) -> Result<Vec<String>, BundleError> {
    let mut cursor = Cursor::new(bytes, "scope table");
    let count = cursor.u32()?;
    let mut scopes = Vec::with_capacity(count as usize);
    for _ in 0..count {
        scopes.push(string_by_id(strings, cursor.u32()?)?);
    }
    cursor.finish()?;
    Ok(scopes)
}

fn encode_language_table(languages: &[LanguageEntry], strings: &[String]) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_u32(&mut bytes, languages.len() as u32);
    for language in languages {
        write_u32(&mut bytes, string_id(strings, &language.canonical));
        write_u32(&mut bytes, string_id(strings, &language.scope_name));
        write_u32(&mut bytes, language.grammar_blob);
        write_u32(&mut bytes, language.license);
        write_u32(
            &mut bytes,
            optional_string_id(strings, language.first_line_pattern.as_deref()),
        );
        write_string_id_vec(&mut bytes, strings, &language.aliases);
        write_string_id_vec(&mut bytes, strings, &language.extensions);
        write_string_id_vec(&mut bytes, strings, &language.basenames);
    }
    bytes
}

fn decode_language_table(
    bytes: &[u8],
    strings: &[String],
) -> Result<Vec<LanguageEntry>, BundleError> {
    let mut cursor = Cursor::new(bytes, "language table");
    let count = cursor.u32()?;
    let mut languages = Vec::with_capacity(count as usize);
    for _ in 0..count {
        languages.push(LanguageEntry {
            canonical: string_by_id(strings, cursor.u32()?)?,
            scope_name: string_by_id(strings, cursor.u32()?)?,
            grammar_blob: cursor.u32()?,
            license: cursor.u32()?,
            first_line_pattern: optional_string_by_id(strings, cursor.u32()?)?,
            aliases: read_string_id_vec(&mut cursor, strings)?,
            extensions: read_string_id_vec(&mut cursor, strings)?,
            basenames: read_string_id_vec(&mut cursor, strings)?,
        });
    }
    cursor.finish()?;
    Ok(languages)
}

fn encode_grammar_blobs(blobs: &[GrammarBlob], strings: &[String]) -> Vec<u8> {
    let record_len = 48usize;
    let payload_start = 4 + blobs.len() * record_len;
    let mut records = Vec::new();
    let mut payload = Vec::new();
    write_u32(&mut records, blobs.len() as u32);
    for blob in blobs {
        write_u32(&mut records, string_id(strings, &blob.language));
        write_u32(&mut records, string_id(strings, &blob.scope_name));
        write_u32(&mut records, blob.codec);
        write_u32(&mut records, blob.flags);
        write_u32(&mut records, blob.raw_len);
        write_u64(&mut records, (payload_start + payload.len()) as u64);
        write_u64(&mut records, blob.bytes.len() as u64);
        write_u32(&mut records, blob.pattern_count);
        write_u32(&mut records, blob.dfa_count);
        write_u32(&mut records, blob.fallback_count);
        payload.extend_from_slice(&blob.bytes);
    }
    records.extend_from_slice(&payload);
    records
}

fn decode_grammar_blobs(bytes: &[u8], strings: &[String]) -> Result<Vec<GrammarBlob>, BundleError> {
    let mut cursor = Cursor::new(bytes, "grammar blobs");
    let count = cursor.u32()?;
    let mut records = Vec::with_capacity(count as usize);
    for _ in 0..count {
        records.push((
            cursor.u32()?,
            cursor.u32()?,
            cursor.u32()?,
            cursor.u32()?,
            cursor.u32()?,
            cursor.u64()? as usize,
            cursor.u64()? as usize,
            cursor.u32()?,
            cursor.u32()?,
            cursor.u32()?,
        ));
    }
    let mut blobs = Vec::with_capacity(records.len());
    for (
        language,
        scope_name,
        codec,
        flags,
        raw_len,
        offset,
        len,
        pattern_count,
        dfa_count,
        fallback_count,
    ) in records
    {
        if offset.checked_add(len).is_none_or(|end| end > bytes.len()) {
            return Err(BundleError::Truncated("grammar blob payload"));
        }
        blobs.push(GrammarBlob {
            language: string_by_id(strings, language)?,
            scope_name: string_by_id(strings, scope_name)?,
            codec,
            flags,
            raw_len,
            bytes: bytes[offset..offset + len].to_vec(),
            pattern_count,
            dfa_count,
            fallback_count,
        });
    }
    Ok(blobs)
}

fn encode_license_table(licenses: &[LicenseEntry], strings: &[String]) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_u32(&mut bytes, licenses.len() as u32);
    for license in licenses {
        write_u32(&mut bytes, string_id(strings, &license.language));
        write_u32(&mut bytes, string_id(strings, &license.source_path));
        write_u32(&mut bytes, string_id(strings, &license.upstream_url));
        write_u32(&mut bytes, string_id(strings, &license.spdx_id));
        write_u32(&mut bytes, string_id(strings, &license.license_text));
        write_u32(&mut bytes, string_id(strings, &license.source_revision));
    }
    bytes
}

fn decode_license_table(
    bytes: &[u8],
    strings: &[String],
) -> Result<Vec<LicenseEntry>, BundleError> {
    let mut cursor = Cursor::new(bytes, "license table");
    let count = cursor.u32()?;
    let mut licenses = Vec::with_capacity(count as usize);
    for _ in 0..count {
        licenses.push(LicenseEntry {
            language: string_by_id(strings, cursor.u32()?)?,
            source_path: string_by_id(strings, cursor.u32()?)?,
            upstream_url: string_by_id(strings, cursor.u32()?)?,
            spdx_id: string_by_id(strings, cursor.u32()?)?,
            license_text: string_by_id(strings, cursor.u32()?)?,
            source_revision: string_by_id(strings, cursor.u32()?)?,
        });
    }
    cursor.finish()?;
    Ok(licenses)
}

fn write_string_id_vec(out: &mut Vec<u8>, strings: &[String], values: &[String]) {
    write_u32(out, values.len() as u32);
    for value in values {
        write_u32(out, string_id(strings, value));
    }
}

fn read_string_id_vec(
    cursor: &mut Cursor<'_>,
    strings: &[String],
) -> Result<Vec<String>, BundleError> {
    let count = cursor.u32()?;
    let mut values = Vec::with_capacity(count as usize);
    for _ in 0..count {
        values.push(string_by_id(strings, cursor.u32()?)?);
    }
    Ok(values)
}

fn extension_match_len(filename: &str, extension: &str) -> Option<usize> {
    let extension = extension.trim_start_matches('.').to_ascii_lowercase();
    (!extension.is_empty() && filename.ends_with(&format!(".{extension}")))
        .then_some(extension.len())
}

fn normalize_token(token: &str) -> String {
    token.trim().trim_start_matches('.').to_ascii_lowercase()
}

fn align_to(value: usize, alignment: usize) -> usize {
    value.div_ceil(alignment) * alignment
}

fn write_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn read_u16_at(bytes: &[u8], offset: usize) -> Result<u16, BundleError> {
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..offset + 2)
            .ok_or(BundleError::TooShort)?
            .try_into()
            .expect("slice length checked"),
    ))
}

fn read_u32_at(bytes: &[u8], offset: usize) -> Result<u32, BundleError> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or(BundleError::TooShort)?
            .try_into()
            .expect("slice length checked"),
    ))
}

fn read_u64_at(bytes: &[u8], offset: usize) -> Result<u64, BundleError> {
    Ok(u64::from_le_bytes(
        bytes
            .get(offset..offset + 8)
            .ok_or(BundleError::TooShort)?
            .try_into()
            .expect("slice length checked"),
    ))
}

struct Cursor<'a> {
    bytes: &'a [u8],
    cursor: usize,
    name: &'static str,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8], name: &'static str) -> Self {
        Self {
            bytes,
            cursor: 0,
            name,
        }
    }

    fn u32(&mut self) -> Result<u32, BundleError> {
        let value =
            read_u32_at(self.bytes, self.cursor).map_err(|_| BundleError::Truncated(self.name))?;
        self.cursor += 4;
        Ok(value)
    }

    fn u64(&mut self) -> Result<u64, BundleError> {
        let value =
            read_u64_at(self.bytes, self.cursor).map_err(|_| BundleError::Truncated(self.name))?;
        self.cursor += 8;
        Ok(value)
    }

    fn bytes(&mut self, len: usize) -> Result<&'a [u8], BundleError> {
        let end = self
            .cursor
            .checked_add(len)
            .ok_or(BundleError::Truncated(self.name))?;
        let bytes = self
            .bytes
            .get(self.cursor..end)
            .ok_or(BundleError::Truncated(self.name))?;
        self.cursor = end;
        Ok(bytes)
    }

    fn finish(&self) -> Result<(), BundleError> {
        if self.cursor == self.bytes.len() {
            Ok(())
        } else {
            Err(BundleError::TrailingBytes(self.name))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_bundle() -> Bundle {
        Bundle {
            source_hash: 7,
            bundle_hash: 0,
            strings: Vec::new(),
            scopes: vec!["source.rust".to_owned()],
            languages: vec![LanguageEntry {
                canonical: "rust".to_owned(),
                scope_name: "source.rust".to_owned(),
                aliases: vec!["rs".to_owned()],
                extensions: vec!["rs".to_owned()],
                basenames: vec!["Cargo.toml".to_owned()],
                first_line_pattern: None,
                grammar_blob: 0,
                license: 0,
            }],
            grammar_blobs: vec![GrammarBlob {
                language: "rust".to_owned(),
                scope_name: "source.rust".to_owned(),
                codec: CODEC_NONE,
                flags: 0,
                raw_len: 2,
                bytes: b"{}".to_vec(),
                pattern_count: 0,
                dfa_count: 0,
                fallback_count: 0,
            }],
            licenses: vec![LicenseEntry {
                language: "rust".to_owned(),
                source_path: "assets/grammars/languages/rust.tmLanguage.json".to_owned(),
                upstream_url: "https://example.invalid".to_owned(),
                spdx_id: "MIT".to_owned(),
                license_text: String::new(),
                source_revision: "test".to_owned(),
            }],
        }
    }

    #[test]
    fn reads_header_and_roundtrips_bundle() {
        let bytes = sample_bundle().to_bytes();
        let header = read_header(&bytes).unwrap();
        assert_eq!(header.format_version, FORMAT_VERSION);
        assert_eq!(header.section_count, 5);
        assert_eq!(header.source_hash, 7);
        assert_ne!(header.bundle_hash, 0);
        let parsed = Bundle::parse(&bytes).unwrap();
        assert_eq!(
            parsed.version_stamp(),
            format!("{:016x}", header.bundle_hash)
        );
        assert_eq!(parsed.available_languages(), vec!["rust"]);
        assert_eq!(parsed.canonical_language("RS"), Some("rust"));
        assert_eq!(parsed.detect_language_from_path("src/lib.rs"), Some("rust"));
    }

    #[test]
    fn rejects_bad_magic() {
        let mut bytes = sample_bundle().to_bytes();
        bytes[0] = b'X';
        assert_eq!(Bundle::parse(&bytes), Err(BundleError::BadMagic));
    }

    #[test]
    fn registry_decodes_grammar_blob_lazily() {
        let mut bundle = sample_bundle();
        bundle.languages[0].canonical = "fixture".to_owned();
        bundle.languages[0].aliases = vec!["fx".to_owned()];
        bundle.grammar_blobs[0].language = "fixture".to_owned();
        bundle.grammar_blobs[0].bytes = br#"{"scopeName":"source.fixture","patterns":[{"match":"true","name":"constant.language.fixture"}]}"#.to_vec();
        bundle.grammar_blobs[0].raw_len = bundle.grammar_blobs[0].bytes.len() as u32;
        let parsed = Bundle::parse(&bundle.to_bytes()).unwrap();
        let mut registry = BundleGrammarRegistry::new(parsed);
        let grammar = registry.grammar("fx").unwrap();
        assert_eq!(grammar.scope_name, "source.fixture");
        assert_eq!(grammar.patterns, vec!["true".to_owned()]);
    }

    #[test]
    fn deterministic_output() {
        let bundle = sample_bundle();
        assert_eq!(bundle.to_bytes(), bundle.to_bytes());
    }
}
