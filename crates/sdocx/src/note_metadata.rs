use crate::binary::Reader;
use crate::{Error, ParseLimits, Result, StoredNote};

#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct NoteMetadata {
    pub application_name: Option<String>,
    pub application_version: Option<NoteApplicationVersion>,
    pub author: Option<NoteAuthor>,
    pub location: Option<NoteLocation>,
    pub template_uri: Option<String>,
    pub last_edited_page_index: Option<i32>,
    pub last_edited_page: Option<NotePageEdit>,
    pub string_table: Option<NoteStringTable>,
    pub body_font_size_delta: Option<i32>,
    pub compatible_pen: Option<NotePenSettings>,
    pub voices: Option<Vec<NoteVoice>>,
    pub attachments: Option<Vec<NoteAttachment>>,
    pub pen: Option<NotePenSettings>,
    pub server_checkpoint: Option<i64>,
    pub fixed_font: Option<String>,
    pub fixed_text_direction: Option<i32>,
    pub fixed_background_theme: Option<i32>,
    pub text_summarization: Option<String>,
    pub stroke_group_size: Option<i32>,
    pub app_custom_data: Option<String>,
    pub first_unparsed_field: Option<usize>,
    pub trailing_data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct NoteApplicationVersion {
    pub major: i32,
    pub minor: i32,
    pub patch_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct NoteAuthor {
    pub name: Option<String>,
    pub phone_number: Option<String>,
    pub email: Option<String>,
    pub image_media_id: u32,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct NoteLocation {
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct NotePageEdit {
    pub image_media_id: u32,
    pub time: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct NoteStringTable {
    pub entries: Vec<NoteStringId>,
    pub trailing_data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct NoteStringId {
    pub id: u32,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct NoteAttachment {
    pub name: String,
    pub media_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct NoteVoice {
    pub media_id: u32,
    pub name: String,
    pub play_time: String,
    pub created_time: i64,
    pub events: Vec<NoteVoiceEvent>,
    pub recording_time: Option<i64>,
    pub trailing_data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct NoteVoiceEvent {
    pub action: i32,
    pub time: i64,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct NotePenSettings {
    pub name: String,
    pub size: f32,
    pub color: u32,
    pub curvable: bool,
    pub advanced_setting: String,
    pub eraser_enabled: bool,
    pub size_level: i32,
    pub particle_density: i32,
    pub particle_size: Option<f32>,
    pub fixed_width: Option<bool>,
    pub hsv: [f32; 3],
    pub color_ui_info: u32,
    pub extension: Option<NotePenExtension>,
    pub trailing_data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct NotePenExtension {
    pub fixed_opacity: bool,
    pub auto_size_enabled: bool,
    pub fit_ratio: f32,
}

impl StoredNote {
    pub fn metadata(&self, note_bytes: &[u8]) -> Result<NoteMetadata> {
        self.metadata_with_limits(note_bytes, &ParseLimits::default())
    }

    pub fn metadata_with_limits(
        &self,
        note_bytes: &[u8],
        limits: &ParseLimits,
    ) -> Result<NoteMetadata> {
        if note_bytes.len() as u64 > limits.max_entry_size {
            return Err(Error::LimitExceeded {
                resource: "note size",
                limit: limits.max_entry_size,
                actual: note_bytes.len() as u64,
            });
        }
        let payload_end = note_bytes
            .len()
            .checked_sub(32)
            .ok_or_else(|| Error::Format("note metadata: note hash trailer is absent".into()))?;
        let flexible_offset = self.header.flexible_data_offset() as usize;
        if flexible_offset < self.fixed_data_end || flexible_offset > payload_end {
            return Err(Error::Format(
                "note metadata: flexible data is outside the note payload".into(),
            ));
        }
        let mut decoder = NoteMetadataDecoder { limits, entries: 0 };
        decoder.parse(
            &note_bytes[flexible_offset..payload_end],
            &self.header.field_mask,
        )
    }
}

struct NoteMetadataDecoder<'a> {
    limits: &'a ParseLimits,
    entries: usize,
}

impl NoteMetadataDecoder<'_> {
    fn parse(&mut self, data: &[u8], fields: &[u8]) -> Result<NoteMetadata> {
        let mut reader = Reader::new(data, "note flexible data");
        let mut metadata = NoteMetadata::default();
        for bit in 0..fields.len() * 8 {
            if fields[bit / 8] & (1 << (bit % 8)) == 0 {
                continue;
            }
            match bit {
                0 => {
                    metadata.application_name = Some(self.string(&mut reader, "application name")?)
                }
                1 => {
                    metadata.application_version = Some(NoteApplicationVersion {
                        major: reader.read_i32("application major version")?,
                        minor: reader.read_i32("application minor version")?,
                        patch_name: self.string(&mut reader, "application patch name")?,
                    });
                }
                2 => {
                    metadata.author = Some(NoteAuthor {
                        name: reader.read_nullable_utf16_u16(
                            "author name",
                            self.limits.max_text_characters,
                        )?,
                        phone_number: reader.read_nullable_utf16_u16(
                            "author phone number",
                            self.limits.max_text_characters,
                        )?,
                        email: reader.read_nullable_utf16_u16(
                            "author email",
                            self.limits.max_text_characters,
                        )?,
                        image_media_id: reader.read_u32("author image media ID")?,
                    });
                }
                3 => {
                    metadata.location = Some(NoteLocation {
                        latitude: reader.read_f64("latitude")?,
                        longitude: reader.read_f64("longitude")?,
                    });
                }
                6 => metadata.template_uri = Some(self.string(&mut reader, "template URI")?),
                7 => {
                    metadata.last_edited_page_index =
                        Some(reader.read_i32("last-edited page index")?)
                }
                9 => {
                    metadata.last_edited_page = Some(NotePageEdit {
                        image_media_id: reader.read_u32("last-edited page image media ID")?,
                        time: reader.read_i64("last-edited page time")?,
                    });
                }
                10 => metadata.string_table = Some(self.string_table(&mut reader)?),
                11 => {
                    metadata.body_font_size_delta = Some(reader.read_i32("body font size delta")?)
                }
                12 => metadata.compatible_pen = Some(self.pen(&mut reader, false)?),
                13 => metadata.voices = Some(self.voices(&mut reader)?),
                14 => metadata.attachments = Some(self.attachments(&mut reader)?),
                15 => {
                    let size = reader.read_u32("pen block size")?;
                    let payload_size = size.checked_sub(4).ok_or_else(|| {
                        Error::Format(
                            "note metadata: pen block is smaller than its size prefix".into(),
                        )
                    })? as usize;
                    let mut pen_reader = Reader::new(
                        reader.read_bytes(payload_size, "pen block")?,
                        "note pen block",
                    );
                    let mut pen = self.pen(&mut pen_reader, true)?;
                    if pen_reader.remaining() != 0 {
                        pen.extension = Some(NotePenExtension {
                            fixed_opacity: pen_reader.read_u32("fixed opacity")? != 0,
                            auto_size_enabled: pen_reader.read_u32("automatic size")? != 0,
                            fit_ratio: pen_reader.read_f32("fit ratio")?,
                        });
                    }
                    pen.trailing_data = self.trailing(&mut pen_reader)?;
                    metadata.pen = Some(pen);
                }
                16 => metadata.server_checkpoint = Some(reader.read_i64("server checkpoint")?),
                17 => metadata.fixed_font = Some(self.string(&mut reader, "fixed font")?),
                18 => {
                    metadata.fixed_text_direction = Some(reader.read_i32("fixed text direction")?)
                }
                19 => {
                    metadata.fixed_background_theme =
                        Some(reader.read_i32("fixed background theme")?)
                }
                20 => {
                    metadata.text_summarization =
                        Some(self.string(&mut reader, "text summarization")?)
                }
                21 => metadata.stroke_group_size = Some(reader.read_i32("stroke group size")?),
                22 => {
                    metadata.app_custom_data = Some(reader.read_utf16_u32(
                        "application custom data",
                        self.limits.max_text_characters,
                    )?)
                }
                _ => {
                    metadata.first_unparsed_field = Some(bit);
                    break;
                }
            }
        }
        metadata.trailing_data = self.trailing(&mut reader)?;
        Ok(metadata)
    }

    fn string(&self, reader: &mut Reader<'_>, field: &'static str) -> Result<String> {
        reader.read_utf16_u16_with_limit(field, self.limits.max_text_characters)
    }

    fn trailing(&self, reader: &mut Reader<'_>) -> Result<Vec<u8>> {
        Ok(reader
            .read_bytes(reader.remaining(), "trailing data")?
            .to_vec())
    }

    fn reserve_entries(
        &mut self,
        reader: &Reader<'_>,
        count: usize,
        minimum_size: usize,
    ) -> Result<()> {
        let total = self
            .entries
            .checked_add(count)
            .ok_or_else(|| Error::Format("note metadata: entry count overflows".into()))?;
        if total > self.limits.max_note_metadata_entries {
            return Err(Error::LimitExceeded {
                resource: "note metadata entries",
                limit: self.limits.max_note_metadata_entries as u64,
                actual: total as u64,
            });
        }
        if count > reader.remaining() / minimum_size {
            return Err(Error::Format(
                "note metadata: entry count exceeds its bounded payload".into(),
            ));
        }
        self.entries = total;
        Ok(())
    }

    fn string_table(&mut self, reader: &mut Reader<'_>) -> Result<NoteStringTable> {
        let size = reader.read_u32("string table size")? as usize;
        let mut reader = Reader::new(
            reader.read_bytes(size, "string table")?,
            "note string table",
        );
        let count = usize::from(reader.read_u16("string count")?);
        self.reserve_entries(&reader, count, 6)?;
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            entries.push(NoteStringId {
                id: reader.read_u32("string ID")?,
                text: self.string(&mut reader, "string text")?,
            });
        }
        Ok(NoteStringTable {
            entries,
            trailing_data: self.trailing(&mut reader)?,
        })
    }

    fn attachments(&mut self, reader: &mut Reader<'_>) -> Result<Vec<NoteAttachment>> {
        let count = usize::from(reader.read_u16("attachment count")?);
        self.reserve_entries(reader, count, 6)?;
        let mut attachments = Vec::with_capacity(count);
        for _ in 0..count {
            attachments.push(NoteAttachment {
                name: self.string(reader, "attachment name")?,
                media_id: reader.read_u32("attachment media ID")?,
            });
        }
        Ok(attachments)
    }

    fn voices(&mut self, reader: &mut Reader<'_>) -> Result<Vec<NoteVoice>> {
        let count = reader.read_u32("voice count")? as usize;
        self.reserve_entries(reader, count, 24)?;
        let mut voices = Vec::with_capacity(count);
        for _ in 0..count {
            let size = reader.read_u32("voice record size")? as usize;
            let mut voice = Reader::new(
                reader.read_bytes(size, "voice record")?,
                "note voice record",
            );
            let media_id = voice.read_u32("voice media ID")?;
            let name = self.string(&mut voice, "voice name")?;
            let play_time = self.string(&mut voice, "voice play time")?;
            let created_time = voice.read_i64("voice creation time")?;
            let event_count = voice.read_u32("voice event count")? as usize;
            self.reserve_entries(&voice, event_count, 12)?;
            let mut events = Vec::with_capacity(event_count);
            for _ in 0..event_count {
                events.push(NoteVoiceEvent {
                    action: voice.read_i32("voice action")?,
                    time: voice.read_i64("voice event time")?,
                });
            }
            let recording_time = (voice.remaining() != 0)
                .then(|| voice.read_i64("recording time"))
                .transpose()?;
            voices.push(NoteVoice {
                media_id,
                name,
                play_time,
                created_time,
                events,
                recording_time,
                trailing_data: self.trailing(&mut voice)?,
            });
        }
        Ok(voices)
    }

    fn pen(&self, reader: &mut Reader<'_>, modern: bool) -> Result<NotePenSettings> {
        let name = self.string(reader, "pen name")?;
        let size = reader.read_f32("pen size")?;
        let color = reader.read_u32("pen color")?;
        let curvable = reader.read_u32("curvable pen")? != 0;
        let advanced_setting = self.string(reader, "advanced pen setting")?;
        let eraser_enabled = reader.read_u32("pen eraser enabled")? != 0;
        let size_level = reader.read_i32("pen size level")?;
        let particle_density = reader.read_i32("pen particle density")?;
        let particle_size = modern
            .then(|| reader.read_f32("pen particle size"))
            .transpose()?;
        let fixed_width = modern
            .then(|| reader.read_u32("pen fixed width").map(|value| value != 0))
            .transpose()?;
        let hsv = [
            reader.read_f32("pen hue")?,
            reader.read_f32("pen saturation")?,
            reader.read_f32("pen value")?,
        ];
        let color_ui_info = reader.read_u32("pen color UI info")?;
        Ok(NotePenSettings {
            name,
            size,
            color,
            curvable,
            advanced_setting,
            eraser_enabled,
            size_level,
            particle_density,
            particle_size,
            fixed_width,
            hsv,
            color_ui_info,
            extension: None,
            trailing_data: Vec::new(),
        })
    }
}
