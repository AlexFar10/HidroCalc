//! Completarea unui șablon Excel (.xlsx), ca `fillTemplate` din aplicația React:
//!
//! - `{cale}` într-o celulă se înlocuiește; o celulă care e **doar** tag-ul primește
//!   valoarea brută (numerele rămân numere, se aplică formatul celulei);
//! - un rând cu `{lista[].camp}` se multiplică pentru fiecare element din `lista`
//!   (formatarea rândului se păstrează); o listă goală elimină rândul;
//! - rândurile de dedesubt, celulele îmbinate și `dimension` se renumerotează.
//!
//! Formulele care referă rândurile mutate nu se ajustează (ca în aplicația React).

use super::document::{format_ro, resolve_path};
use serde_json::Value;
use std::collections::BTreeSet;
use std::io::{Cursor, Read, Write};

fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Textul concatenat al tuturor `<t>` dintr-un fragment.
fn all_t(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find("<t") {
        let after = &rest[i + 2..];
        if !(after.starts_with('>') || after.starts_with(' ')) {
            rest = after;
            continue;
        }
        let Some(gt) = after.find('>') else { break };
        if after[..gt].ends_with('/') {
            rest = &after[gt + 1..];
            continue;
        }
        let body = &after[gt + 1..];
        let end = body.find("</t>").unwrap_or(body.len());
        out.push_str(&xml_unescape(&body[..end]));
        rest = &body[end..];
    }
    out
}

fn shared_strings(xml: &str) -> Vec<String> {
    xml.split("<si>").skip(1).map(|si| all_t(si.split("</si>").next().unwrap_or(""))).collect()
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let i = tag.find(&key)? + key.len();
    Some(&tag[i..i + tag[i..].find('"')?])
}

fn col_row(r: &str) -> (String, u32) {
    let col: String = r.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    (col.clone(), r[col.len()..].parse().unwrap_or(0))
}

/// O celulă: atributele (fără `t` și `r`), referința și textul.
struct Cell {
    col: String,
    attrs: String,
    text: Option<String>,
    raw: String,
}

fn parse_cells(row_body: &str, sst: &[String]) -> Vec<Cell> {
    let mut cells = Vec::new();
    let mut rest = row_body;
    while let Some(i) = rest.find("<c ") {
        let after = &rest[i..];
        let open_end = after.find('>').unwrap_or(after.len() - 1);
        let open = &after[..open_end + 1];
        let (raw, consumed) = if open.ends_with("/>") {
            (open.to_string(), open.len())
        } else {
            let close = after.find("</c>").map_or(after.len(), |c| c + 4);
            (after[..close].to_string(), close)
        };
        let r = attr(open, "r").unwrap_or("").to_string();
        let t = attr(open, "t");
        let inner = &raw[open.len().min(raw.len())..];
        let v = inner.split("<v>").nth(1).and_then(|x| x.split("</v>").next()).map(xml_unescape);
        let text = match t {
            Some("s") => v.and_then(|i| i.trim().parse::<usize>().ok()).and_then(|i| sst.get(i).cloned()),
            Some("inlineStr") => Some(all_t(inner)),
            Some("str") => v,
            _ => None,
        };
        let attrs: String = open
            .trim_start_matches("<c")
            .trim_end_matches("/>")
            .trim_end_matches('>')
            .split('"')
            .collect::<Vec<_>>()
            .chunks(2)
            .filter_map(|ch| {
                let key = ch[0].trim().trim_end_matches('=');
                (ch.len() == 2 && !key.is_empty() && key != "r" && key != "t").then(|| format!(" {key}=\"{}\"", ch[1]))
            })
            .collect();
        cells.push(Cell { col: col_row(&r).0, attrs, text, raw });
        rest = &after[consumed..];
    }
    cells
}

/// Valoarea unui tag (fără acolade) sau `None` dacă lipsește.
type Lookup<'a> = dyn Fn(&str) -> Option<Value> + 'a;

fn fill_text(text: &str, lookup: &Lookup, missing: &mut BTreeSet<String>) -> Result<Value, String> {
    let trimmed = text.trim();
    let is_whole = trimmed.starts_with('{') && trimmed.ends_with('}') && trimmed[1..trimmed.len() - 1].find(['{', '}']).is_none();
    if is_whole {
        let path = &trimmed[1..trimmed.len() - 1];
        return Ok(match lookup(path) {
            Some(v @ (Value::Number(_) | Value::String(_))) => v,
            Some(Value::Null) => Value::String(String::new()),
            Some(v) => Value::String(format_ro(&v)),
            None => {
                missing.insert(path.to_string());
                Value::String(String::new())
            }
        });
    }
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find('{') {
        out.push_str(&rest[..i]);
        let Some(j) = rest[i..].find('}') else {
            out.push_str(&rest[i..]);
            rest = "";
            break;
        };
        let path = &rest[i + 1..i + j];
        match lookup(path) {
            Some(v) => out.push_str(&format_ro(&v)),
            None => {
                missing.insert(path.to_string());
            }
        }
        rest = &rest[i + j + 1..];
    }
    out.push_str(rest);
    Ok(Value::String(out))
}

fn cell_xml(col: &str, row: u32, attrs: &str, v: &Value) -> String {
    match v {
        Value::Number(n) => format!("<c r=\"{col}{row}\"{attrs}><v>{n}</v></c>"),
        Value::String(s) if s.is_empty() => format!("<c r=\"{col}{row}\"{attrs}/>"),
        other => format!("<c r=\"{col}{row}\"{attrs} t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>", xml_escape(&format_ro(other))),
    }
}

fn row_tag_list(text: &str) -> Option<String> {
    let i = text.find("[].")?;
    let start = text[..i].rfind('{')? + 1;
    Some(text[start..i].to_string())
}

fn fill_sheet(xml: &str, sst: &[String], data: &Value, missing: &mut BTreeSet<String>) -> Result<String, String> {
    let Some(sd_start) = xml.find("<sheetData") else { return Ok(xml.to_string()) };
    let body_start = sd_start + xml[sd_start..].find('>').ok_or("sheetData")? + 1;
    if xml[sd_start..body_start].ends_with("/>") {
        return Ok(xml.to_string());
    }
    let body_end = body_start + xml[body_start..].find("</sheetData>").ok_or("sheetData")?;
    let body = &xml[body_start..body_end];

    let mut new_body = String::new();
    let mut shifts: Vec<(u32, i64)> = Vec::new(); // (rândul șablon, delta) pentru renumerotare
    let mut offset: i64 = 0;
    let mut rest = body;
    while let Some(i) = rest.find("<row") {
        let after = &rest[i..];
        let open_end = after.find('>').ok_or("row")?;
        let open = &after[..open_end + 1];
        let (inner, consumed) = if open.ends_with("/>") {
            ("", open.len())
        } else {
            let close = after.find("</row>").ok_or("row")?;
            (&after[open.len()..close], close + 6)
        };
        let old_r: u32 = attr(open, "r").and_then(|v| v.parse().ok()).unwrap_or(0);
        let cells = parse_cells(inner, sst);
        let open_rest = |r: u32| {
            let mut o = open.trim_end_matches("/>").trim_end_matches('>').to_string();
            if let Some(v) = attr(&o, "r") {
                o = o.replacen(&format!(" r=\"{v}\""), &format!(" r=\"{r}\""), 1);
            }
            o.push('>');
            o
        };
        let list = cells.iter().find_map(|c| c.text.as_deref().and_then(row_tag_list));
        if let Some(list) = list {
            let items: Vec<Value> = match resolve_path(data, &list) {
                Some(Value::Array(a)) => a.clone(),
                _ => {
                    missing.insert(format!("{list}[]"));
                    Vec::new()
                }
            };
            for (k, item) in items.iter().enumerate() {
                let r = (old_r as i64 + offset) as u32 + k as u32;
                new_body.push_str(&open_rest(r));
                for c in &cells {
                    match &c.text {
                        Some(t) if t.contains('{') => {
                            let prefix = format!("{list}[].");
                            let lookup = |path: &str| -> Option<Value> {
                                match path.strip_prefix(&prefix) {
                                    Some(field) => resolve_path(item, field).cloned(),
                                    None => resolve_path(data, path).cloned(),
                                }
                            };
                            let v = fill_text(t, &lookup, missing)?;
                            new_body.push_str(&cell_xml(&c.col, r, &c.attrs, &v));
                        }
                        _ => new_body.push_str(&retarget(&c.raw, &c.col, r)),
                    }
                }
                new_body.push_str("</row>");
            }
            let delta = items.len() as i64 - 1;
            shifts.push((old_r, delta));
            offset += delta;
        } else {
            let r = (old_r as i64 + offset) as u32;
            new_body.push_str(&open_rest(r));
            for c in &cells {
                match &c.text {
                    Some(t) if t.contains('{') => {
                        let lookup = |path: &str| resolve_path(data, path).cloned();
                        let v = fill_text(t, &lookup, missing)?;
                        new_body.push_str(&cell_xml(&c.col, r, &c.attrs, &v));
                    }
                    _ => new_body.push_str(&retarget(&c.raw, &c.col, r)),
                }
            }
            new_body.push_str("</row>");
        }
        rest = &after[consumed..];
    }

    let map_row = |r: u32| -> u32 {
        let mut d = 0i64;
        for &(tr, delta) in &shifts {
            if r > tr {
                d += delta;
            }
        }
        (r as i64 + d).max(1) as u32
    };
    let map_ref = |rf: &str| -> String {
        rf.split(':')
            .map(|p| {
                let (c, r) = col_row(p);
                if r == 0 { p.to_string() } else { format!("{c}{}", map_row(r)) }
            })
            .collect::<Vec<_>>()
            .join(":")
    };
    let mut out = format!("{}{}{}", &xml[..body_start], new_body, &xml[body_end..]);
    // dimension și celulele îmbinate
    for key in ["<dimension ref=\"", "<mergeCell ref=\""] {
        let mut res = String::new();
        let mut rest = out.as_str();
        while let Some(i) = rest.find(key) {
            let s = i + key.len();
            let e = s + rest[s..].find('"').unwrap_or(0);
            res.push_str(&rest[..s]);
            res.push_str(&map_ref(&rest[s..e]));
            rest = &rest[e..];
        }
        res.push_str(rest);
        out = res;
    }
    Ok(out)
}

/// Schimbă rândul din referința unei celule păstrate neschimbată.
fn retarget(raw: &str, col: &str, row: u32) -> String {
    match attr(raw, "r") {
        Some(old) => raw.replacen(&format!(" r=\"{old}\""), &format!(" r=\"{col}{row}\""), 1),
        None => raw.to_string(),
    }
}

/// Completează șablonul `.xlsx`; întoarce registrul și variabilele negăsite.
pub fn render(template: &[u8], data: &Value) -> Result<(Vec<u8>, BTreeSet<String>), String> {
    let mut zin = zip::ZipArchive::new(Cursor::new(template)).map_err(|e| format!("Fișierul nu este un .xlsx valid: {e}"))?;
    let read = |z: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str| -> Option<String> {
        let mut s = String::new();
        z.by_name(name).ok()?.read_to_string(&mut s).ok()?;
        Some(s)
    };
    let sst = read(&mut zin, "xl/sharedStrings.xml").map(|x| shared_strings(&x)).unwrap_or_default();
    let has_calc_chain = zin.by_name("xl/calcChain.xml").is_ok();
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut missing = BTreeSet::new();
    for i in 0..zin.len() {
        let mut f = zin.by_index(i).map_err(|e| e.to_string())?;
        let name = f.name().to_string();
        if name == "xl/calcChain.xml" {
            continue; // referințele s-ar decala; Excel îl reconstruiește
        }
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        if name.starts_with("xl/worksheets/sheet") && name.ends_with(".xml") {
            let xml = String::from_utf8(buf).map_err(|e| e.to_string())?;
            buf = fill_sheet(&xml, &sst, data, &mut missing).map_err(|e| format!("{name}: {e}"))?.into_bytes();
        } else if has_calc_chain && (name == "[Content_Types].xml" || name == "xl/_rels/workbook.xml.rels") {
            let xml = String::from_utf8(buf).map_err(|e| e.to_string())?;
            buf = strip_calc_chain(&xml).into_bytes();
        }
        out.start_file(name, opts).map_err(|e| e.to_string())?;
        out.write_all(&buf).map_err(|e| e.to_string())?;
    }
    Ok((out.finish().map_err(|e| e.to_string())?.into_inner(), missing))
}

fn strip_calc_chain(xml: &str) -> String {
    let mut out = String::new();
    let mut rest = xml;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let end = rest[i..].find('>').map_or(rest.len(), |e| i + e + 1);
        let tag = &rest[i..end];
        if !tag.contains("calcChain") {
            out.push_str(tag);
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn example_template_rows_and_values() {
        let tpl = include_bytes!("../../../../docs/sabloane/exemplu-liste-cantitati.xlsx");
        let data = json!({
            "proiect": {"nume": "Sat", "localitate": "L", "judet": "J"},
            "cantitati": [
                {"nr": 1, "denumire": "Săpătură", "um": "m³", "cantitate": 120.5, "observatii": ""},
                {"nr": 2, "denumire": "Conductă De 110", "um": "m", "cantitate": 300, "observatii": "Tr1, Tr2"},
                {"nr": 3, "denumire": "Hidrant", "um": "buc", "cantitate": 2, "observatii": ""},
            ],
            "retea": {"lungime_totala": 300, "diametre": []},
        });
        let (bytes, missing) = render(tpl, &data).unwrap();
        assert!(missing.is_empty(), "{missing:?}");
        let mut z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut s1 = String::new();
        z.by_name("xl/worksheets/sheet1.xml").unwrap().read_to_string(&mut s1).unwrap();
        assert!(s1.contains("Sat"));
        // 3 rânduri generate (5, 6, 7), iar totalul de pe rândul 7 coboară pe 9
        assert!(s1.contains("<c r=\"B7\" t=\"inlineStr\"><is><t xml:space=\"preserve\">Hidrant</t>"), "{s1}");
        assert!(s1.contains("<c r=\"D6\"><v>300</v></c>"));
        assert!(s1.contains("<c r=\"D9\"><v>300</v></c>"), "totalul s-a mutat pe rândul 9");
        assert!(s1.contains("<dimension ref=\"A1:E9\"/>"));
        let mut s2 = String::new();
        z.by_name("xl/worksheets/sheet2.xml").unwrap().read_to_string(&mut s2).unwrap();
        assert!(!s2.contains("diametre[]"), "lista goală elimină rândul");
    }
}
