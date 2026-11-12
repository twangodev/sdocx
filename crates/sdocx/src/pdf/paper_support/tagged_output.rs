//! Transport page-occurrence structure trees and assemble output tagging state.

use std::collections::{HashMap, HashSet};

use lopdf::{Dictionary, Document, Object, ObjectId};

use super::graph_import::GraphCloner;
use super::{deref, obj_dict};

/// Output structure tree state. Each output page occurrence gets independent
/// structure elements and a fresh ParentTree key, including duplicate pages.
pub(super) struct TaggedOutput {
    root_id: ObjectId,
    next_key: i64,
    reserved_keys: HashSet<i64>,
    root_kids: Vec<Object>,
    parent_entries: std::collections::BTreeMap<i64, Vec<Object>>,
    role_map: Dictionary,
}

impl TaggedOutput {
    pub(super) fn new(output: &mut Document) -> Self {
        Self {
            root_id: output.new_object_id(),
            next_key: 0,
            reserved_keys: HashSet::new(),
            root_kids: Vec::new(),
            parent_entries: Default::default(),
            role_map: Dictionary::new(),
        }
    }

    pub(super) fn page_has_tags(&self, doc: &Document, page_id: ObjectId) -> Result<bool, String> {
        let page = doc.get_dictionary(page_id).map_err(|e| e.to_string())?;
        let Some(key) = page.get(b"StructParents").ok() else {
            return Ok(false);
        };
        let key = deref(doc, key)?
            .as_i64()
            .map_err(|e| format!("page StructParents is not an integer: {e}"))?;
        let tree = struct_parent_tree(doc)?
            .ok_or("page has StructParents but catalog has no ParentTree")?;
        let values = tree
            .get(&key)
            .ok_or_else(|| format!("ParentTree has no page key {key}"))?;
        let values = deref(doc, values)?
            .as_array()
            .map_err(|e| format!("ParentTree page key {key} is not an array: {e}"))?;
        for value in values {
            if !matches!(deref(doc, value)?, Object::Null) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(super) fn allocate_key(&mut self) -> i64 {
        while self.reserved_keys.contains(&self.next_key)
            || self.parent_entries.contains_key(&self.next_key)
        {
            self.next_key += 1;
        }
        let key = self.next_key;
        self.next_key += 1;
        key
    }

    pub(super) fn reserve_page_keys(
        &mut self,
        doc: &Document,
        page_id: ObjectId,
    ) -> Result<(), String> {
        let page = doc.get_dictionary(page_id).map_err(|e| e.to_string())?;
        if let Ok(value) = page.get(b"StructParents") {
            self.reserved_keys.insert(
                deref(doc, value)?
                    .as_i64()
                    .map_err(|e| format!("StructParents is not an integer: {e}"))?,
            );
        }
        let Some(annots) = page.get(b"Annots").ok() else {
            return Ok(());
        };
        for annot in deref(doc, annots)?.as_array().map_err(|e| e.to_string())? {
            let dict = obj_dict(doc, annot)?;
            if let Ok(value) = dict.get(b"StructParent") {
                self.reserved_keys.insert(
                    deref(doc, value)?
                        .as_i64()
                        .map_err(|e| format!("annotation StructParent is not an integer: {e}"))?,
                );
            }
        }
        Ok(())
    }

    pub(super) fn append_page(
        &mut self,
        output: &mut Document,
        source: &Document,
        source_page_id: ObjectId,
        output_page_id: ObjectId,
        form_id: Option<ObjectId>,
        output_key: i64,
    ) -> Result<(), String> {
        let source_page = source
            .get_dictionary(source_page_id)
            .map_err(|e| e.to_string())?;
        let source_key = source_page
            .get(b"StructParents")
            .map_err(|e| e.to_string())?;
        let source_key = deref(source, source_key)?
            .as_i64()
            .map_err(|e| e.to_string())?;
        let parent_tree = struct_parent_tree(source)?.ok_or("tagged page has no ParentTree")?;
        let source_parent_array = deref(
            source,
            parent_tree
                .get(&source_key)
                .ok_or_else(|| format!("ParentTree has no page key {source_key}"))?,
        )?
        .as_array()
        .map_err(|e| format!("ParentTree page key {source_key} is not an array: {e}"))?
        .as_slice();
        let root_id = struct_tree_root_id(source)?.ok_or("tagged page has no StructTreeRoot")?;
        let root = source.get_dictionary(root_id).map_err(|e| e.to_string())?;
        self.merge_role_map(source, root)?;
        reject_unsupported_structure_root(root)?;
        let roots = structure_kids(source, root.get(b"K").map_err(|e| e.to_string())?)?;
        let mut state = TagCloneState {
            source_page_id,
            output_page_id,
            form_id,
            source_parent_array,
            source_key,
            output_parent_array: Vec::new(),
            seen_mcids: HashSet::new(),
            seen_elements: HashSet::new(),
            graph_objects: Default::default(),
        };
        for item in roots {
            if let Some(item) = clone_structure_item(
                source,
                output,
                item,
                self.root_id,
                None,
                None,
                &mut state,
                0,
            )? {
                self.root_kids.push(item);
            }
        }
        if state.seen_mcids.is_empty() {
            return Err("page StructParents has no matching structure-tree MCIDs".into());
        }
        for (mcid, value) in state.source_parent_array.iter().enumerate() {
            if !matches!(deref(source, value)?, Object::Null)
                && !state.seen_mcids.contains(&(mcid as i64))
            {
                return Err(format!(
                    "ParentTree key {source_key} has an unrepresented MCID {mcid}"
                ));
            }
        }
        self.parent_entries
            .insert(output_key, state.output_parent_array);
        Ok(())
    }

    fn merge_role_map(&mut self, doc: &Document, root: &Dictionary) -> Result<(), String> {
        let Some(role_map) = root.get(b"RoleMap").ok() else {
            return Ok(());
        };
        let role_map = obj_dict(doc, role_map)?;
        for (name, target) in role_map.iter() {
            let target = deref(doc, target)?
                .as_name()
                .map_err(|e| format!("RoleMap target for {:?} is not a name: {e}", name))?;
            if let Ok(existing) = self.role_map.get(name) {
                if existing.as_name().ok() != Some(target) {
                    return Err(format!(
                        "conflicting overlay RoleMap definition for {:?}",
                        String::from_utf8_lossy(name)
                    ));
                }
            } else {
                self.role_map
                    .set(name.clone(), Object::Name(target.to_vec()));
            }
        }
        Ok(())
    }

    pub(super) fn finish(self, output: &mut Document) -> Option<(ObjectId, Dictionary)> {
        if self.parent_entries.is_empty() {
            return None;
        }
        let mut nums = Vec::with_capacity(self.parent_entries.len() * 2);
        for (key, values) in self.parent_entries {
            nums.push(Object::Integer(key));
            nums.push(output.add_object(Object::Array(values)).into());
        }
        let mut parent_tree = Dictionary::new();
        parent_tree.set("Nums", nums);
        let mut root = Dictionary::new();
        root.set("Type", "StructTreeRoot");
        root.set("K", Object::Array(self.root_kids));
        root.set("ParentTree", output.add_object(parent_tree));
        root.set("ParentTreeNextKey", self.next_key);
        if !self.role_map.is_empty() {
            root.set("RoleMap", self.role_map);
        }
        Some((self.root_id, root))
    }
}

struct TagCloneState<'a> {
    source_page_id: ObjectId,
    output_page_id: ObjectId,
    form_id: Option<ObjectId>,
    source_parent_array: &'a [Object],
    source_key: i64,
    output_parent_array: Vec<Object>,
    seen_mcids: HashSet<i64>,
    seen_elements: HashSet<ObjectId>,
    graph_objects: std::collections::HashMap<ObjectId, ObjectId>,
}

fn struct_tree_root_id(doc: &Document) -> Result<Option<ObjectId>, String> {
    let catalog_id = doc
        .trailer
        .get(b"Root")
        .map_err(|e| e.to_string())?
        .as_reference()
        .map_err(|e| e.to_string())?;
    let catalog = doc.get_dictionary(catalog_id).map_err(|e| e.to_string())?;
    let Some(root) = catalog.get(b"StructTreeRoot").ok() else {
        return Ok(None);
    };
    match root {
        Object::Reference(id) => Ok(Some(*id)),
        _ => Err("StructTreeRoot must be indirect".into()),
    }
}

pub(super) fn struct_parent_tree(
    doc: &Document,
) -> Result<Option<std::collections::BTreeMap<i64, Object>>, String> {
    let Some(root_id) = struct_tree_root_id(doc)? else {
        return Ok(None);
    };
    let root = doc.get_dictionary(root_id).map_err(|e| e.to_string())?;
    let Some(tree) = root.get(b"ParentTree").ok() else {
        return Ok(None);
    };
    fn visit(
        doc: &Document,
        node: &Object,
        output: &mut std::collections::BTreeMap<i64, Object>,
        seen: &mut HashSet<ObjectId>,
        depth: usize,
    ) -> Result<(), String> {
        if depth > 64 {
            return Err("ParentTree nesting exceeds 64".into());
        }
        if let Object::Reference(id) = node
            && !seen.insert(*id)
        {
            return Err("cycle or repeated node in ParentTree".into());
        }
        let node = deref(doc, node)?;
        let dict = node.as_dict().map_err(|e| e.to_string())?;
        let nums = dict.get(b"Nums").ok();
        let kids = dict.get(b"Kids").ok();
        if nums.is_some() == kids.is_some() {
            return Err("ParentTree node must have exactly one of Nums or Kids".into());
        }
        if let Some(nums) = nums {
            let nums = deref(doc, nums)?.as_array().map_err(|e| e.to_string())?;
            if nums.len() % 2 != 0 {
                return Err("ParentTree Nums has an odd item count".into());
            }
            for pair in nums.as_chunks::<2>().0 {
                let key = deref(doc, &pair[0])?.as_i64().map_err(|e| e.to_string())?;
                if output.insert(key, pair[1].clone()).is_some() {
                    return Err(format!("duplicate ParentTree key {key}"));
                }
                if output.len() > 1_000_000 {
                    return Err("ParentTree exceeds 1,000,000 entries".into());
                }
            }
        } else if let Some(kids) = kids {
            for child in deref(doc, kids)?.as_array().map_err(|e| e.to_string())? {
                visit(doc, child, output, seen, depth + 1)?;
            }
        }
        Ok(())
    }
    let mut output = std::collections::BTreeMap::new();
    visit(doc, tree, &mut output, &mut HashSet::new(), 0)?;
    Ok(Some(output))
}

pub(super) fn reject_unsupported_structure_root(root: &Dictionary) -> Result<(), String> {
    for (key, _) in root.iter() {
        match key.as_slice() {
            b"Type" | b"K" | b"ParentTree" | b"ParentTreeNextKey" | b"RoleMap" => {}
            b"ClassMap" | b"Namespaces" | b"IDTree" => {
                return Err(format!(
                    "overlay StructTreeRoot {:?} is not yet supported",
                    String::from_utf8_lossy(key)
                ));
            }
            _ => {
                return Err(format!(
                    "overlay StructTreeRoot key {:?} is not yet supported",
                    String::from_utf8_lossy(key)
                ));
            }
        }
    }
    Ok(())
}

fn structure_kids(doc: &Document, object: &Object) -> Result<Vec<Object>, String> {
    if let Object::Array(items) = object {
        return Ok(items.clone());
    }
    if matches!(object, Object::Null) {
        return Ok(Vec::new());
    }
    if let Object::Reference(_) = object
        && let Object::Array(items) = deref(doc, object)?
    {
        return Ok(items.clone());
    }
    Ok(vec![object.clone()])
}

fn check_page_parent_slot(
    doc: &Document,
    state: &TagCloneState<'_>,
    mcid: i64,
    source_element: Option<ObjectId>,
) -> Result<(), String> {
    let index = usize::try_from(mcid).map_err(|_| "negative MCID")?;
    let parent = state.source_parent_array.get(index).ok_or_else(|| {
        format!(
            "ParentTree key {} has no slot for MCID {mcid}",
            state.source_key
        )
    })?;
    if matches!(deref(doc, parent)?, Object::Null) {
        return Err(format!("ParentTree slot for MCID {mcid} is null"));
    }
    if let Some(expected) = source_element {
        let actual = parent
            .as_reference()
            .map_err(|e| format!("ParentTree MCID {mcid} parent is not indirect: {e}"))?;
        if actual != expected {
            return Err(format!(
                "ParentTree MCID {mcid} points to a different StructElem"
            ));
        }
    }
    Ok(())
}

fn page_matches(
    doc: &Document,
    explicit: Option<&Object>,
    inherited_page: Option<ObjectId>,
    state: &TagCloneState<'_>,
    mcid: i64,
    element_id: Option<ObjectId>,
) -> Result<bool, String> {
    let page = if let Some(page) = explicit {
        Some(
            page.as_reference()
                .map_err(|e| format!("structure Pg is not indirect: {e}"))?,
        )
    } else {
        inherited_page
    };
    if let Some(page) = page {
        if page != state.source_page_id {
            return Ok(false);
        }
    } else if element_id.is_none() {
        return Err("MCID without inherited Pg has no indirect owning StructElem".into());
    }
    check_page_parent_slot(doc, state, mcid, element_id)?;
    Ok(true)
}

// These walkers carry the document pair, structural parent/page context, clone state,
// and recursion depth explicitly so each recursive edge remains auditable.
#[allow(clippy::too_many_arguments)]
fn clone_structure_item(
    source: &Document,
    target: &mut Document,
    item: Object,
    parent_target_id: ObjectId,
    source_parent_element: Option<ObjectId>,
    inherited_page: Option<ObjectId>,
    state: &mut TagCloneState<'_>,
    depth: usize,
) -> Result<Option<Object>, String> {
    if depth > 128 {
        return Err("StructElem nesting exceeds 128".into());
    }
    match item {
        Object::Integer(mcid) => {
            if !page_matches(
                source,
                None,
                inherited_page,
                state,
                mcid,
                source_parent_element,
            )? {
                return Ok(None);
            }
            if !state.seen_mcids.insert(mcid) {
                return Err(format!("duplicate MCID {mcid} in selected overlay page"));
            }
            let entry = parent_slot_mut(&mut state.output_parent_array, mcid)?;
            *entry = Object::Reference(parent_target_id);
            if let Some(form_id) = state.form_id {
                let mut mcr = Dictionary::new();
                mcr.set("Type", "MCR");
                mcr.set("Pg", state.output_page_id);
                mcr.set("Stm", form_id);
                mcr.set("MCID", mcid);
                Ok(Some(Object::Dictionary(mcr)))
            } else {
                Ok(Some(Object::Integer(mcid)))
            }
        }
        Object::Array(items) => {
            let mut copied = Vec::new();
            for child in items {
                if let Some(child) = clone_structure_item(
                    source,
                    target,
                    child,
                    parent_target_id,
                    source_parent_element,
                    inherited_page,
                    state,
                    depth + 1,
                )? {
                    copied.push(child);
                }
            }
            Ok((!copied.is_empty()).then_some(Object::Array(copied)))
        }
        Object::Reference(id) => {
            if !state.seen_elements.insert(id) {
                return Err("cycle or repeated StructElem reference in overlay K tree".into());
            }
            let object = source.get_object(id).map_err(|e| e.to_string())?.clone();
            clone_structure_dictionary(
                source,
                target,
                object,
                Some(id),
                parent_target_id,
                source_parent_element,
                inherited_page,
                state,
                depth + 1,
            )
        }
        Object::Dictionary(dict) => clone_structure_dictionary(
            source,
            target,
            Object::Dictionary(dict),
            None,
            parent_target_id,
            source_parent_element,
            inherited_page,
            state,
            depth + 1,
        ),
        Object::Null => Ok(None),
        other => Err(format!("unsupported StructElem K item: {other:?}")),
    }
}

#[allow(clippy::too_many_arguments)]
fn clone_structure_dictionary(
    source: &Document,
    target: &mut Document,
    object: Object,
    source_id: Option<ObjectId>,
    parent_target_id: ObjectId,
    source_parent_element: Option<ObjectId>,
    inherited_page: Option<ObjectId>,
    state: &mut TagCloneState<'_>,
    depth: usize,
) -> Result<Option<Object>, String> {
    let dict = match object {
        Object::Dictionary(dict) => dict,
        Object::Stream(stream) => stream.dict,
        other => {
            return Err(format!(
                "StructElem K reference is not a dictionary: {other:?}"
            ));
        }
    };
    if dict
        .get(b"Type")
        .and_then(Object::as_name)
        .is_ok_and(|name| name == b"MCR")
    {
        return clone_mcr(
            source,
            dict,
            parent_target_id,
            source_parent_element,
            inherited_page,
            state,
        );
    }
    if dict
        .get(b"Type")
        .and_then(Object::as_name)
        .is_ok_and(|name| name == b"OBJR")
    {
        return Err("overlay tagged annotations (OBJR) are not yet supported".into());
    }
    if !dict.has(b"S") {
        return Err("StructElem K dictionary has neither /S nor /Type /MCR".into());
    }
    if dict.has(b"ID") || dict.has(b"Ref") {
        return Err("StructElem /ID and /Ref remapping is not yet supported".into());
    }
    let explicit_page = dict.get(b"Pg").ok();
    let page = match explicit_page {
        Some(value) => Some(
            value
                .as_reference()
                .map_err(|e| format!("StructElem Pg is not indirect: {e}"))?,
        ),
        None => inherited_page,
    };
    if page.is_some_and(|page| page != state.source_page_id) {
        return Ok(None);
    }

    let target_id = target.new_object_id();
    target.objects.insert(target_id, Object::Null);
    let mut new_dict = Dictionary::new();
    let mut any_content = false;
    for (key, value) in dict.iter() {
        match key.as_slice() {
            b"P" => {}
            b"Pg" => new_dict.set("Pg", state.output_page_id),
            b"K" => {
                let children = structure_kids(source, value)?;
                let mut copied = Vec::new();
                for child in children {
                    if let Some(child) = clone_structure_item(
                        source,
                        target,
                        child,
                        target_id,
                        source_id,
                        page,
                        state,
                        depth + 1,
                    )? {
                        copied.push(child);
                    }
                }
                if !copied.is_empty() {
                    any_content = true;
                    new_dict.set("K", Object::Array(copied));
                }
            }
            _ => {
                let page_remap = HashMap::from([(state.source_page_id, state.output_page_id)]);
                let mut cloner = GraphCloner::new(
                    source,
                    target,
                    Some(&page_remap),
                    &mut state.graph_objects,
                    (0, 0),
                );
                new_dict.set(key.clone(), cloner.clone_object(value, None)?);
            }
        }
    }
    if !any_content {
        target.objects.remove(&target_id);
        return Ok(None);
    }
    new_dict.set("P", Object::Reference(parent_target_id));
    target
        .objects
        .insert(target_id, Object::Dictionary(new_dict));
    Ok(Some(Object::Reference(target_id)))
}

fn clone_mcr(
    source: &Document,
    dict: Dictionary,
    parent_target_id: ObjectId,
    source_parent_element: Option<ObjectId>,
    inherited_page: Option<ObjectId>,
    state: &mut TagCloneState<'_>,
) -> Result<Option<Object>, String> {
    if dict.has(b"Stm") {
        return Err("nested tagged Form content (/MCR /Stm) is not yet supported".into());
    }
    let mcid = dict
        .get(b"MCID")
        .map_err(|e| e.to_string())?
        .as_i64()
        .map_err(|e| e.to_string())?;
    if !page_matches(
        source,
        dict.get(b"Pg").ok(),
        inherited_page,
        state,
        mcid,
        source_parent_element,
    )? {
        return Ok(None);
    }
    if !state.seen_mcids.insert(mcid) {
        return Err(format!("duplicate MCID {mcid} in selected overlay page"));
    }
    let entry = parent_slot_mut(&mut state.output_parent_array, mcid)?;
    *entry = Object::Reference(parent_target_id);
    let mut mcr = Dictionary::new();
    mcr.set("Type", "MCR");
    mcr.set("Pg", state.output_page_id);
    if let Some(form_id) = state.form_id {
        mcr.set("Stm", form_id);
    }
    mcr.set("MCID", mcid);
    Ok(Some(Object::Dictionary(mcr)))
}

fn parent_slot_mut(array: &mut Vec<Object>, mcid: i64) -> Result<&mut Object, String> {
    let index = usize::try_from(mcid).map_err(|_| "negative MCID")?;
    if index > 1_000_000 {
        return Err("MCID exceeds 1,000,000".into());
    }
    if array.len() <= index {
        array.resize(index + 1, Object::Null);
    }
    Ok(&mut array[index])
}
