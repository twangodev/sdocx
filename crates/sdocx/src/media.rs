use crate::binary::Reader;
use crate::{Error, ParseLimits, Result};
use std::collections::HashMap;

pub(crate) struct MediaResolver {
    bindings: HashMap<u32, std::result::Result<usize, String>>,
    inferred: bool,
}

impl MediaResolver {
    pub(crate) fn new(
        manifest: Option<&MediaManifest>,
        assets: &[crate::MediaAsset],
        names: &HashMap<String, usize>,
    ) -> Self {
        let mut bindings = HashMap::new();
        let indexes: HashMap<_, _> = assets
            .iter()
            .enumerate()
            .map(|(i, asset)| (asset.name.as_str(), i))
            .collect();
        let records: Vec<_> = if let Some(manifest) = manifest {
            manifest
                .entries
                .iter()
                .map(|entry| (entry.bind_id, format!("media/{}", entry.file_name)))
                .collect()
        } else {
            names
                .keys()
                .filter(|name| name.starts_with("media/"))
                .filter_map(|name| media_archive_id(name).map(|id| (id, name.clone())))
                .collect()
        };
        for (id, name) in records {
            let resolved = match names.get(&name).copied().unwrap_or(0) {
                0 => Err(format!("media ID {id} names missing archive entry {name}")),
                1 => indexes
                    .get(name.as_str())
                    .copied()
                    .ok_or_else(|| format!("media ID {id} names unsupported media {name}")),
                _ => Err(format!(
                    "media ID {id} names duplicate archive entry {name}"
                )),
            };
            match bindings.entry(id) {
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(resolved);
                }
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    *entry.get_mut() = Err(format!("media ID {id} has ambiguous bindings"));
                }
            }
        }
        Self {
            bindings,
            inferred: manifest.is_none(),
        }
    }

    pub(crate) fn resolve(&self, id: Option<u32>) -> std::result::Result<(usize, bool), String> {
        let id = id.ok_or_else(|| "image has no supported main media reference".to_owned())?;
        match self.bindings.get(&id) {
            Some(Ok(index)) => Ok((*index, self.inferred)),
            Some(Err(message)) => Err(message.clone()),
            None => Err(format!("media ID {id} has no binding")),
        }
    }
}

pub(crate) fn media_archive_id(name: &str) -> Option<u32> {
    name.rsplit('/').next()?.split('@').next()?.parse().ok()
}

/// Modern `media/mediaInfo.dat` (format versions newer than 3001).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct MediaManifest {
    /// Native format version, independent of the archive's filename.
    pub format_version: u32,
    /// Bind IDs and filenames in manifest order. IDs need not be consecutive.
    pub entries: Vec<MediaManifestEntry>,
    /// Bytes following the `EOFX` marker, retained for forward compatibility.
    pub trailing_data: Vec<u8>,
}

/// One size-bounded native media record.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct MediaManifestEntry {
    /// ID referenced by native objects; authoritative over filename prefixes.
    pub bind_id: u32,
    /// Filename relative to the archive's `media/` directory.
    pub file_name: String,
    /// Recorded SHA-256 hexadecimal digest. Parsing does not verify the asset hash.
    pub sha256: Option<String>,
    /// Native reference count.
    pub reference_count: u16,
    /// Raw modification timestamp.
    pub modified_time_raw: i64,
    /// Whether the native writer marked this asset as attached.
    pub is_attached: bool,
    /// Uninterpreted bytes inside the record after the known fields.
    pub trailing_data: Vec<u8>,
}

/// Parse a modern media manifest without loading any referenced files.
pub fn parse_media_manifest_bytes(data: &[u8]) -> Result<MediaManifest> {
    parse_media_manifest_bytes_with_limits(data, &ParseLimits::default())
}

/// Parse a modern media manifest with the archive-entry limit applied to its count.
pub fn parse_media_manifest_bytes_with_limits(
    data: &[u8],
    limits: &ParseLimits,
) -> Result<MediaManifest> {
    let mut reader = Reader::new(data, "media manifest");
    let format_version = reader.read_u32("format version")?;
    if format_version <= 3001 {
        return Err(Error::Format(
            "legacy media manifests are not supported".into(),
        ));
    }
    let count = usize::from(reader.read_u16("media count")?);
    if count > limits.max_archive_entries {
        return Err(Error::LimitExceeded {
            resource: "media manifest entries",
            limit: limits.max_archive_entries as u64,
            actual: count as u64,
        });
    }
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let size = reader.read_u32("media record size")? as usize;
        let mut record = Reader::new(reader.read_bytes(size, "media record")?, "media record");
        let bind_id = record.read_u32("bind ID")?;
        let file_name = record.read_utf16_u16("filename")?;
        let prefix = record.read_bytes(2, "file hash")?;
        let sha256 = if prefix == [0, 0] {
            None
        } else {
            let mut hash = prefix.to_vec();
            hash.extend_from_slice(record.read_bytes(62, "file hash")?);
            if !hash.iter().all(u8::is_ascii_hexdigit) {
                return Err(Error::Format(
                    "media record: invalid SHA-256 hexadecimal digest".into(),
                ));
            }
            Some(String::from_utf8(hash).expect("hexadecimal ASCII"))
        };
        let reference_count = record.read_u16("reference count")?;
        let modified_time_raw = record.read_i64("modification timestamp")?;
        let is_attached = record.read_u8("attached flag")? != 0;
        let trailing_data = record
            .read_bytes(record.remaining(), "media record extensions")?
            .to_vec();
        entries.push(MediaManifestEntry {
            bind_id,
            file_name,
            sha256,
            reference_count,
            modified_time_raw,
            is_attached,
            trailing_data,
        });
    }
    if reader.read_bytes(4, "end marker")? != b"EOFX" {
        return Err(Error::Format("media manifest: invalid EOFX marker".into()));
    }
    Ok(MediaManifest {
        format_version,
        entries,
        trailing_data: reader
            .read_bytes(reader.remaining(), "manifest extensions")?
            .to_vec(),
    })
}
