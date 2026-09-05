use std::collections::{HashMap, VecDeque};

use sha2::{Digest, Sha256};

use crate::report::{DiagnosticCode, ParseReport};
use crate::{Error, PageManifest, ParseLimits, Result, StoredNote, StoredObject, StoredPage};

const PAGE_SIGNATURE: &[u8] = b"Page for SAMSUNG S-Pen SDK";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IntegrityCounts {
    pub matched: usize,
    pub mismatched: usize,
    pub unavailable: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IntegrityReport {
    pub note: IntegrityCounts,
    pub objects: IntegrityCounts,
    pub layers: IntegrityCounts,
    pub pages: IntegrityCounts,
    pub manifest: IntegrityCounts,
}

enum Scope {
    Note,
    Object,
    Layer,
    Page,
    Manifest,
}

#[derive(Default)]
pub(crate) struct IntegrityVerifier {
    summary: IntegrityReport,
    diagnostics: ParseReport,
    note_hash: Option<[u8; 32]>,
    page_hashes: HashMap<String, VecDeque<Option<[u8; 32]>>>,
}

impl IntegrityVerifier {
    pub(crate) fn verify_note(&mut self, data: &[u8], note: &StoredNote) {
        let Some((payload, stored)) = data.split_last_chunk::<32>() else {
            self.unavailable(Scope::Note, "note.note", "note hash trailer is absent");
            return;
        };
        let flexible_offset = note.header.integrity_offset as usize;
        if payload.len() < note.fixed_data_end
            || flexible_offset < note.fixed_data_end
            || flexible_offset > payload.len()
        {
            self.unavailable(
                Scope::Note,
                "note.note",
                "note data overlaps or extends past the hash trailer",
            );
            return;
        }
        self.note_hash = Some(*stored);
        self.compare(
            Scope::Note,
            "note.note",
            "note payload",
            *stored,
            Sha256::digest(payload).into(),
        );
    }

    pub(crate) fn verify_page(
        &mut self,
        data: &[u8],
        page: &StoredPage,
        entry: &str,
        limits: &ParseLimits,
    ) -> Result<()> {
        let mut page_digest = Sha256::new();
        for layer in &page.layers.layers {
            let mut layer_digest = Sha256::new();
            self.verify_objects(data, &layer.objects, entry, &mut layer_digest);
            let label = format!("layer at 0x{:x}", layer.header_offset);
            match layer.metadata_with_limits(data, limits) {
                Ok(metadata) => {
                    if let (Some(uuid), Some(modified_time)) =
                        (metadata.uuid, metadata.modified_time)
                    {
                        layer_digest.update(identity_hash(&uuid, modified_time));
                        self.compare(
                            Scope::Layer,
                            entry,
                            &label,
                            layer.integrity_trailer,
                            layer_digest.finalize().into(),
                        );
                    } else {
                        self.unavailable(
                            Scope::Layer,
                            entry,
                            format!("{label}: UUID or modification time is absent"),
                        );
                    }
                }
                Err(error @ Error::LimitExceeded { .. }) => return Err(error),
                Err(error) => self.unavailable(Scope::Layer, entry, format!("{label}: {error}")),
            }
            page_digest.update(layer.integrity_trailer);
        }

        let footer = data.get(page.integrity_offset..).unwrap_or_default();
        let stored = if footer.len() == 32 + PAGE_SIGNATURE.len() && footer[32..] == *PAGE_SIGNATURE
        {
            Some(footer[..32].try_into().unwrap())
        } else {
            self.unavailable(
                Scope::Page,
                entry,
                "page hash or signature is absent, truncated or followed by unexpected bytes",
            );
            None
        };
        if let Some(stored) = stored {
            if let Some(modified_time) = page.header.modified_time_raw {
                page_digest.update(identity_hash(&page.header.uuid, modified_time as i64));
                self.compare(
                    Scope::Page,
                    entry,
                    "page logical hash",
                    stored,
                    page_digest.finalize().into(),
                );
            } else {
                self.unavailable(Scope::Page, entry, "page modification time is absent");
            }
        }
        self.page_hashes
            .entry(page.header.uuid.clone())
            .or_default()
            .push_back(stored);
        Ok(())
    }

    fn verify_objects(
        &mut self,
        data: &[u8],
        objects: &[StoredObject],
        entry: &str,
        layer_digest: &mut Sha256,
    ) {
        for object in objects {
            let label = format!("object at 0x{:x}", object.payload_offset);
            match object.base_metadata(data) {
                Ok(metadata) => self.compare(
                    Scope::Object,
                    entry,
                    &label,
                    object.integrity_trailer,
                    identity_hash(&metadata.uuid, metadata.modified_time_raw),
                ),
                Err(error) => self.unavailable(Scope::Object, entry, format!("{label}: {error}")),
            }
            layer_digest.update(object.integrity_trailer);
            self.verify_objects(data, &object.children, entry, layer_digest);
        }
    }

    pub(crate) fn finish(
        mut self,
        manifest: Option<&PageManifest>,
        report: &mut ParseReport,
    ) -> IntegrityReport {
        if let Some(manifest) = manifest {
            if let Some(stored) = self.note_hash {
                self.compare(
                    Scope::Manifest,
                    "pageIdInfo.dat",
                    "note trailer link",
                    manifest.integrity_header,
                    stored,
                );
            } else {
                self.unavailable(
                    Scope::Manifest,
                    "pageIdInfo.dat",
                    "note trailer link: no readable note hash",
                );
            }
            for (index, record) in manifest.entries.iter().enumerate() {
                let stored = self
                    .page_hashes
                    .get_mut(&record.page_id)
                    .and_then(VecDeque::pop_front)
                    .flatten();
                let label = format!("page link at manifest index {index}");
                if let Some(stored) = stored {
                    self.compare(
                        Scope::Manifest,
                        "pageIdInfo.dat",
                        &label,
                        record.integrity_hash,
                        stored,
                    );
                } else {
                    self.unavailable(
                        Scope::Manifest,
                        "pageIdInfo.dat",
                        format!("{label}: matching page hash is unavailable"),
                    );
                }
            }
        } else {
            self.unavailable(
                Scope::Manifest,
                "pageIdInfo.dat",
                "manifest links cannot be checked because the manifest is absent",
            );
        }
        report.diagnostics.extend(self.diagnostics.diagnostics);
        self.summary
    }

    fn counts(&mut self, scope: Scope) -> &mut IntegrityCounts {
        match scope {
            Scope::Note => &mut self.summary.note,
            Scope::Object => &mut self.summary.objects,
            Scope::Layer => &mut self.summary.layers,
            Scope::Page => &mut self.summary.pages,
            Scope::Manifest => &mut self.summary.manifest,
        }
    }

    fn compare(
        &mut self,
        scope: Scope,
        entry: &str,
        label: &str,
        stored: [u8; 32],
        computed: [u8; 32],
    ) {
        if stored == computed {
            self.counts(scope).matched += 1;
        } else {
            self.counts(scope).mismatched += 1;
            self.diagnostics.warning(
                DiagnosticCode::IntegrityMismatch,
                Some(entry.into()),
                format!(
                    "{label}: stored {}, computed {}",
                    hex_hash(&stored),
                    hex_hash(&computed)
                ),
            );
        }
    }

    fn unavailable(&mut self, scope: Scope, entry: &str, message: impl Into<String>) {
        self.counts(scope).unavailable += 1;
        self.diagnostics.warning(
            DiagnosticCode::IntegrityUnavailable,
            Some(entry.into()),
            message,
        );
    }
}

fn identity_hash(uuid: &str, modified_time: i64) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(uuid.as_bytes());
    digest.update(modified_time.to_string().as_bytes());
    digest.finalize().into()
}

fn hex_hash(hash: &[u8; 32]) -> String {
    hash.iter().map(|byte| format!("{byte:02x}")).collect()
}
