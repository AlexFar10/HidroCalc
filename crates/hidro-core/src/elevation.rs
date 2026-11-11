//! Cote de teren din modelul digital al terenului (DEM), din dalele publice
//! „Terrarium" de pe AWS (Mapzen/Tilezen; în România sursa e SRTM ~30 m).
//!
//! Fiecare dală PNG 256×256 codifică altitudinea în culoare:
//! `h = R·256 + G + B/256 − 32768` [m]. Dalele descărcate se păstrează în
//! directorul de cache al aplicației, deci zonele deja interogate merg offline.
//!
//! Precizie verticală tipică ±2–5 m: valorile sunt *estimative* și nu înlocuiesc
//! ridicarea topografică.

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// z13 ≈ 13 m/pixel la latitudinea României — sub rezoluția datelor SRTM (30 m),
/// deci un zoom mai mare n-ar aduce informație nouă, doar mai multe dale.
pub const ZOOM: u32 = 13;
const TILE_SIZE: usize = 256;
const TILE_URL: &str = "https://s3.amazonaws.com/elevation-tiles-prod/terrarium";
const MAX_LAT: f64 = 85.051_128_78;
const MAX_POINTS: usize = 10_000;

pub use crate::geo::LatLng;

/// Altitudinile terenului [m] pentru fiecare punct, în aceeași ordine.
/// `None` pentru coordonate invalide sau în afara proiecției Web Mercator.
///
/// `cache_dir` = directorul în care se păstrează dalele descărcate (pentru
/// zoom-ul [`ZOOM`]). Funcția e blocantă (rețea + disc): apelați-o dintr-un
/// fir de lucru, nu din firul interfeței.
pub fn fetch_elevations(points: &[LatLng], cache_dir: PathBuf) -> Result<Vec<Option<f64>>, String> {
    if points.len() > MAX_POINTS {
        return Err(format!("Prea multe puncte ({} > {MAX_POINTS})", points.len()));
    }
    let mut sampler = Sampler::new(cache_dir);
    points.iter().map(|p| sampler.elevation(p.lat, p.lng)).collect()
}

/// Dală decodată: 256×256 altitudini, rând cu rând.
type Tile = Vec<f32>;

struct Sampler {
    cache_dir: PathBuf,
    tiles: HashMap<(u32, u32), Tile>,
    agent: ureq::Agent,
}

impl Sampler {
    fn new(cache_dir: PathBuf) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(20))
            .user_agent(concat!("HidroCalc/", env!("CARGO_PKG_VERSION")))
            .build();
        Self { cache_dir, tiles: HashMap::new(), agent }
    }

    /// Interpolare biliniară între cei 4 pixeli din jurul punctului; pixelii
    /// vecini pot cădea în dale diferite.
    fn elevation(&mut self, lat: f64, lng: f64) -> Result<Option<f64>, String> {
        if !lat.is_finite() || !lng.is_finite() || lat.abs() > MAX_LAT || lng.abs() > 180.0 {
            return Ok(None);
        }
        let (gx, gy) = global_px(lat, lng, ZOOM);
        // Centrul pixelului (i, j) e la (i + 0.5, j + 0.5).
        let fx = gx - 0.5;
        let fy = gy - 0.5;
        let x0 = fx.floor();
        let y0 = fy.floor();
        let tx = fx - x0;
        let ty = fy - y0;
        let max = (TILE_SIZE as i64) << ZOOM;
        let clamp = |v: f64| (v as i64).clamp(0, max - 1) as u64;
        let (x0, y0) = (clamp(x0), clamp(y0));
        let (x1, y1) = (clamp(fx.floor() + 1.0), clamp(fy.floor() + 1.0));

        let h00 = self.pixel(x0, y0)?;
        let h10 = self.pixel(x1, y0)?;
        let h01 = self.pixel(x0, y1)?;
        let h11 = self.pixel(x1, y1)?;
        let top = h00 * (1.0 - tx) + h10 * tx;
        let bottom = h01 * (1.0 - tx) + h11 * tx;
        let h = top * (1.0 - ty) + bottom * ty;
        Ok(Some((h * 100.0).round() / 100.0))
    }

    fn pixel(&mut self, gx: u64, gy: u64) -> Result<f64, String> {
        let key = ((gx / TILE_SIZE as u64) as u32, (gy / TILE_SIZE as u64) as u32);
        if !self.tiles.contains_key(&key) {
            let tile = self.load_tile(key.0, key.1)?;
            self.tiles.insert(key, tile);
        }
        let tile = &self.tiles[&key];
        let px = (gx % TILE_SIZE as u64) as usize;
        let py = (gy % TILE_SIZE as u64) as usize;
        Ok(tile[py * TILE_SIZE + px] as f64)
    }

    fn load_tile(&self, x: u32, y: u32) -> Result<Tile, String> {
        let path = self.cache_dir.join(format!("{x}_{y}.png"));
        if let Ok(bytes) = fs::read(&path) {
            if let Ok(tile) = decode_terrarium(&bytes) {
                return Ok(tile);
            }
            // Fișier corupt în cache → îl descărcăm din nou.
        }
        let bytes = self.download(x, y)?;
        let tile = decode_terrarium(&bytes)?;
        write_cache(&path, &bytes);
        Ok(tile)
    }

    fn download(&self, x: u32, y: u32) -> Result<Vec<u8>, String> {
        let url = format!("{TILE_URL}/{ZOOM}/{x}/{y}.png");
        let resp = self
            .agent
            .get(&url)
            .call()
            .map_err(|e| format!("Nu s-a putut descărca dala de altitudini ({e}). Verificați conexiunea la internet."))?;
        let mut bytes = Vec::new();
        resp.into_reader()
            .take(4 * 1024 * 1024)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("Eroare la citirea dalei: {e}"))?;
        Ok(bytes)
    }
}

/// Coordonate pixel globale Web Mercator la nivelul de zoom `z`.
fn global_px(lat: f64, lng: f64, z: u32) -> (f64, f64) {
    let world = (TILE_SIZE as f64) * f64::from(1u32 << z);
    let x = (lng + 180.0) / 360.0 * world;
    let sin = lat.to_radians().sin();
    let y = (0.5 - ((1.0 + sin) / (1.0 - sin)).ln() / (4.0 * std::f64::consts::PI)) * world;
    (x, y)
}

fn decode_terrarium(bytes: &[u8]) -> Result<Tile, String> {
    let mut decoder = png::Decoder::new(bytes);
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| format!("PNG invalid: {e}"))?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(|e| format!("PNG invalid: {e}"))?;
    if info.width as usize != TILE_SIZE || info.height as usize != TILE_SIZE {
        return Err(format!("Dală cu dimensiuni neașteptate: {}×{}", info.width, info.height));
    }
    let channels = match info.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        other => return Err(format!("Format PNG neașteptat: {other:?}")),
    };
    let stride = info.line_size;
    let mut out = Vec::with_capacity(TILE_SIZE * TILE_SIZE);
    for row in 0..TILE_SIZE {
        let line = &buf[row * stride..row * stride + TILE_SIZE * channels];
        for px in line.chunks_exact(channels) {
            let (r, g, b) = (px[0] as f32, px[1] as f32, px[2] as f32);
            out.push(r * 256.0 + g + b / 256.0 - 32768.0);
        }
    }
    Ok(out)
}

/// Scriere atomică (fișier temporar + rename); eșecul cache-ului nu e fatal.
fn write_cache(path: &Path, bytes: &[u8]) {
    let Some(dir) = path.parent() else { return };
    if fs::create_dir_all(dir).is_err() {
        return;
    }
    let tmp = path.with_extension("png.tmp");
    if fs::write(&tmp, bytes).is_ok() && fs::rename(&tmp, path).is_err() {
        let _ = fs::remove_file(&tmp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_px_origin_and_center() {
        let (x, y) = global_px(0.0, 0.0, 0);
        assert!((x - 128.0).abs() < 1e-9 && (y - 128.0).abs() < 1e-9);
        let (x, _) = global_px(0.0, -180.0, 1);
        assert!(x.abs() < 1e-9);
    }

    #[test]
    fn terrarium_formula() {
        // R=128, G=0, B=0 → 0 m
        let v = 128.0f32 * 256.0 + 0.0 + 0.0 / 256.0 - 32768.0;
        assert_eq!(v, 0.0);
    }
}

#[cfg(test)]
mod network_tests {
    use super::*;

    /// Necesită internet: `cargo test -- --ignored elevation`
    #[test]
    #[ignore]
    fn elevation_known_points() {
        let dir = std::env::temp_dir().join("hidrocalc-terrarium-test");
        let mut s = Sampler::new(dir);
        // Timișoara, Piața Victoriei (~90 m)
        let tm = s.elevation(45.7537, 21.2257).unwrap().unwrap();
        // Brașov, Piața Sfatului (~570 m)
        let bv = s.elevation(45.6427, 25.5887).unwrap().unwrap();
        println!("Timișoara {tm} m, Brașov {bv} m");
        assert!((70.0..120.0).contains(&tm));
        assert!((520.0..620.0).contains(&bv));
    }
}
