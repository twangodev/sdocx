//! Import source PDF graphs with canonical and per-page annotation identities.

use std::collections::{HashMap, HashSet};

use lopdf::{Dictionary, Document, Object, ObjectId};

use super::{ParsedSource, ValidatedAssemblyPage, deref, inherited_optional};

pub(super) fn reserve_annotation_plans(
    parsed: &std::collections::BTreeMap<String, ParsedSource>,
    pages: &[ValidatedAssemblyPage<'_>],
    output_page_ids: &[ObjectId],
    page_remaps: &std::collections::BTreeMap<String, HashMap<ObjectId, ObjectId>>,
    selected_sources: &HashSet<String>,
    output: &mut Document,
) -> Result<std::collections::BTreeMap<String, AnnotationPlan>, String> {
    // Reserve annotation identities for each selected page occurrence before
    // following any cross-page links. A shared source annotation has a distinct
    // identity on each occurrence, while external links use its first occurrence.
    let mut annotation_plans = std::collections::BTreeMap::<String, AnnotationPlan>::new();
    for key in selected_sources {
        annotation_plans.entry(key.clone()).or_default();
    }
    for (page, output_id) in pages.iter().zip(output_page_ids) {
        if let Some(imported) = &page.imported {
            let source = &parsed[imported.source_key];
            let page = source
                .doc
                .get_dictionary(imported.source_page_id)
                .map_err(|e| e.to_string())?;
            if let Ok(annots) = page.get(b"Annots") {
                let mut ids = Vec::new();
                collect_annotation_ids(&source.doc, annots, &mut ids);
                let plan = annotation_plans
                    .entry(imported.source_key.to_owned())
                    .or_default();
                for id in ids {
                    if plan.occurrences.contains_key(&(*output_id, id)) {
                        continue;
                    }
                    let target = output.new_object_id();
                    plan.occurrences.insert((*output_id, id), target);
                    plan.canonical.entry(id).or_insert(target);
                    plan.pages.insert(target, *output_id);
                }
            }
        }
    }

    for (key, plan) in &mut annotation_plans {
        plan.sensitive = annotation_sensitive_objects(&parsed[key].doc, &plan.canonical);
        let source = &parsed[key];
        for page_id in source.pages.values() {
            if page_remaps[key].contains_key(page_id) {
                continue;
            }
            if let Ok(page) = source.doc.get_dictionary(*page_id)
                && let Ok(annots) = page.get(b"Annots")
            {
                let mut ids = Vec::new();
                collect_annotation_ids(&source.doc, annots, &mut ids);
                plan.unselected.extend(ids);
            }
        }
    }
    Ok(annotation_plans)
}

fn is_annotation_dictionary(doc: &Document, dict: &Dictionary) -> bool {
    match dict.get(b"Type").ok().map(|kind| deref(doc, kind)) {
        None | Some(Ok(Object::Null)) => dict.has(b"Subtype") && dict.has(b"Rect"),
        Some(Ok(Object::Name(kind))) => kind == b"Annot",
        _ => false,
    }
}

#[derive(Default)]
pub(super) struct AnnotationPlan {
    canonical: HashMap<ObjectId, ObjectId>,
    occurrences: HashMap<(ObjectId, ObjectId), ObjectId>,
    pages: HashMap<ObjectId, ObjectId>,
    initialized: HashSet<ObjectId>,
    sensitive: HashSet<ObjectId>,
    unselected: HashSet<ObjectId>,
    local_objects: HashMap<(Option<ObjectId>, ObjectId), ObjectId>,
}

fn collect_annotation_ids(doc: &Document, object: &Object, ids: &mut Vec<ObjectId>) {
    let mut seen = HashSet::new();
    let mut pending = vec![object];
    while let Some(object) = pending.pop() {
        match object {
            Object::Array(items) => pending.extend(items.iter().rev()),
            Object::Reference(id) if seen.insert(*id) => match doc.objects.get(id) {
                Some(Object::Array(items)) => pending.extend(items.iter().rev()),
                Some(Object::Dictionary(dict)) if is_annotation_dictionary(doc, dict) => {
                    ids.push(*id)
                }
                Some(next @ Object::Reference(_)) => pending.push(next),
                _ => {}
            },
            _ => {}
        }
    }
}

// Compute the reverse reference closure once per source. Page objects are
// boundaries: following their parent and resources would make unrelated shared
// objects occurrence-specific merely because they point at a page.
fn annotation_sensitive_objects(
    doc: &Document,
    annotations: &HashMap<ObjectId, ObjectId>,
) -> HashSet<ObjectId> {
    let pages: HashSet<_> = doc.get_pages().into_values().collect();
    let mut reverse: HashMap<ObjectId, Vec<ObjectId>> = HashMap::new();
    for (id, object) in &doc.objects {
        if pages.contains(id) {
            continue;
        }
        let mut stack = vec![object];
        while let Some(value) = stack.pop() {
            match value {
                Object::Reference(target) if !pages.contains(target) => {
                    reverse.entry(*target).or_default().push(*id)
                }
                Object::Array(items) => stack.extend(items),
                Object::Dictionary(dict) => stack.extend(dict.iter().map(|(_, value)| value)),
                Object::Stream(stream) => stack.extend(stream.dict.iter().map(|(_, value)| value)),
                _ => {}
            }
        }
    }
    let mut sensitive: HashSet<_> = annotations.keys().copied().collect();
    let mut pending: Vec<_> = sensitive.iter().copied().collect();
    while let Some(id) = pending.pop() {
        if let Some(parents) = reverse.get(&id) {
            for parent in parents {
                if sensitive.insert(*parent) {
                    pending.push(*parent);
                }
            }
        }
    }
    sensitive
}

pub(super) struct GraphCloner<'a> {
    source: &'a Document,
    target: &'a mut Document,
    page_remap: Option<&'a std::collections::HashMap<ObjectId, ObjectId>>,
    pages_root_id: ObjectId,
    objects: &'a mut std::collections::HashMap<ObjectId, ObjectId>,
    annotation_objects: std::collections::HashMap<(Option<ObjectId>, ObjectId), ObjectId>,
    pub(super) plan: Option<&'a mut AnnotationPlan>,
    source_pages: HashSet<ObjectId>,
    cloned_count: usize,
}

impl<'a> GraphCloner<'a> {
    pub(super) fn new(
        source: &'a Document,
        target: &'a mut Document,
        page_remap: Option<&'a std::collections::HashMap<ObjectId, ObjectId>>,
        objects: &'a mut std::collections::HashMap<ObjectId, ObjectId>,
        pages_root_id: ObjectId,
    ) -> Self {
        let source_pages = source.get_pages().into_values().collect();
        Self {
            source,
            target,
            page_remap,
            pages_root_id,
            objects,
            annotation_objects: Default::default(),
            plan: None,
            source_pages,
            cloned_count: 0,
        }
    }

    pub(super) fn clone_page(
        &mut self,
        page: &Dictionary,
        target_page: ObjectId,
    ) -> Result<Dictionary, String> {
        let mut result = Dictionary::new();
        for (key, value) in page.iter() {
            if key == b"Parent" {
                continue;
            }
            if key == b"Annots" {
                let annotations = self.clone_annotation_list(value, target_page, 0)?;
                result.set(key.clone(), annotations);
            } else {
                result.set(key.clone(), self.clone_object(value, Some(target_page))?);
            }
        }
        for key in [b"MediaBox" as &[u8], b"CropBox", b"Rotate", b"Resources"] {
            if result.has(key) {
                continue;
            }
            if let Some(value) = inherited_optional(self.source, page, key)? {
                result.set(key, self.clone_object(value, Some(target_page))?);
            }
        }
        if !result.has(b"MediaBox") {
            return Err("selected page has no inherited MediaBox".into());
        }
        result.set("Parent", Object::Reference(self.pages_root_id));
        Ok(result)
    }

    pub(super) fn clone_object(
        &mut self,
        object: &Object,
        current_page: Option<ObjectId>,
    ) -> Result<Object, String> {
        self.clone_object_at(object, current_page, 0)
    }

    fn clone_annotation_list(
        &mut self,
        object: &Object,
        current_page: ObjectId,
        depth: usize,
    ) -> Result<Object, String> {
        if depth > 128 {
            return Err("annotation array nesting exceeds 128".into());
        }
        match object {
            Object::Array(items) => Ok(Object::Array(
                items
                    .iter()
                    .map(|item| self.clone_annotation_list(item, current_page, depth + 1))
                    .collect::<Result<_, _>>()?,
            )),
            Object::Reference(id) => {
                if let Some(mapped) = self.annotation_objects.get(&(Some(current_page), *id)) {
                    return Ok(Object::Reference(*mapped));
                }
                let source = self.source;
                let source_object = source
                    .objects
                    .get(id)
                    .ok_or_else(|| format!("missing PDF object {id:?}"))?;
                if matches!(source_object, Object::Array(_)) {
                    let target_id = self.target.new_object_id();
                    self.annotation_objects
                        .insert((Some(current_page), *id), target_id);
                    self.target.objects.insert(target_id, Object::Null);
                    let cloned =
                        self.clone_annotation_list(source_object, current_page, depth + 1)?;
                    self.target.objects.insert(target_id, cloned);
                    Ok(Object::Reference(target_id))
                } else {
                    self.clone_object_at(object, Some(current_page), depth + 1)
                }
            }
            _ => self.clone_object_at(object, Some(current_page), depth + 1),
        }
    }

    fn clone_object_at(
        &mut self,
        object: &Object,
        current_page: Option<ObjectId>,
        depth: usize,
    ) -> Result<Object, String> {
        if depth > 128 {
            return Err("PDF object graph nesting exceeds 128".into());
        }
        match object {
            Object::Reference(id) => {
                if let Some(mapped) = self.page_remap.and_then(|m| m.get(id)) {
                    return Ok(Object::Reference(*mapped));
                }
                if self.source_pages.contains(id) {
                    return Err("reachable source graph references an unselected page".into());
                }
                let is_annotation = self
                    .source
                    .objects
                    .get(id)
                    .and_then(|o| o.as_dict().ok())
                    .is_some_and(|dict| is_annotation_dictionary(self.source, dict));
                if is_annotation && let Some(plan) = &mut self.plan {
                    let mapped = current_page
                        .and_then(|page| plan.occurrences.get(&(page, *id)).copied())
                        .or_else(|| plan.canonical.get(id).copied());
                    if let Some(target_id) = mapped {
                        if plan.initialized.insert(target_id) {
                            let annotation_page = plan.pages[&target_id];
                            self.target.objects.insert(target_id, Object::Null);
                            let source = self.source;
                            let source_object =
                                source.get_object(*id).map_err(|e| e.to_string())?;
                            let copied = self.clone_object_at(
                                source_object,
                                Some(annotation_page),
                                depth + 1,
                            )?;
                            self.target.objects.insert(target_id, copied);
                        }
                        return Ok(Object::Reference(target_id));
                    }
                }
                if is_annotation
                    && self
                        .plan
                        .as_ref()
                        .is_some_and(|plan| plan.unselected.contains(id))
                {
                    return Err("annotation references an unselected page".into());
                }
                if is_annotation
                    && let Ok(dict) = self.source.get_dictionary(*id)
                    && let Ok(Object::Reference(owner)) = dict.get(b"P")
                    && self.source_pages.contains(owner)
                    && self.page_remap.and_then(|map| map.get(owner)).is_none()
                {
                    return Err("annotation references an unselected page".into());
                }
                let page_sensitive = !is_annotation
                    && self
                        .plan
                        .as_ref()
                        .is_some_and(|plan| plan.sensitive.contains(id));
                if page_sensitive
                    && let Some(mapped) = self
                        .plan
                        .as_ref()
                        .and_then(|plan| plan.local_objects.get(&(current_page, *id)))
                {
                    return Ok(Object::Reference(*mapped));
                }
                if is_annotation
                    && let Some(mapped) = self.annotation_objects.get(&(current_page, *id))
                {
                    return Ok(Object::Reference(*mapped));
                }
                if !is_annotation
                    && !page_sensitive
                    && let Some(mapped) = self.objects.get(id)
                {
                    return Ok(Object::Reference(*mapped));
                }
                self.cloned_count += 1;
                if self.cloned_count > 1_000_000 {
                    return Err("reachable PDF object graph exceeds 1,000,000 objects".into());
                }
                let target_id = self.target.new_object_id();
                if is_annotation {
                    self.annotation_objects
                        .insert((current_page, *id), target_id);
                } else if page_sensitive {
                    self.plan
                        .as_mut()
                        .expect("page-sensitive objects require a plan")
                        .local_objects
                        .insert((current_page, *id), target_id);
                } else {
                    self.objects.insert(*id, target_id);
                }
                self.target.objects.insert(target_id, Object::Null);
                let source = self.source;
                let source_object = source
                    .objects
                    .get(id)
                    .ok_or_else(|| format!("missing PDF object {id:?}"))?;
                let copied = self.clone_object_at(source_object, current_page, depth + 1)?;
                self.target.objects.insert(target_id, copied);
                Ok(Object::Reference(target_id))
            }
            Object::Dictionary(dict) => {
                let mut copied = Dictionary::new();
                let is_annotation = is_annotation_dictionary(self.source, dict);
                for (key, value) in dict.iter() {
                    if key == b"P" && is_annotation && current_page.is_some() {
                        continue;
                    }
                    copied.set(
                        key.clone(),
                        self.clone_object_at(value, current_page, depth + 1)?,
                    );
                }
                if is_annotation && let Some(page) = current_page {
                    copied.set("P", Object::Reference(page));
                }
                Ok(Object::Dictionary(copied))
            }
            Object::Stream(stream) => {
                let mut copied = stream.clone();
                let mut dict = Dictionary::new();
                for (key, value) in stream.dict.iter() {
                    dict.set(
                        key.clone(),
                        self.clone_object_at(value, current_page, depth + 1)?,
                    );
                }
                copied.dict = dict;
                Ok(Object::Stream(copied))
            }
            Object::Array(items) => Ok(Object::Array(
                items
                    .iter()
                    .map(|item| self.clone_object_at(item, current_page, depth + 1))
                    .collect::<Result<_, _>>()?,
            )),
            other => Ok(other.clone()),
        }
    }
}
