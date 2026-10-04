use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct MediaManifestSummary<'a> {
    format_version: u32,
    entries: Vec<MediaManifestEntrySummary<'a>>,
    trailing_byte_length: usize,
}

#[derive(Serialize)]
struct MediaManifestEntrySummary<'a> {
    bind_id: u32,
    file_name: &'a str,
    /// The native record's digest; source content is not verified here.
    sha256: Option<&'a str>,
    reference_count: u16,
    modified_time_raw: String,
    is_attached: bool,
    trailing_byte_length: usize,
}

impl<'a> From<&'a sdocx::MediaManifest> for MediaManifestSummary<'a> {
    fn from(manifest: &'a sdocx::MediaManifest) -> Self {
        Self {
            format_version: manifest.format_version,
            entries: manifest
                .entries
                .iter()
                .map(|entry| MediaManifestEntrySummary {
                    bind_id: entry.bind_id,
                    file_name: &entry.file_name,
                    sha256: entry.sha256.as_deref(),
                    reference_count: entry.reference_count,
                    modified_time_raw: entry.modified_time_raw.to_string(),
                    is_attached: entry.is_attached,
                    trailing_byte_length: entry.trailing_data.len(),
                })
                .collect(),
            trailing_byte_length: manifest.trailing_data.len(),
        }
    }
}

#[derive(Serialize)]
pub(crate) struct ArchiveResourceSummary<'a> {
    resource_index: usize,
    name: &'a str,
    archive_id: Option<u32>,
    kind: ResourceKind,
    media_index: Option<usize>,
    data_available: bool,
    byte_length: Option<usize>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum ResourceKind {
    MediaAsset,
    Opaque,
}

pub(crate) fn archive_resources(
    metadata: &sdocx::DocumentMetadata,
) -> Vec<ArchiveResourceSummary<'_>> {
    metadata
        .archive_resources
        .iter()
        .enumerate()
        .map(|(resource_index, resource)| {
            let (kind, media_index) = match &resource.content {
                sdocx::ArchiveResourceContent::MediaAsset { media_index } => {
                    (ResourceKind::MediaAsset, Some(*media_index))
                }
                sdocx::ArchiveResourceContent::Opaque { .. } => (ResourceKind::Opaque, None),
            };
            let data = resource.data(&metadata.media_assets);
            ArchiveResourceSummary {
                resource_index,
                name: &resource.name,
                archive_id: resource.archive_id,
                kind,
                media_index,
                data_available: data.is_some(),
                byte_length: data.map(<[u8]>::len),
            }
        })
        .collect()
}

#[derive(Serialize)]
pub(crate) struct PageManifestSummary<'a> {
    integrity_header: &'a [u8],
    entries: &'a [sdocx::PageManifestEntry],
    trailing_byte_length: usize,
}

impl<'a> From<&'a sdocx::PageManifest> for PageManifestSummary<'a> {
    fn from(manifest: &'a sdocx::PageManifest) -> Self {
        Self {
            integrity_header: &manifest.integrity_header,
            entries: &manifest.entries,
            trailing_byte_length: manifest.trailing_data.len(),
        }
    }
}

#[cfg(test)]
#[path = "source_summary_tests.rs"]
mod tests;
