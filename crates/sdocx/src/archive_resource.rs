use std::collections::{HashMap, HashSet};

use crate::media::MediaBindings;
use crate::{DocumentMetadata, MediaAsset};

/// A retained file from the archive's `media/` directory, independent of rendering support.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ArchiveResource {
    /// ZIP entry path, including the `media/` prefix.
    pub name: String,
    /// Numeric filename prefix, when present; this is not the authoritative bind ID.
    pub archive_id: Option<u32>,
    /// The single owner of this resource's source bytes.
    pub content: ArchiveResourceContent,
}

/// Source-byte ownership for one retained archive resource.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ArchiveResourceContent {
    /// Bytes owned by `DocumentMetadata::media_assets`, admitted by the image extraction path.
    MediaAsset { media_index: usize },
    /// Bytes retained without interpreting their format or admitting them to image rendering.
    Opaque { data: Vec<u8> },
}

impl ArchiveResource {
    /// Return source bytes, rejecting a stale image index after caller mutation.
    pub fn data<'a>(&'a self, media_assets: &'a [MediaAsset]) -> Option<&'a [u8]> {
        match &self.content {
            ArchiveResourceContent::MediaAsset { media_index } => media_assets
                .get(*media_index)
                .filter(|asset| asset.name == self.name)
                .map(|asset| asset.data.as_slice()),
            ArchiveResourceContent::Opaque { data } => Some(data),
        }
    }
}

/// A source binding whose bytes are retained; this does not certify a supported format.
#[derive(Debug)]
pub struct ResolvedArchiveResource<'a> {
    pub resource: &'a ArchiveResource,
    pub data: &'a [u8],
    /// The archive lacked a manifest, so the binding came from a numeric filename prefix.
    pub inferred: bool,
}

/// Failure to resolve a source reference, independent of native decoding or drawing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ArchiveResourceError {
    #[error("media ID {id} has no binding")]
    UnboundId { id: u32 },
    #[error("media ID {id} has ambiguous bindings")]
    AmbiguousBinding { id: u32 },
    #[error("media ID {id} names missing archive entry {name}")]
    MissingEntry { id: u32, name: String },
    #[error("archive entry {name} has ambiguous source files")]
    AmbiguousEntry { name: String },
    #[error("source bytes for archive entry {name} are unavailable")]
    DataUnavailable { name: String },
}

/// Borrowed source lookup prepared once for a document's media bindings.
pub struct ArchiveResourceResolver<'a> {
    metadata: &'a DocumentMetadata,
    bindings: MediaBindings,
    resources: HashMap<&'a str, Option<&'a ArchiveResource>>,
}

impl<'a> ArchiveResourceResolver<'a> {
    pub(crate) fn new(metadata: &'a DocumentMetadata) -> Self {
        let mut resources = HashMap::new();
        for resource in &metadata.archive_resources {
            resources
                .entry(resource.name.as_str())
                .and_modify(|entry| *entry = None)
                .or_insert(Some(resource));
        }
        let names: HashSet<_> = resources.keys().map(|name| (*name).to_owned()).collect();
        Self {
            metadata,
            bindings: MediaBindings::new(metadata.media_manifest.as_ref(), &names),
            resources,
        }
    }

    /// Resolve a raw archive bind ID. Callers must validate signed object references
    /// and sentinel meanings before requesting source lookup.
    pub fn resolve(&self, id: u32) -> Result<ResolvedArchiveResource<'a>, ArchiveResourceError> {
        let (name, inferred) = self.bindings.resolve(id)?;
        let resource = self.resources.get(name).copied().ok_or_else(|| {
            ArchiveResourceError::DataUnavailable {
                name: name.to_owned(),
            }
        })?;
        let resource = resource.ok_or_else(|| ArchiveResourceError::AmbiguousEntry {
            name: name.to_owned(),
        })?;
        let data = resource.data(&self.metadata.media_assets).ok_or_else(|| {
            ArchiveResourceError::DataUnavailable {
                name: name.to_owned(),
            }
        })?;
        Ok(ResolvedArchiveResource {
            resource,
            data,
            inferred,
        })
    }
}

impl DocumentMetadata {
    /// Prepare authoritative source lookup without adding image rendering support.
    pub fn archive_resource_resolver(&self) -> ArchiveResourceResolver<'_> {
        ArchiveResourceResolver::new(self)
    }

    /// Resolve one source binding, preserving missing and ambiguous references.
    pub fn resolve_archive_resource(
        &self,
        id: u32,
    ) -> Result<ResolvedArchiveResource<'_>, ArchiveResourceError> {
        self.archive_resource_resolver().resolve(id)
    }
}
