//! PDF geometry is read only in the isolated worker. Coordinates are PDF bp,
//! bottom-left origin; generated overlays convert them to the host's coordinates.
use lopdf::{Document, Object, ObjectId};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, io};

pub(crate) mod overlay;
#[cfg(test)]
mod tests;
pub(crate) mod worker;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Geometry {
    pub(crate) size: [f64; 2],
    pub(crate) anchors: BTreeMap<String, [f64; 2]>,
    pub(crate) links: Vec<Link>,
}
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Link {
    pub(crate) rect: [f64; 4],
    pub(crate) target: Target,
}
#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum Target {
    Point([f64; 2]),
    Uri(String),
}

fn invalid(message: &str) -> io::Error {
    io::Error::other(message)
}
fn deref<'a>(doc: &'a Document, value: &'a Object) -> io::Result<&'a Object> {
    doc.dereference(value)
        .map(|(_, obj)| obj)
        .map_err(io::Error::other)
}
fn array<'a>(doc: &'a Document, value: &'a Object) -> io::Result<&'a Vec<Object>> {
    deref(doc, value)?.as_array().map_err(io::Error::other)
}
fn number(value: &Object) -> io::Result<f64> {
    let n = value.as_float().map_err(io::Error::other)? as f64;
    if !n.is_finite() {
        return Err(invalid("nonfinite PDF coordinate"));
    }
    Ok(n)
}
fn inherited<'a>(doc: &'a Document, mut page: ObjectId, key: &[u8]) -> io::Result<&'a Object> {
    for _ in 0..16 {
        let dict = doc.get_dictionary(page).map_err(io::Error::other)?;
        if let Ok(value) = dict.get(key) {
            return deref(doc, value);
        }
        page = dict
            .get(b"Parent")
            .and_then(Object::as_reference)
            .map_err(io::Error::other)?;
    }
    Err(invalid("page tree depth exceeded"))
}
fn point(doc: &Document, value: &Object, page: ObjectId) -> io::Result<[f64; 2]> {
    let a = array(doc, value)?;
    if a.len() < 5
        || a[0].as_reference().ok() != Some(page)
        || a[1].as_name().ok() != Some(b"XYZ".as_slice())
    {
        return Err(invalid("only local single-page XYZ destinations supported"));
    }
    Ok([number(&a[2])?, number(&a[3])?])
}
fn names(
    doc: &Document,
    obj: &Object,
    out: &mut BTreeMap<Vec<u8>, Object>,
    depth: usize,
) -> io::Result<()> {
    if depth > 16 || out.len() > 4096 {
        return Err(invalid("destination tree limit"));
    }
    let dict = deref(doc, obj)?.as_dict().map_err(io::Error::other)?;
    if let Ok(entries) = dict.get(b"Names") {
        let entries = array(doc, entries)?;
        if entries.len() % 2 != 0 {
            return Err(invalid("invalid destination tree"));
        }
        for pair in entries.chunks_exact(2) {
            let value = deref(doc, &pair[1])?;
            let value = match value {
                Object::Dictionary(d) => d.get(b"D").map_err(io::Error::other)?,
                _ => value,
            };
            out.insert(
                pair[0].as_str().map_err(io::Error::other)?.to_vec(),
                value.clone(),
            );
        }
    }
    if let Ok(kids) = dict.get(b"Kids") {
        for child in array(doc, kids)? {
            names(doc, child, out, depth + 1)?;
        }
    }
    Ok(())
}
fn destinations(doc: &Document) -> io::Result<BTreeMap<Vec<u8>, Object>> {
    let mut out = BTreeMap::new();
    let catalog = doc.catalog().map_err(io::Error::other)?;
    if let Ok(obj) = catalog.get(b"Names") {
        let dict = deref(doc, obj)?.as_dict().map_err(io::Error::other)?;
        if let Ok(tree) = dict.get(b"Dests") {
            names(doc, tree, &mut out, 0)?;
        }
    }
    Ok(out)
}
fn target(
    doc: &Document,
    value: &Object,
    names: &BTreeMap<Vec<u8>, Object>,
    page: ObjectId,
) -> io::Result<Target> {
    let value = deref(doc, value)?;
    let value = match value {
        Object::Name(name) | Object::String(name, _) => names
            .get(name)
            .ok_or_else(|| invalid("missing named destination"))?,
        _ => value,
    };
    Ok(Target::Point(point(doc, value, page)?))
}
fn link(
    doc: &Document,
    dict: &lopdf::Dictionary,
    names: &BTreeMap<Vec<u8>, Object>,
    page: ObjectId,
) -> io::Result<Link> {
    if dict.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Link".as_slice()) {
        return Err(invalid(
            "unsupported PDF annotation; cannot silently discard",
        ));
    }
    let r = array(doc, dict.get(b"Rect").map_err(io::Error::other)?)?;
    if r.len() != 4 {
        return Err(invalid("invalid annotation rectangle"));
    }
    let rect = [
        number(&r[0])?,
        number(&r[1])?,
        number(&r[2])?,
        number(&r[3])?,
    ];
    let target = if let Ok(dest) = dict.get(b"Dest") {
        target(doc, dest, names, page)?
    } else {
        let action = deref(doc, dict.get(b"A").map_err(io::Error::other)?)?
            .as_dict()
            .map_err(io::Error::other)?;
        match action
            .get(b"S")
            .and_then(Object::as_name)
            .map_err(io::Error::other)?
        {
            b"GoTo" => target(
                doc,
                action.get(b"D").map_err(io::Error::other)?,
                names,
                page,
            )?,
            b"URI" => {
                let uri = String::from_utf8(
                    action
                        .get(b"URI")
                        .and_then(Object::as_str)
                        .map_err(io::Error::other)?
                        .to_vec(),
                )
                .map_err(io::Error::other)?;
                if !["https://", "http://", "scholium-ref:"]
                    .iter()
                    .any(|p| uri.starts_with(p))
                {
                    return Err(invalid("unsupported PDF URI scheme"));
                }
                Target::Uri(uri)
            }
            _ => return Err(invalid("unsupported PDF action")),
        }
    };
    Ok(Link { rect, target })
}
pub(crate) fn inspect(bytes: &[u8]) -> io::Result<Geometry> {
    let doc = Document::load_mem(bytes).map_err(io::Error::other)?;
    let pages = doc.get_pages();
    if pages.len() != 1 {
        return Err(invalid("vector requires one PDF page"));
    }
    let page = *pages.values().next().ok_or_else(|| invalid("empty PDF"))?;
    let dict = doc.get_dictionary(page).map_err(io::Error::other)?;
    if inherited(&doc, page, b"Rotate").is_ok_and(|v| v.as_i64().ok() != Some(0))
        || dict.has(b"UserUnit")
    {
        return Err(invalid("rotated or scaled PDF pages unsupported"));
    }
    let bounds = array(&doc, inherited(&doc, page, b"MediaBox")?)?;
    if bounds.len() != 4
        || number(&bounds[0])? != 0.0
        || number(&bounds[1])? != 0.0
        || inherited(&doc, page, b"CropBox").is_ok()
    {
        return Err(invalid("nonzero origin or cropped PDF unsupported"));
    }
    let size = [number(&bounds[2])?, number(&bounds[3])?];
    if size.iter().any(|v| *v <= 0.0 || *v > 20000.0) {
        return Err(invalid("invalid PDF size"));
    }
    let names = destinations(&doc)?;
    let mut anchors = BTreeMap::new();
    for (name, value) in &names {
        if let Some(label) = name.strip_prefix(b"sch:") {
            anchors.insert(
                String::from_utf8(label.to_vec()).map_err(io::Error::other)?,
                point(&doc, value, page)?,
            );
        }
    }
    let links = doc
        .get_page_annotations(page)
        .map_err(io::Error::other)?
        .into_iter()
        .map(|d| link(&doc, d, &names, page))
        .collect::<io::Result<Vec<_>>>()?;
    Ok(Geometry {
        size,
        anchors,
        links,
    })
}
