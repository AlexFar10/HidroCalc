//! Logica de domeniu HidroCalc, independentă de interfață.
//!
//! - [`model`] — `ProjectData`, compatibil JSON cu aplicația React/Tauri
//! - [`calculations`] — formulele NP 133 / PM 8/2-2013 (pașii 3–8)
//! - [`pipeline`] — lanțul de calcul al pașilor 3–7 direct pe `ProjectData`
//! - [`schema`] — schema rețelei: scară, import topologie tronsoane
//! - [`defaults`] — valori implicite normative
//! - [`export`] — date pentru documente, cantități, DXF, XLSX
//! - [`geo`] — Web Mercator (dale OSM), ancora hărții, Stereo 70 → WGS84
//! - [`topo`] — import puncte topo (CSV/GPX/KML, Stereo 70)
//! - [`terrain`] — completarea automată a cotelor (DEM + topo)
//! - [`elevation`] — cote de teren din DEM (dale Terrarium/SRTM, cu cache pe disc)

pub mod calculations;
pub mod defaults;
pub mod elevation;
pub mod export;
pub mod geo;
pub mod model;
pub mod pipeline;
pub mod scene3d;
pub mod schema;
pub mod terrain;
pub mod topo;
