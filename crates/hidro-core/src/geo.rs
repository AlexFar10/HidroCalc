//! Geografie: proiecția Web Mercator a dalelor OSM, legătura schemă ↔ WGS84 prin
//! ancora hărții (portat din `mapTiles.ts`) și transformarea Stereo 70 → WGS84.

use serde::{Deserialize, Serialize};
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

pub const TILE_SIZE: f64 = 256.0;
const EARTH_CIRCUMFERENCE: f64 = 40_075_016.686;
const PX_TO_MM: f64 = 0.26458;

/// Ancora hărții: punctul schemei (worldX, worldY) corespunde coordonatelor (lat, lng);
/// la nivelul `tileZoom` (fracționar), 1 px de schemă = 1 px OSM.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapAnchor {
    pub lat: f64,
    pub lng: f64,
    pub world_x: f64,
    pub world_y: f64,
    pub tile_zoom: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LatLng {
    pub lat: f64,
    pub lng: f64,
}

/// Nivelul de zoom OSM la care un pixel de schemă (la scara dată) are mărimea unui pixel de dală.
pub fn compute_tile_zoom(lat: f64, scale_ratio: f64) -> f64 {
    let m_per_px = scale_ratio * PX_TO_MM / 1000.0;
    (EARTH_CIRCUMFERENCE * lat.to_radians().cos() / (TILE_SIZE * m_per_px)).log2().clamp(0.0, 19.0)
}

pub fn lng_to_osm_px(lng: f64, z: f64) -> f64 {
    (lng + 180.0) / 360.0 * TILE_SIZE * 2f64.powf(z)
}

pub fn lat_to_osm_py(lat: f64, z: f64) -> f64 {
    let s = lat.to_radians().sin();
    (0.5 - ((1.0 + s) / (1.0 - s)).ln() / (4.0 * PI)) * TILE_SIZE * 2f64.powf(z)
}

pub fn osm_px_to_lng(px: f64, z: f64) -> f64 {
    px / (TILE_SIZE * 2f64.powf(z)) * 360.0 - 180.0
}

pub fn osm_py_to_lat(py: f64, z: f64) -> f64 {
    let n = PI - 2.0 * PI * py / (TILE_SIZE * 2f64.powf(z));
    n.sinh().atan().to_degrees()
}

/// Punct din schemă → WGS84 (harta e rotită cu `rotation_deg` în jurul ancorei).
pub fn world_to_latlng(wx: f64, wy: f64, a: &MapAnchor, rotation_deg: f64) -> LatLng {
    let (s, c) = rotation_deg.to_radians().sin_cos();
    let (dx, dy) = (wx - a.world_x, wy - a.world_y);
    let z = a.tile_zoom;
    let px = lng_to_osm_px(a.lng, z) + dx * c + dy * s;
    let py = lat_to_osm_py(a.lat, z) - dx * s + dy * c;
    LatLng { lat: osm_py_to_lat(py, z), lng: osm_px_to_lng(px, z) }
}

/// WGS84 → punct din schemă (inversa lui [`world_to_latlng`]).
pub fn latlng_to_world(lat: f64, lng: f64, a: &MapAnchor, rotation_deg: f64) -> (f64, f64) {
    let (s, c) = rotation_deg.to_radians().sin_cos();
    let z = a.tile_zoom;
    let dx = lng_to_osm_px(lng, z) - lng_to_osm_px(a.lng, z);
    let dy = lat_to_osm_py(lat, z) - lat_to_osm_py(a.lat, z);
    (a.world_x + dx * c - dy * s, a.world_y + dx * s + dy * c)
}

// ─── Stereo 70 (EPSG:3844) → WGS84 ──────────────────────────────────────────
// +proj=sterea +lat_0=46 +lon_0=25 +k=0.99975 +x_0=500000 +y_0=500000 +ellps=krass
// +towgs84=2.329,-147.042,-92.08,0.309,-0.325,-0.497,5.69  (Position Vector)

struct Ellipsoid {
    a: f64,
    es: f64,
}

const KRASS: Ellipsoid = Ellipsoid { a: 6_378_245.0, es: 0.006_693_421_622_965_943 }; // f = 1/298.3
const WGS84: Ellipsoid = Ellipsoid { a: 6_378_137.0, es: 0.006_694_379_990_141_317 };

fn srat(esinp: f64, exp: f64) -> f64 {
    ((1.0 - esinp) / (1.0 + esinp)).powf(exp)
}

/// Proiecția stereografică dublă (sfera conformă Gauss), ca `sterea` din PROJ.
struct Sterea {
    e: f64,
    c: f64,
    k: f64,
    ratexp: f64,
    phic0: f64,
    sinc0: f64,
    cosc0: f64,
    r2: f64,
    lon0: f64,
    k0: f64,
    x0: f64,
    y0: f64,
    a: f64,
}

impl Sterea {
    fn new(ell: &Ellipsoid, lat0: f64, lon0: f64, k0: f64, x0: f64, y0: f64) -> Self {
        let es = ell.es;
        let e = es.sqrt();
        let phi0 = lat0.to_radians();
        let sphi = phi0.sin();
        let cphi2 = phi0.cos().powi(2);
        let rc = (1.0 - es).sqrt() / (1.0 - es * sphi * sphi);
        let c = (1.0 + es * cphi2 * cphi2 / (1.0 - es)).sqrt();
        let chi = (sphi / c).asin();
        let ratexp = 0.5 * c * e;
        let k = (0.5 * chi + FRAC_PI_4).tan() / ((0.5 * phi0 + FRAC_PI_4).tan().powf(c) * srat(e * sphi, ratexp));
        Self { e, c, k, ratexp, phic0: chi, sinc0: chi.sin(), cosc0: chi.cos(), r2: 2.0 * rc, lon0: lon0.to_radians(), k0, x0, y0, a: ell.a }
    }

    /// (Est, Nord) [m] → (lat, lon) [rad] pe elipsoidul proiecției.
    fn inverse(&self, east: f64, north: f64) -> (f64, f64) {
        let x = (east - self.x0) / (self.a * self.k0);
        let y = (north - self.y0) / (self.a * self.k0);
        let rho = x.hypot(y);
        let (phi, lam) = if rho != 0.0 {
            let cc = 2.0 * rho.atan2(self.r2);
            let (sinc, cosc) = cc.sin_cos();
            ((cosc * self.sinc0 + y * sinc * self.cosc0 / rho).asin(), (x * sinc).atan2(rho * self.cosc0 * cosc - y * self.sinc0 * sinc))
        } else {
            (self.phic0, 0.0)
        };
        // inversa sferei Gauss
        let lam = lam / self.c;
        let num = ((0.5 * phi + FRAC_PI_4).tan() / self.k).powf(1.0 / self.c);
        let mut p = phi;
        for _ in 0..20 {
            let next = 2.0 * (num * srat(self.e * p.sin(), -0.5 * self.e)).atan() - FRAC_PI_2;
            if (next - p).abs() < 1e-14 {
                p = next;
                break;
            }
            p = next;
        }
        let _ = self.ratexp;
        (p, lam + self.lon0)
    }
}

fn geodetic_to_ecef(ell: &Ellipsoid, lat: f64, lon: f64, h: f64) -> [f64; 3] {
    let (sl, cl) = lat.sin_cos();
    let n = ell.a / (1.0 - ell.es * sl * sl).sqrt();
    [(n + h) * cl * lon.cos(), (n + h) * cl * lon.sin(), (n * (1.0 - ell.es) + h) * sl]
}

fn ecef_to_geodetic(ell: &Ellipsoid, p: [f64; 3]) -> (f64, f64) {
    let [x, y, z] = p;
    let lon = y.atan2(x);
    let r = x.hypot(y);
    let mut lat = z.atan2(r * (1.0 - ell.es));
    for _ in 0..10 {
        let s = lat.sin();
        let n = ell.a / (1.0 - ell.es * s * s).sqrt();
        let h = r / lat.cos() - n;
        lat = z.atan2(r * (1.0 - ell.es * n / (n + h)));
    }
    (lat, lon)
}

/// Stereo 70: (Est, Nord) [m] → WGS84 (precizie ~1–3 m, parametrii Helmert EPSG).
pub fn stereo70_to_wgs84(east: f64, north: f64) -> LatLng {
    let proj = Sterea::new(&KRASS, 46.0, 25.0, 0.99975, 500_000.0, 500_000.0);
    let (lat, lon) = proj.inverse(east, north);
    let [x, y, z] = geodetic_to_ecef(&KRASS, lat, lon, 0.0);
    let sec = PI / 180.0 / 3600.0;
    let (dx, dy, dz) = (2.329, -147.042, -92.08);
    let (rx, ry, rz) = (0.309 * sec, -0.325 * sec, -0.497 * sec);
    let m = 1.0 + 5.69e-6;
    let p = [
        dx + m * (x - rz * y + ry * z),
        dy + m * (rz * x + y - rx * z),
        dz + m * (-ry * x + rx * y + z),
    ];
    let (lat, lon) = ecef_to_geodetic(&WGS84, p);
    LatLng { lat: lat.to_degrees(), lng: lon.to_degrees() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stereo70_matches_proj4js() {
        // Valori de referință calculate cu proj4js (aplicația React).
        for (e, n, lat, lng) in [
            (500000.0, 500000.0, 45.999954321, 24.998088108),
            (208000.0, 476000.0, 45.722138309, 21.245655883),
            (585000.0, 318000.0, 44.356980153, 26.064384875),
            (750000.0, 650000.0, 47.302426823, 28.304732629),
            (320000.0, 600000.0, 46.875534611, 22.636214649),
        ] {
            let p = stereo70_to_wgs84(e, n);
            assert!((p.lat - lat).abs() < 2e-7 && (p.lng - lng).abs() < 2e-7, "{e},{n}: {p:?} vs {lat},{lng}");
        }
    }

    #[test]
    fn world_latlng_roundtrip_with_rotation() {
        let a = MapAnchor { lat: 45.75, lng: 21.23, world_x: 100.0, world_y: -40.0, tile_zoom: compute_tile_zoom(45.75, 1000.0) };
        for rot in [0.0, 17.5, -90.0] {
            let ll = world_to_latlng(812.0, 377.0, &a, rot);
            let (x, y) = latlng_to_world(ll.lat, ll.lng, &a, rot);
            assert!((x - 812.0).abs() < 1e-6 && (y - 377.0).abs() < 1e-6);
        }
        // La scara 1:1000, 1 px de schemă ≈ 0.26458 m pe teren.
        let p0 = world_to_latlng(100.0, -40.0, &a, 0.0);
        let p1 = world_to_latlng(1100.0, -40.0, &a, 0.0);
        let m = (p1.lng - p0.lng).to_radians() * 6_371_000.0 * p0.lat.to_radians().cos();
        assert!((m - 264.58).abs() < 1.0, "{m}");
    }
}
