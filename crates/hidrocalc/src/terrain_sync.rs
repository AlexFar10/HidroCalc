//! Completarea automată a cotelor tronsoanelor din DEM și din punctele topo
//! (echivalentul hook-ului `useTerrainSync`): debounce 700 ms după ultima modificare,
//! descărcarea DEM pe un fir separat, aplicarea pe tronsoanele curente.

use crate::map_ui;
use hidro_core::elevation::{self, LatLng as ElevLatLng};
use hidro_core::geo::{latlng_to_world, world_to_latlng};
use hidro_core::model::ProjectData;
use hidro_core::schema::meters_per_px;
use hidro_core::terrain::{self, Target};
use std::hash::{Hash, Hasher};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

const DEBOUNCE: Duration = Duration::from_millis(700);

type DemResult = (Vec<Target>, Result<Vec<Option<f64>>, String>);

#[derive(Default)]
pub struct TerrainSync {
    key: Option<u64>,
    pending_since: Option<Instant>,
    inflight: Option<Receiver<DemResult>>,
    last_error: Option<String>,
}

impl TerrainSync {
    /// Apelat la fiecare cadru; întoarce un mesaj de eroare nou (de afișat o singură dată).
    pub fn tick(&mut self, ctx: &eframe::egui::Context, p: &mut ProjectData) -> Option<String> {
        let mut new_error = None;
        if let Some(rx) = &self.inflight {
            if let Ok((targets, dem)) = rx.try_recv() {
                self.inflight = None;
                let dem = match dem {
                    Ok(d) => {
                        self.last_error = None;
                        d
                    }
                    Err(e) => {
                        if self.last_error.as_ref() != Some(&e) {
                            self.last_error = Some(e.clone());
                            new_error = Some(e);
                        }
                        vec![None; targets.len()]
                    }
                };
                self.apply(p, &targets, &dem);
            }
        }

        let Some(anchor) = map_ui::anchor(p) else {
            self.key = None;
            return new_error;
        };
        let topo = map_ui::topo_points(p);
        let enabled = map_ui::auto_terrain(p);
        if topo.is_empty() && !enabled {
            return new_error;
        }
        let targets = terrain::collect_targets(&p.network_schema, &p.network_sections);
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for t in &targets {
            t.section_id.hash(&mut h);
            (t.end as u8).hash(&mut h);
            t.x.to_bits().hash(&mut h);
            t.y.to_bits().hash(&mut h);
        }
        for v in [anchor.lat, anchor.lng, anchor.world_x, anchor.world_y, map_ui::rotation(p), map_ui::topo_radius(p), crate::canvas::scale_ratio(p)] {
            v.to_bits().hash(&mut h);
        }
        enabled.hash(&mut h);
        // Conținutul punctelor, nu doar numărul lor: înlocuirea lor redeclanșează calculul.
        for t in topo {
            for v in [t.lat, t.lng, t.z] {
                v.to_bits().hash(&mut h);
            }
        }
        let key = h.finish();
        if self.key != Some(key) {
            self.key = Some(key);
            self.pending_since = (!targets.is_empty()).then(Instant::now);
        }
        if let Some(since) = self.pending_since {
            if since.elapsed() < DEBOUNCE || self.inflight.is_some() {
                ctx.request_repaint_after(DEBOUNCE);
                return new_error;
            }
            self.pending_since = None;
            if enabled {
                let rot = map_ui::rotation(p);
                let pts: Vec<ElevLatLng> = targets
                    .iter()
                    .map(|t| {
                        let ll = world_to_latlng(t.x, t.y, &anchor, rot);
                        ElevLatLng { lat: ll.lat, lng: ll.lng }
                    })
                    .collect();
                let (tx, rx) = channel();
                let ctx = ctx.clone();
                std::thread::spawn(move || {
                    let res = match crate::db::dem_cache_dir() {
                        Some(dir) => elevation::fetch_elevations(&pts, dir),
                        None => Err("Directorul de cache nu e disponibil".into()),
                    };
                    let _ = tx.send((targets, res));
                    ctx.request_repaint();
                });
                self.inflight = Some(rx);
            } else {
                let none = vec![None; targets.len()];
                self.apply(p, &targets, &none);
            }
        }
        new_error
    }

    fn apply(&mut self, p: &mut ProjectData, targets: &[Target], dem: &[Option<f64>]) {
        let Some(anchor) = map_ui::anchor(p) else { return };
        let rot = map_ui::rotation(p);
        let topo: Vec<(f64, f64, f64)> = map_ui::topo_points(p)
            .iter()
            .map(|t| {
                let (x, y) = latlng_to_world(t.lat, t.lng, &anchor, rot);
                (x, y, t.z)
            })
            .collect();
        let radius_px = map_ui::topo_radius(p) / meters_per_px(crate::canvas::scale_ratio(p));
        let values = terrain::resolve(targets, dem, &topo, radius_px);
        terrain::apply(&mut p.network_sections, targets, &values);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hidro_core::calculations::network::NetworkSection;
    use hidro_core::geo::{compute_tile_zoom, MapAnchor};
    use hidro_core::model::{SchemaElement, SchemaKind};
    use serde_json::json;

    #[test]
    fn topo_points_fill_eligible_ends_after_debounce() {
        let mut p = ProjectData::default();
        let a = MapAnchor { lat: 45.75, lng: 21.23, world_x: 0.0, world_y: 0.0, tile_zoom: compute_tile_zoom(45.75, 1000.0) };
        p.network_schema = vec![SchemaElement {
            kind: SchemaKind::Tronson,
            x: 0.0,
            y: 0.0,
            x2: Some(400.0),
            y2: Some(0.0),
            label: Some("Tr1".into()),
            ..Default::default()
        }];
        p.network_sections = vec![NetworkSection::new("s1".into(), "Tr1".into())];
        let end = world_to_latlng(401.0, 2.0, &a, 0.0); // ~0.6 m de capătul aval
        let o = &mut p.network_canvas_settings;
        o.map_anchor = Some(a);
        o.set("autoTerrainCota", json!(false));
        o.set("topoPoints", json!([{ "id": "t", "lat": end.lat, "lng": end.lng, "z": 93.27 }]));

        let ctx = eframe::egui::Context::default();
        let mut sync = TerrainSync::default();
        sync.tick(&ctx, &mut p);
        assert_eq!(p.network_sections[0].cota_down, None, "încă în debounce");
        std::thread::sleep(DEBOUNCE + Duration::from_millis(50));
        sync.tick(&ctx, &mut p);
        let s = &p.network_sections[0];
        assert_eq!((s.cota_down, s.cota_down_source.as_deref()), (Some(93.27), Some("topo")));
        assert_eq!(s.cota_up, None, "capătul amonte e departe de punct");
    }
}
