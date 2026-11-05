//! Import de puncte topografice (X, Y, Z) din CSV/TXT, GPX sau KML (portat din `topoImport.ts`).
//!
//! Sisteme acceptate: WGS84 (lat, lng) și Stereo 70; convenția topografică românească
//! e X = Nord, Y = Est.

use crate::geo::{stereo70_to_wgs84, LatLng};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Crs {
    Auto,
    Wgs84,
    /// Stereo 70 cu X = Nord, Y = Est (convenția topografică).
    Stereo70,
    /// Stereo 70 cu ordinea Est, Nord.
    Stereo70En,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopoPoint {
    pub id: String,
    pub lat: f64,
    pub lng: f64,
    pub z: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Csv,
    Gpx,
    Kml,
}

#[derive(Debug, Clone)]
pub struct ParseResult {
    pub points: Vec<(Option<String>, LatLng, f64)>,
    pub skipped_no_z: usize,
    pub skipped_invalid: usize,
    pub format: Format,
}

impl ParseResult {
    fn new(format: Format) -> Self {
        Self { points: Vec::new(), skipped_no_z: 0, skipped_invalid: 0, format }
    }
}

fn ro_lat(v: f64) -> bool {
    (43.0..=49.0).contains(&v)
}
fn ro_lng(v: f64) -> bool {
    (20.0..=30.5).contains(&v)
}
fn valid_z(z: f64) -> bool {
    z.is_finite() && z > -500.0 && z < 9000.0
}

fn to_latlng(a: f64, b: f64, crs: Crs) -> Option<LatLng> {
    let geo = a.abs() <= 180.0 && b.abs() <= 180.0;
    let mode = if crs == Crs::Auto { if geo { Crs::Wgs84 } else { Crs::Stereo70 } } else { crs };
    match mode {
        Crs::Wgs84 => {
            if !geo {
                return None;
            }
            if ro_lng(a) && ro_lat(b) && !ro_lat(a) {
                return Some(LatLng { lat: b, lng: a });
            }
            (a.abs() <= 90.0).then_some(LatLng { lat: a, lng: b })
        }
        _ if geo => None,
        Crs::Stereo70 => Some(stereo70_to_wgs84(b, a)),
        _ => Some(stereo70_to_wgs84(a, b)),
    }
}

/// Număr cu `,` sau `.` zecimal; spațiile interioare sunt ignorate.
fn num(s: &str) -> Option<f64> {
    let t: String = s.trim().chars().filter(|c| !c.is_whitespace()).collect::<String>().replacen(',', ".", 1);
    let ok = !t.is_empty()
        && t.chars().enumerate().all(|(i, c)| c.is_ascii_digit() || c == '.' || c == 'e' || c == 'E' || ((c == '-' || c == '+') && (i == 0 || matches!(t.as_bytes()[i - 1], b'e' | b'E'))));
    if ok { t.parse::<f64>().ok().filter(|v| v.is_finite()) } else { None }
}

fn split_row(line: &str) -> Vec<String> {
    if line.contains(';') {
        return line.split(';').map(str::to_owned).collect();
    }
    if line.contains('\t') {
        return line.split('\t').map(str::to_owned).collect();
    }
    let ws: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
    if ws.iter().filter(|c| num(c).is_some()).count() >= 3 {
        return ws;
    }
    if line.matches(',').count() >= 2 {
        return line.split(',').map(str::to_owned).collect();
    }
    ws
}

fn parse_csv(text: &str, crs: Crs) -> ParseResult {
    let mut res = ParseResult::new(Format::Csv);
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cells: Vec<String> = split_row(line).into_iter().map(|c| c.trim().trim_matches('"').to_string()).collect();
        let nums: Vec<Option<f64>> = cells.iter().map(|c| num(c)).collect();
        let numeric: Vec<f64> = nums.iter().flatten().copied().collect();
        if numeric.len() < 2 {
            continue; // antet sau text
        }
        let mut name = cells.iter().zip(&nums).find(|(c, n)| n.is_none() && !c.is_empty()).map(|(c, _)| c.clone());
        let mut vals = numeric.clone();
        if name.is_none() && numeric.len() >= 4 {
            name = Some(format!("{}", numeric[0]));
            vals = numeric[1..].to_vec();
        }
        if vals.len() < 3 {
            res.skipped_no_z += 1;
            continue;
        }
        match to_latlng(vals[0], vals[1], crs) {
            Some(ll) if valid_z(vals[2]) => res.points.push((name, ll, vals[2])),
            _ => res.skipped_invalid += 1,
        }
    }
    res
}

fn child_text<'a>(n: roxmltree::Node<'a, 'a>, tag: &str) -> Option<&'a str> {
    n.descendants().find(|c| c.tag_name().name() == tag).and_then(|c| c.text())
}

fn parse_gpx(doc: &roxmltree::Document) -> ParseResult {
    let mut res = ParseResult::new(Format::Gpx);
    for el in doc.descendants().filter(|n| matches!(n.tag_name().name(), "wpt" | "trkpt" | "rtept")) {
        let lat = el.attribute("lat").and_then(|v| v.trim().parse::<f64>().ok());
        let lng = el.attribute("lon").and_then(|v| v.trim().parse::<f64>().ok());
        let (Some(lat), Some(lng)) = (lat, lng) else {
            res.skipped_invalid += 1;
            continue;
        };
        let z = child_text(el, "ele").and_then(|v| v.trim().parse::<f64>().ok()).unwrap_or(f64::NAN);
        if !valid_z(z) {
            res.skipped_no_z += 1;
            continue;
        }
        let name = child_text(el, "name").map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        res.points.push((name, LatLng { lat, lng }, z));
    }
    res
}

fn parse_kml(doc: &roxmltree::Document) -> ParseResult {
    let mut res = ParseResult::new(Format::Kml);
    for pm in doc.descendants().filter(|n| n.tag_name().name() == "Placemark") {
        let name = child_text(pm, "name").map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        // Implicit KML e „clampToGround”: altitudinea din fișier nu e cota terenului.
        let has_z = child_text(pm, "altitudeMode").map(str::trim).unwrap_or("clampToGround") == "absolute";
        let tuples: Vec<&str> = pm
            .descendants()
            .filter(|n| n.tag_name().name() == "coordinates")
            .filter_map(|n| n.text())
            .flat_map(str::split_whitespace)
            .collect();
        let multi = tuples.len() > 1;
        for (i, tuple) in tuples.iter().enumerate() {
            let parts: Vec<f64> = tuple.split(',').map(|v| v.trim().parse::<f64>().unwrap_or(f64::NAN)).collect();
            let (lng, lat) = (parts.first().copied().unwrap_or(f64::NAN), parts.get(1).copied().unwrap_or(f64::NAN));
            if !lat.is_finite() || !lng.is_finite() {
                res.skipped_invalid += 1;
                continue;
            }
            match parts.get(2) {
                Some(&alt) if has_z && valid_z(alt) => {
                    let label = name.as_ref().map(|n| if multi { format!("{n}.{}", i + 1) } else { n.clone() });
                    res.points.push((label, LatLng { lat, lng }, alt));
                }
                _ => res.skipped_no_z += 1,
            }
        }
    }
    res
}

pub fn parse_file(file_name: &str, text: &str, crs: Crs) -> ParseResult {
    let head: String = text.chars().take(2000).collect::<String>().to_lowercase();
    let ext = file_name.rsplit('.').next().unwrap_or("").to_lowercase();
    let gpx = ext == "gpx" || head.contains("<gpx");
    if gpx || ext == "kml" || head.contains("<kml") {
        return match roxmltree::Document::parse(text) {
            Ok(doc) => if gpx { parse_gpx(&doc) } else { parse_kml(&doc) },
            Err(_) => ParseResult { skipped_invalid: 1, ..ParseResult::new(if gpx { Format::Gpx } else { Format::Kml }) },
        };
    }
    parse_csv(text, crs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_stereo70_with_names_and_comma_decimals() {
        let txt = "Nr;X;Y;Z\nP1;476000;208000;91,5\n2 476010,5 208012,25 92,10\nheader text\nP3;476000;208000\n";
        let r = parse_file("puncte.csv", txt, Crs::Auto);
        assert_eq!(r.points.len(), 2);
        assert_eq!(r.points[0].0.as_deref(), Some("P1"));
        assert!((r.points[0].1.lat - 45.722138309).abs() < 1e-6); // X=Nord, Y=Est
        assert_eq!(r.points[1].0.as_deref(), Some("2"));
        assert!((r.points[1].2 - 92.10).abs() < 1e-9);
        assert_eq!(r.skipped_no_z, 1);
    }

    #[test]
    fn csv_wgs84_lng_lat_order_detected() {
        let r = parse_file("a.txt", "21.2257,45.7537,90\n", Crs::Auto);
        assert_eq!(r.points[0].1, LatLng { lat: 45.7537, lng: 21.2257 });
    }

    #[test]
    fn gpx_and_kml() {
        let gpx = r#"<gpx><wpt lat="45.75" lon="21.22"><ele>91.2</ele><name>A</name></wpt><wpt lat="45.76" lon="21.23"/></gpx>"#;
        let r = parse_file("t.gpx", gpx, Crs::Auto);
        assert_eq!((r.points.len(), r.skipped_no_z), (1, 1));
        let kml = r#"<kml><Placemark><name>L</name><altitudeMode>absolute</altitudeMode><LineString><coordinates>21.2,45.7,90 21.3,45.8,95</coordinates></LineString></Placemark><Placemark><Point><coordinates>21,45,100</coordinates></Point></Placemark></kml>"#;
        let r = parse_file("t.kml", kml, Crs::Auto);
        assert_eq!(r.points.len(), 2);
        assert_eq!(r.points[1].0.as_deref(), Some("L.2"));
        assert_eq!(r.skipped_no_z, 1); // clampToGround
    }
}
