//! Validate before cloning JSON into retained scenes or entering raster loops.
use super::invalid;
use crate::LatexError;
use serde_json::Value;
const MAX_ROWS: u64 = 20_000;
const MAX_ITEMS: usize = 100_000;
const MAX_COORDINATE: f64 = 1_000_000_000_000.0;

pub(crate) fn message(value: &Value) -> Result<(), LatexError> {
    if value["pages_total"].as_u64().is_some_and(|n| n > 256) {
        return Err(invalid("page count limit exceeded"));
    }
    for page in array(&value["pages_changed"]) {
        page_number(&page["page"])?;
        display_list(&page["dl"])?;
    }
    display_list(&value["dl"])?;
    let mut fragment_count = 0;
    for fragments in std::iter::once(&value["fragments"])
        .chain(array(&value["placements"]).map(|p| &p["fragments"]))
    {
        for fragment in array(fragments) {
            fragment_count += 1;
            if fragment_count > MAX_ROWS {
                return Err(invalid("fragment count limit exceeded"));
            }
            page_number(&fragment["page"])?;
            let first = fragment["first_line"].as_u64().unwrap_or(0);
            let last = fragment["last_line"].as_u64().unwrap_or(u64::MAX);
            if first == 0
                || first > last
                || last > MAX_ROWS
                || array(&fragment["xs"]).count() as u64 != last - first + 1
                || array(&fragment["baselines"]).count() as u64 != last - first + 1
            {
                return Err(invalid("fragment range limit exceeded"));
            }
        }
    }
    if array(&value["pages_changed"]).count() > 256
        || array(&value["placements"]).count() > MAX_ROWS as usize
    {
        return Err(invalid("scene count limit exceeded"));
    }
    coordinates(value)
}
fn array(value: &Value) -> impl Iterator<Item = &Value> + Clone {
    value.as_array().into_iter().flatten()
}
fn page_number(value: &Value) -> Result<(), LatexError> {
    if !value.as_u64().is_some_and(|n| (1..=256).contains(&n)) {
        return Err(invalid("page number limit exceeded"));
    }
    Ok(())
}
fn display_list(value: &Value) -> Result<(), LatexError> {
    let rows = array(&value["lines"]);
    let count = rows.clone().count();
    let fonts = value["fonts"].as_object().map_or(0, |m| m.len());
    let items =
        rows.map(|r| array(&r["items"]).count()).sum::<usize>() + array(&value["other"]).count();
    if count > MAX_ROWS as usize || fonts > 128 || items > MAX_ITEMS {
        return Err(invalid("display list count limit exceeded"));
    }
    Ok(())
}
fn coordinates(value: &Value) -> Result<(), LatexError> {
    // Depth is capped by serde_json. Total nodes are capped independently of encoded byte length.
    let mut pending = vec![(value, false)];
    let mut visited = 0;
    while let Some((value, coordinate)) = pending.pop() {
        visited += 1;
        if visited > MAX_ITEMS * 12 {
            return Err(invalid("JSON node limit exceeded"));
        }
        match value {
            Value::Array(a) => pending.extend(a.iter().map(|v| (v, coordinate))),
            Value::Object(m) => {
                for (key, v) in m {
                    let coordinate = matches!(
                        key.as_str(),
                        "x" | "y"
                            | "w"
                            | "h"
                            | "d"
                            | "width"
                            | "height"
                            | "page_width"
                            | "page_height"
                            | "size"
                            | "slant"
                            | "extend"
                            | "xs"
                            | "baselines"
                    );
                    pending.push((v, coordinate));
                }
            }
            Value::Number(n)
                if coordinate
                    && n.as_f64()
                        .is_some_and(|n| !n.is_finite() || n.abs() > MAX_COORDINATE) =>
            {
                return Err(invalid("numeric coordinate limit exceeded"));
            }
            _ => {}
        }
    }
    Ok(())
}
