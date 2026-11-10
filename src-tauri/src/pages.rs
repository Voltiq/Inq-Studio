//! Page organisation: rotate, delete, reorder, duplicate, insert blank pages,
//! insert pages from other PDFs, and extract pages to a new file.
//!
//! Reordering flattens the page tree: every page becomes a direct child of
//! the root, after copying the attributes it used to inherit (resources,
//! boxes, rotation) onto the page itself, so nothing changes visually.

use crate::util::*;
use lopdf::{dictionary, Document, Object, ObjectId};

const INHERITABLE: [&[u8]; 4] = [b"Resources", b"MediaBox", b"CropBox", b"Rotate"];

pub fn page_ids(doc: &Document) -> Vec<ObjectId> {
    doc.get_pages().into_values().collect()
}

pub fn page_id(doc: &Document, index: usize) -> Result<ObjectId, String> {
    page_ids(doc).get(index).copied().ok_or_else(|| format!("Page {} does not exist.", index + 1))
}

fn root_pages(doc: &Document) -> Result<ObjectId, String> {
    doc.catalog().map_err(err)?.get(b"Pages").and_then(Object::as_reference).map_err(err)
}

/// Copies inherited attributes onto the page.
fn materialize(doc: &mut Document, id: ObjectId) {
    for key in INHERITABLE {
        let has = doc.get_dictionary(id).map(|d| d.has(key)).unwrap_or(true);
        if has {
            continue;
        }
        if let Some(v) = inherited(doc, id, key).cloned() {
            if let Ok(d) = doc.get_dictionary_mut(id) {
                d.set(key.to_vec(), v);
            }
        }
    }
}

/// Makes `ids` the document's pages, in that order.
pub fn set_order(doc: &mut Document, ids: &[ObjectId]) -> Result<(), String> {
    let root = root_pages(doc)?;
    for id in ids {
        materialize(doc, *id);
    }
    for id in ids {
        doc.get_dictionary_mut(*id).map_err(err)?.set("Parent", Object::Reference(root));
    }
    let root_dict = doc.get_dictionary_mut(root).map_err(err)?;
    root_dict.set("Kids", ids.iter().map(|id| Object::Reference(*id)).collect::<Vec<_>>());
    root_dict.set("Count", ids.len() as i64);
    Ok(())
}

pub fn rotate(doc: &mut Document, indices: &[usize], delta: i64) -> Result<(), String> {
    let ids = page_ids(doc);
    for i in indices {
        let id = *ids.get(*i).ok_or("No such page")?;
        let cur = inherited(doc, id, b"Rotate").and_then(num).unwrap_or(0.0) as i64;
        doc.get_dictionary_mut(id).map_err(err)?.set("Rotate", (cur + delta).rem_euclid(360));
    }
    Ok(())
}

pub fn delete(doc: &mut Document, indices: &[usize]) -> Result<(), String> {
    let ids = page_ids(doc);
    let keep: Vec<ObjectId> = ids.iter().enumerate().filter(|(i, _)| !indices.contains(i)).map(|(_, id)| *id).collect();
    if keep.is_empty() {
        return Err("A PDF needs at least one page.".into());
    }
    set_order(doc, &keep)?;
    for (i, id) in ids.iter().enumerate() {
        if indices.contains(&i) {
            doc.objects.remove(id);
        }
    }
    Ok(())
}

/// `order` lists the current page indices in their new order.
pub fn reorder(doc: &mut Document, order: &[usize]) -> Result<(), String> {
    let ids = page_ids(doc);
    let mut seen = vec![false; ids.len()];
    if order.len() != ids.len() || order.iter().any(|i| *i >= ids.len() || std::mem::replace(&mut seen[*i], true)) {
        return Err("The new page order is not a rearrangement of the pages.".into());
    }
    let new: Vec<ObjectId> = order.iter().map(|i| ids[*i]).collect();
    set_order(doc, &new)
}

/// Inserts an empty page at `at` (0 = before the first), sized like its neighbour.
pub fn insert_blank(doc: &mut Document, at: usize) -> Result<(), String> {
    let mut ids = page_ids(doc);
    let neighbour = ids.get(at.saturating_sub(1)).or(ids.first()).copied();
    let media = neighbour
        .and_then(|id| inherited(doc, id, b"MediaBox").cloned())
        .unwrap_or_else(|| Object::Array(vec![0.into(), 0.into(), 595.into(), 842.into()]));
    let root = root_pages(doc)?;
    let page = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => root,
        "MediaBox" => media,
        "Resources" => dictionary! {},
    });
    ids.insert(at.min(ids.len()), page);
    set_order(doc, &ids)
}

pub fn duplicate(doc: &mut Document, index: usize) -> Result<(), String> {
    let mut ids = page_ids(doc);
    let src = *ids.get(index).ok_or("No such page")?;
    materialize(doc, src);
    let mut copy = doc.get_dictionary(src).map_err(err)?.clone();
    copy.remove(b"Annots");
    let id = doc.add_object(copy);
    ids.insert(index + 1, id);
    set_order(doc, &ids)
}

/// Inserts every page of another PDF at `at`.
pub fn insert_document(doc: &mut Document, mut other: Document, at: usize) -> Result<usize, String> {
    if other.is_encrypted() {
        return Err("That PDF is password protected. Open it, save an unprotected copy, and insert that.".into());
    }
    other.renumber_objects_with(doc.max_id + 1);
    let other_ids: Vec<ObjectId> = other.get_pages().into_values().collect();
    for id in &other_ids {
        materialize(&mut other, *id);
    }
    let catalog_id = other.trailer.get(b"Root").and_then(Object::as_reference).ok();
    for (id, obj) in other.objects.into_iter() {
        let skip = Some(id) == catalog_id
            || matches!(&obj, Object::Dictionary(d) if d.get(b"Type").and_then(Object::as_name).ok() == Some(b"Pages"));
        if !skip {
            doc.objects.insert(id, obj);
        }
    }
    doc.max_id = doc.objects.keys().map(|k| k.0).max().unwrap_or(doc.max_id);
    let mut ids = page_ids(doc);
    let at = at.min(ids.len());
    let count = other_ids.len();
    for (k, id) in other_ids.into_iter().enumerate() {
        ids.insert(at + k, id);
    }
    set_order(doc, &ids)?;
    Ok(count)
}

/// A new document holding only the given pages.
pub fn extract(doc: &Document, indices: &[usize]) -> Result<Document, String> {
    let mut copy = doc.clone();
    let ids = page_ids(&copy);
    let keep: Vec<ObjectId> = indices.iter().filter_map(|i| ids.get(*i).copied()).collect();
    if keep.is_empty() {
        return Err("Select at least one page to extract.".into());
    }
    set_order(&mut copy, &keep)?;
    for id in ids.iter().filter(|id| !keep.contains(id)) {
        copy.objects.remove(id);
    }
    // Outlines point at pages that may be gone.
    if let Ok(cat) = copy.catalog_mut() {
        cat.remove(b"Outlines");
        cat.remove(b"PageLabels");
    }
    copy.prune_objects();
    Ok(copy)
}
