//! Completarea unui șablon Word (.docx) cu sintaxa docxtemplater folosită de aplicația React:
//!
//! - valoare: `{proiect.nume}`; în buclă, câmpurile elementului direct (`{nume}`), iar
//!   variabilele globale rămân accesibile; `{.}` = elementul curent;
//! - buclă / condiție: `{#lista}…{/lista}`; negare: `{^x}…{/x}`;
//!   - ambele tag-uri în același paragraf → se repetă textul dintre ele;
//!   - în rânduri ale aceluiași tabel → se repetă rândurile (un rând per element);
//!   - în paragrafe diferite → se repetă paragrafele (un paragraf care conține doar
//!     tag-ul de buclă dispare din rezultat — opțiunea `paragraphLoop`);
//! - numerele se formatează românește; `\n` din valori devine întrerupere de rând.
//!
//! Word poate sparge un tag în mai multe „run”-uri; paragrafele afectate sunt
//! unificate în primul run (formatarea parțială din acel paragraf se pierde).

use super::document::{format_ro, resolve_path};
use serde_json::Value;
use std::collections::BTreeSet;
use std::io::{Cursor, Read, Write};

#[derive(Debug, Clone)]
struct Tag {
    start: usize,
    end: usize,
    kind: char, // ' ' valoare, '#', '^', '/'
    name: String,
}

#[derive(Debug, Clone)]
struct Loop {
    open: usize, // index în `tags`
    outer: (usize, usize),
    inner: (usize, usize),
}

fn find_back(xml: &str, pos: usize, pats: &[&str]) -> Option<usize> {
    pats.iter().filter_map(|p| xml[..pos].rfind(p)).max()
}

/// Începutul elementului `<w:p>` / `<w:tr>` care conține `pos` (nu `<w:pPr>`, `<w:trPr>`).
fn enclosing(xml: &str, pos: usize, el: &str) -> Option<(usize, usize)> {
    let open_a = format!("<{el}>");
    let open_b = format!("<{el} ");
    let close = format!("</{el}>");
    let start = find_back(xml, pos, &[&open_a, &open_b])?;
    // dacă între start și pos s-a închis deja elementul, nu e cel care conține pos
    if xml[start..pos].contains(&close) {
        return None;
    }
    let end = pos + xml[pos..].find(&close)? + close.len();
    Some((start, end))
}

/// Textul tuturor `<w:t>` din paragraf.
fn para_text(p: &str) -> String {
    let mut out = String::new();
    let mut rest = p;
    while let Some(i) = rest.find("<w:t") {
        let after = &rest[i + 4..];
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
        let Some(close) = body.find("</w:t>") else { break };
        out.push_str(&body[..close]);
        rest = &body[close..];
    }
    out
}

/// Unifică textul paragrafelor în care un tag e spart între mai multe `<w:t>`.
fn normalize(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut i = 0;
    while let Some(rel) = xml[i..].find("<w:p") {
        let s = i + rel;
        let next = xml.as_bytes().get(s + 4).copied();
        if !matches!(next, Some(b'>') | Some(b' ')) {
            out.push_str(&xml[i..s + 4]);
            i = s + 4;
            continue;
        }
        let Some(e_rel) = xml[s..].find("</w:p>") else { break };
        let e = s + e_rel + 6;
        let p = &xml[s..e];
        out.push_str(&xml[i..s]);
        let pieces: Vec<&str> = p.split("<w:t").skip(1).collect();
        let split = pieces.iter().any(|piece| {
            let txt = piece.split_once('>').map(|(_, b)| b.split("</w:t>").next().unwrap_or("")).unwrap_or("");
            txt.matches('{').count() != txt.matches('}').count()
        });
        if split {
            let full = para_text(p);
            let mut first = true;
            let mut res = String::new();
            let mut rest = p;
            while let Some(k) = rest.find("<w:t") {
                let after = &rest[k + 4..];
                if !(after.starts_with('>') || after.starts_with(' ')) {
                    res.push_str(&rest[..k + 4]);
                    rest = after;
                    continue;
                }
                let gt = after.find('>').unwrap_or(0);
                let body = &after[gt + 1..];
                let close = body.find("</w:t>").unwrap_or(0);
                res.push_str(&rest[..k]);
                if first {
                    res.push_str(&format!("<w:t xml:space=\"preserve\">{full}</w:t>"));
                    first = false;
                } else {
                    res.push_str("<w:t></w:t>");
                }
                rest = &body[close + 6..];
            }
            res.push_str(rest);
            out.push_str(&res);
        } else {
            out.push_str(p);
        }
        i = e;
    }
    out.push_str(&xml[i..]);
    // tag-urile au nevoie de spațiile păstrate
    out.replace("<w:t>{", "<w:t xml:space=\"preserve\">{")
}

/// Tag-urile din conținutul `<w:t>` (după normalizare, fiecare e într-un singur `<w:t>`).
fn scan(xml: &str) -> Vec<Tag> {
    let mut tags = Vec::new();
    let mut i = 0;
    let b = xml.as_bytes();
    let mut in_text = false;
    while i < b.len() {
        if b[i] == b'<' {
            in_text = xml[i..].starts_with("<w:t>") || xml[i..].starts_with("<w:t ");
            i += 1;
            continue;
        }
        if in_text && b[i] == b'{' {
            if let Some(len) = xml[i..].find('}') {
                let inner = xml[i + 1..i + len].trim().to_string();
                if !inner.is_empty() && !inner.contains('<') {
                    let (kind, name) = match inner.chars().next() {
                        Some(c @ ('#' | '^' | '/')) => (c, inner[1..].trim().to_string()),
                        _ => (' ', inner),
                    };
                    tags.push(Tag { start: i, end: i + len + 1, kind, name });
                    i += len + 1;
                    continue;
                }
            }
        }
        i += 1;
    }
    tags
}

fn build_loops(xml: &str, tags: &[Tag]) -> Result<Vec<Loop>, String> {
    let mut stack: Vec<usize> = Vec::new();
    let mut loops = Vec::new();
    for (i, t) in tags.iter().enumerate() {
        match t.kind {
            '#' | '^' => stack.push(i),
            '/' => {
                let o = stack.pop().ok_or_else(|| format!("tag de închidere fără deschidere: {{/{}}}", t.name))?;
                if tags[o].name != t.name {
                    return Err(format!("{{#{}}} închis cu {{/{}}}", tags[o].name, t.name));
                }
                let (ot, ct) = (&tags[o], t);
                let p_open = enclosing(xml, ot.start, "w:p");
                let p_close = enclosing(xml, ct.start, "w:p");
                let tr_open = enclosing(xml, ot.start, "w:tr");
                let tr_close = enclosing(xml, ct.start, "w:tr");
                let same_table = |a: (usize, usize), b: (usize, usize)| !xml[a.0..b.1].contains("</w:tbl>");
                let (outer, inner) = if p_open.is_some() && p_open == p_close {
                    ((ot.start, ct.end), (ot.end, ct.start))
                } else if let (Some(a), Some(b)) = (tr_open, tr_close).filter_pair(|a, b| same_table(*a, *b)) {
                    ((a.0, b.1), (a.0, b.1))
                } else if let (Some(a), Some(b)) = (p_open, p_close) {
                    let only = |p: (usize, usize), tag: &Tag| para_text(&xml[p.0..p.1]).trim() == xml[tag.start..tag.end].trim();
                    if only(a, ot) && only(b, ct) {
                        ((a.0, b.1), (a.1, b.0))
                    } else {
                        ((a.0, b.1), (a.0, b.1))
                    }
                } else {
                    ((ot.start, ct.end), (ot.end, ct.start))
                };
                loops.push(Loop { open: o, outer, inner });
            }
            _ => {}
        }
    }
    if let Some(o) = stack.pop() {
        return Err(format!("buclă neînchisă: {{#{}}}", tags[o].name));
    }
    Ok(loops)
}

trait PairFilter<A> {
    fn filter_pair(self, f: impl Fn(&A, &A) -> bool) -> (Option<A>, Option<A>);
}
impl<A> PairFilter<A> for (Option<A>, Option<A>) {
    fn filter_pair(self, f: impl Fn(&A, &A) -> bool) -> (Option<A>, Option<A>) {
        match self {
            (Some(a), Some(b)) if f(&a, &b) => (Some(a), Some(b)),
            _ => (None, None),
        }
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn lookup<'a>(scopes: &'a [Value], path: &str) -> Option<&'a Value> {
    if path == "." || path == "this" {
        return scopes.last();
    }
    scopes.iter().rev().find_map(|s| resolve_path(s, path))
}

fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

struct Ctx<'a> {
    xml: &'a str,
    tags: &'a [Tag],
    loops: &'a [Loop],
    missing: BTreeSet<String>,
}

impl Ctx<'_> {
    fn render(&mut self, range: (usize, usize), scopes: &mut Vec<Value>, skip: &[usize]) -> String {
        let mut out = String::new();
        let mut pos = range.0;
        // buclele de nivel superior din interval
        let mut tops: Vec<&Loop> = self.loops.iter().filter(|l| l.outer.0 >= range.0 && l.outer.1 <= range.1 && !skip.contains(&l.open)).collect();
        tops.sort_by_key(|l| l.outer.0);
        let mut filtered: Vec<&Loop> = Vec::new();
        for l in tops {
            if filtered.last().is_none_or(|p: &&Loop| l.outer.0 >= p.outer.1) {
                filtered.push(l);
            }
        }
        let mut li = 0;
        let tags: Vec<(usize, &Tag)> = self.tags.iter().enumerate().filter(|(_, t)| t.start >= range.0 && t.end <= range.1).collect();
        let mut ti = 0;
        while pos < range.1 {
            let next_loop = filtered.get(li).map(|l| l.outer.0);
            let next_tag = tags.get(ti).map(|(_, t)| t.start);
            match (next_loop, next_tag) {
                (Some(ls), nt) if nt.is_none_or(|t| ls <= t) => {
                    let l = filtered[li];
                    out.push_str(&self.xml[pos..l.outer.0]);
                    out.push_str(&self.render_loop(l, scopes));
                    pos = l.outer.1;
                    li += 1;
                    while tags.get(ti).is_some_and(|(_, t)| t.start < pos) {
                        ti += 1;
                    }
                }
                (_, Some(ts)) => {
                    let (_, t) = tags[ti];
                    out.push_str(&self.xml[pos..ts]);
                    if t.kind == ' ' {
                        match lookup(scopes, &t.name) {
                            Some(v) if !v.is_null() => out.push_str(&escape(&format_ro(v)).replace('\n', "</w:t><w:br/><w:t xml:space=\"preserve\">")),
                            _ => {
                                self.missing.insert(t.name.clone());
                            }
                        }
                    }
                    pos = t.end;
                    ti += 1;
                }
                _ => {
                    out.push_str(&self.xml[pos..range.1]);
                    pos = range.1;
                }
            }
        }
        out
    }

    fn render_loop(&mut self, l: &Loop, scopes: &mut Vec<Value>) -> String {
        let open = &self.tags[l.open];
        let (kind, name) = (open.kind, open.name.clone());
        let val = lookup(scopes, &name).cloned();
        if val.is_none() {
            self.missing.insert(name);
        }
        let skip = [l.open];
        let mut out = String::new();
        if kind == '^' {
            if !truthy(val.as_ref()) {
                out.push_str(&self.render(l.inner, scopes, &skip));
            }
            return out;
        }
        match val {
            Some(Value::Array(items)) => {
                for it in items {
                    scopes.push(it);
                    out.push_str(&self.render(l.inner, scopes, &skip));
                    scopes.pop();
                }
            }
            Some(obj @ Value::Object(_)) => {
                scopes.push(obj);
                out.push_str(&self.render(l.inner, scopes, &skip));
                scopes.pop();
            }
            v if truthy(v.as_ref()) => out.push_str(&self.render(l.inner, scopes, &skip)),
            _ => {}
        }
        out
    }
}

/// Completează un XML Word; întoarce XML-ul rezultat și variabilele negăsite.
pub fn render_xml(xml: &str, data: &Value) -> Result<(String, BTreeSet<String>), String> {
    let xml = normalize(xml);
    let tags = scan(&xml);
    let loops = build_loops(&xml, &tags)?;
    let mut ctx = Ctx { xml: &xml, tags: &tags, loops: &loops, missing: BTreeSet::new() };
    let mut scopes = vec![data.clone()];
    let out = ctx.render((0, xml.len()), &mut scopes, &[]);
    Ok((out, ctx.missing))
}

/// Completează șablonul `.docx`; întoarce documentul și variabilele negăsite.
pub fn render(template: &[u8], data: &Value) -> Result<(Vec<u8>, BTreeSet<String>), String> {
    let mut zin = zip::ZipArchive::new(Cursor::new(template)).map_err(|e| format!("Fișierul nu este un .docx valid: {e}"))?;
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut missing = BTreeSet::new();
    for i in 0..zin.len() {
        let mut f = zin.by_index(i).map_err(|e| e.to_string())?;
        let name = f.name().to_string();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        let is_part = name.starts_with("word/") && name.ends_with(".xml") && (name == "word/document.xml" || name.contains("header") || name.contains("footer"));
        if is_part {
            let xml = String::from_utf8(buf).map_err(|e| e.to_string())?;
            let (res, miss) = render_xml(&xml, data).map_err(|e| format!("{name}: {e}"))?;
            missing.extend(miss);
            buf = res.into_bytes();
        }
        out.start_file(name, opts).map_err(|e| e.to_string())?;
        out.write_all(&buf).map_err(|e| e.to_string())?;
    }
    let bytes = out.finish().map_err(|e| e.to_string())?.into_inner();
    Ok((bytes, missing))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn p(t: &str) -> String {
        format!("<w:p><w:r><w:t xml:space=\"preserve\">{t}</w:t></w:r></w:p>")
    }

    #[test]
    fn values_loops_conditions_and_rows() {
        let data = json!({
            "proiect": {"nume": "Sat & co", "data": "05.11.2026"},
            "q": 1234.5,
            "are": true, "nu": false,
            "lista": [{"nume": "Tr1", "l": 12.5}, {"nume": "Tr2", "l": 7}],
        });
        let xml = format!(
            "<w:document><w:body>{}{}{}{}{}{}<w:tbl><w:tr><w:tc>{}</w:tc><w:tc>{}</w:tc></w:tr></w:tbl>{}</w:body></w:document>",
            p("Proiect: {proiect.nume}, Q = {q}"),
            p("{#are}DA{/are}{#nu}NU{/nu}{^nu}neg{/nu}"),
            p("{#lista}"),
            p("• {nume} ({proiect.nume})"),
            p("{/lista}"),
            p("{#lista}{nume};{/lista}"),
            p("{#lista}{nume}"),
            p("{l}{/lista}"),
            p("{lipsa.x}")
        );
        let (out, missing) = render_xml(&xml, &data).unwrap();
        assert!(out.contains("Proiect: Sat &amp; co, Q = 1.234,5"));
        assert!(out.contains(">DAneg<"));
        assert_eq!(out.matches("• Tr").count(), 2);
        assert!(out.contains("• Tr2 (Sat &amp; co)"));
        assert!(!out.contains("{#lista}") && !out.contains("{/lista}"));
        assert!(out.contains(">Tr1;Tr2;<"));
        assert_eq!(out.matches("<w:tr>").count(), 2, "un rând de tabel per element");
        assert!(out.contains(">12,5<") && out.contains(">7<"));
        assert!(missing.contains("lipsa.x"));
    }

    #[test]
    fn split_runs_are_merged() {
        let xml = "<w:p><w:r><w:t>Nume: {pro</w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>iect.nume}</w:t></w:r></w:p>";
        let (out, missing) = render_xml(xml, &json!({"proiect": {"nume": "X"}})).unwrap();
        assert!(out.contains("Nume: X"), "{out}");
        assert!(missing.is_empty());
    }
}
