//! Modelul de date al unui proiect HidroCalc (`ProjectData`).
//!
//! Formatul JSON este identic cu cel al aplicației Tauri/React (chei camelCase),
//! deci proiectele exportate de acolo se pot importa aici și invers.
//!
//! Compatibilitate în ambele sensuri:
//! - câmpurile lipsă primesc valorile implicite (`#[serde(default)]`);
//! - câmpurile necunoscute se păstrează în `extra` și se scriu înapoi neschimbate,
//!   deci un proiect deschis aici nu pierde date pe care această versiune încă nu
//!   le editează (schema rețelei, setările canvas-ului etc.).

use crate::calculations::network::NetworkSection;
use crate::geo::MapAnchor;
use crate::scene3d::TerrainGrid;
use crate::topo::TopoPoint;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub type Extra = Map<String, Value>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectGeneral {
    pub name: String,
    pub locality: String,
    pub county: String,
    pub beneficiary: String,
    pub designer: String,
    pub date: String,
    pub water_source: String,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for ProjectGeneral {
    fn default() -> Self {
        Self {
            name: String::new(),
            locality: String::new(),
            county: String::new(),
            beneficiary: String::new(),
            designer: String::new(),
            date: String::new(),
            water_source: "conductă magistrală existentă".into(),
            extra: Extra::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PublicConsumer {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub custom_type: String,
    /// Nr. unități
    pub quantity: f64,
    /// Nr. consumatori (N_pi)
    pub num_consumers: f64,
    pub unit_type: String,
    pub q_specific: f64,
    pub hours_per_day: f64,
    pub kor: f64,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for PublicConsumer {
    fn default() -> Self {
        Self {
            id: String::new(),
            kind: "Primărie".into(),
            custom_type: String::new(),
            quantity: 0.0,
            num_consumers: 0.0,
            unit_type: "angajat".into(),
            q_specific: 30.0,
            hours_per_day: 8.0,
            kor: 2.4,
            extra: Extra::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConsumersData {
    pub inhabitants: f64,
    pub growth_rate: f64,
    pub perspective_years: f64,
    pub perspective_inhabitants: f64,
    pub q_gosp_per_capita: f64,
    pub public_consumers: Vec<PublicConsumer>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for ConsumersData {
    fn default() -> Self {
        Self { inhabitants: 0.0, growth_rate: 1.0, perspective_years: 20.0, perspective_inhabitants: 0.0, q_gosp_per_capita: 120.0, public_consumers: Vec::new(), extra: Extra::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WaterNeedConstants {
    pub kor_gosp: f64,
    pub kor_public_med: f64,
    pub kzi_gosp: f64,
    pub kzi_public: f64,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for WaterNeedConstants {
    fn default() -> Self {
        Self { kor_gosp: 2.64, kor_public_med: 2.0, kzi_gosp: 1.3, kzi_public: 1.3, extra: Extra::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WaterDemandCoefficients {
    pub kp: f64,
    pub kss: f64,
    pub ksr: f64,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for WaterDemandCoefficients {
    fn default() -> Self {
        Self { kp: 1.1, kss: 1.02, ksr: 1.03, extra: Extra::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HydrantData {
    // Hidranți exteriori
    pub debit_hydrant: f64,
    pub min_pressure: f64,
    pub operating_time: f64,
    pub simultaneous_fires: f64,
    pub coeff_a: f64,
    pub num_hydrants: f64,
    pub timp_refacere: f64,
    // Hidranți interiori
    pub debit_hydrant_int: f64,
    pub min_pressure_int: f64,
    pub operating_time_int: f64,
    pub simultaneous_fires_int: f64,
    pub num_hydrants_int: f64,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for HydrantData {
    fn default() -> Self {
        Self {
            debit_hydrant: 5.0,
            min_pressure: 0.7,
            operating_time: 2.0,
            simultaneous_fires: 1.0,
            coeff_a: 0.7,
            num_hydrants: 2.0,
            timp_refacere: 48.0,
            debit_hydrant_int: 2.5,
            min_pressure_int: 0.7,
            operating_time_int: 0.5,
            simultaneous_fires_int: 1.0,
            num_hydrants_int: 2.0,
            extra: Extra::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ReservoirData {
    pub cota_teren: f64,
    pub hb: f64,
    pub i_med: f64,
    pub l: f64,
    pub hr: f64,
    pub c: f64,
    pub coef_debit_min: f64,
    pub t_av: f64,
    pub ti: f64,
    pub qa: f64,
    pub amplasare: String,
    pub material: String,
    pub forma: String,
    pub pozitie: String,
    pub num_cuves: f64,
    pub dim_inaltime_totala: f64,
    pub dim_inaltime_rezervot: f64,
    pub dim_diametru_rezervot: f64,
    pub dim_capacitate_utila: f64,
    pub dim_capacitate_totala: f64,
    pub dim_platforma_betonata: f64,
    pub dim_latime_rezervot: f64,
    pub dim_lungime_rezervot: f64,
    /// Lungime platformă (paralelipipedic) — cheia JSON are „L” mare.
    #[serde(rename = "dimPlatformaBetonataL")]
    pub dim_platforma_betonata_lungime: f64,
    /// Lățime platformă (paralelipipedic) — cheia JSON are „l” mic.
    #[serde(rename = "dimPlatformaBetonatal")]
    pub dim_platforma_betonata_latime: f64,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for ReservoirData {
    fn default() -> Self {
        Self {
            cota_teren: 0.0,
            hb: 0.0,
            i_med: 0.0,
            l: 0.0,
            hr: 0.0,
            c: 0.15,
            coef_debit_min: 0.0,
            t_av: 0.0,
            ti: 0.0,
            qa: 0.0,
            amplasare: "suprateran".into(),
            material: "oțel".into(),
            forma: "cilindric".into(),
            pozitie: "de capăt".into(),
            num_cuves: 1.0,
            dim_inaltime_totala: 0.0,
            dim_inaltime_rezervot: 0.0,
            dim_diametru_rezervot: 0.0,
            dim_capacitate_utila: 0.0,
            dim_capacitate_totala: 0.0,
            dim_platforma_betonata: 0.0,
            dim_latime_rezervot: 0.0,
            dim_lungime_rezervot: 0.0,
            dim_platforma_betonata_lungime: 0.0,
            dim_platforma_betonata_latime: 0.0,
            extra: Extra::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NetworkConstants {
    pub roughness: f64,
    pub viscosity: f64,
    pub economic_velocity: f64,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for NetworkConstants {
    fn default() -> Self {
        Self { roughness: 0.0001, viscosity: 0.000_001_31, economic_velocity: 1.0, extra: Extra::new() }
    }
}

/// Tipul unui element din schema rețelei.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SchemaKind {
    #[default]
    Tronson,
    Rezervor,
    Sursa,
    Hidrant,
    CaminGrav,
    CaminAer,
    Text,
    Subsistem,
}

/// Element al schemei rețelei (identic cu `SchemaElement` din aplicația React).
///
/// Pentru tronson, (x, y) e capătul amonte și (x2, y2) cel aval; pentru celelalte
/// elemente (x, y) e centrul / punctul de conexiune. Coordonatele sunt în „pixeli
/// lume” ai canvas-ului (1 px = 0.26458 mm la scara desenului).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SchemaElement {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: SchemaKind,
    pub x: f64,
    pub y: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x2: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y2: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_offset_x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_offset_y: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub w: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub override_length: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<SchemaElement>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<Value>>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectData {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub current_step: u32,
    pub general: ProjectGeneral,
    pub consumers: ConsumersData,
    pub water_need_constants: WaterNeedConstants,
    pub water_demand_coefficients: WaterDemandCoefficients,
    pub hydrants: HydrantData,
    pub reservoir: ReservoirData,
    pub network_constants: NetworkConstants,
    /// Tronsoanele rețelei (pasul 8), cu rezultatele hidraulice.
    pub network_sections: Vec<NetworkSection>,
    /// Schema rețelei desenată pe canvas (pasul 8).
    pub network_schema: Vec<SchemaElement>,
    pub network_drawing: Value,
    #[serde(deserialize_with = "lenient")]
    pub network_canvas_settings: CanvasSettings,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for ProjectData {
    fn default() -> Self {
        Self {
            id: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
            current_step: 1,
            general: ProjectGeneral::default(),
            consumers: ConsumersData::default(),
            water_need_constants: WaterNeedConstants::default(),
            water_demand_coefficients: WaterDemandCoefficients::default(),
            hydrants: HydrantData::default(),
            reservoir: ReservoirData::default(),
            network_constants: NetworkConstants::default(),
            network_sections: Vec::new(),
            network_schema: Vec::new(),
            network_drawing: Value::Array(Vec::new()),
            network_canvas_settings: CanvasSettings::default(),
            extra: Extra::new(),
        }
    }
}

/// Setările implicite ale canvas-ului (identice cu `createDefaultProject()` din aplicația React).
fn default_canvas_settings() -> Extra {
    let v = serde_json::json!({
        "scaleRatio": 1000,
        "unit": "m",
        "gridVisible": false,
        "gridSpacing": 10,
        "snapToGrid": false,
        "snapToEndpoints": false,
        "layers": [{ "id": "default", "name": "Strat 1", "visible": true, "locked": false }],
        "activeLayerId": "default",
        "bgImageOpacity": 1,
        "showLegend": false,
        "showNorthArrow": false,
        "mapOpacity": 0.7,
        "mapRotation": 0,
        "mapLockPan": false,
        "mapLockZoom": false,
        "mapLockRotation": false
    });
    match v {
        Value::Object(m) => m,
        _ => Extra::new(),
    }
}

/// Citire tolerantă: o valoare cu formă neașteptată (ex. `null` dintr-un proiect vechi)
/// devine valoarea implicită, în loc să facă tot proiectul de necitit.
fn lenient<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned + Default,
{
    let v = Value::deserialize(d)?;
    Ok(serde_json::from_value(v).unwrap_or_default())
}

/// Setările canvas-ului (`networkCanvasSettings`).
///
/// Datele mari, citite la fiecare cadru — ancora hărții, punctele topo, grila de teren —
/// sunt câmpuri tipizate (fără deserializare la fiecare citire). Restul setărilor, mici,
/// rămân chei JSON (`get` / `set`), deci cele necunoscute se păstrează neschimbate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CanvasSettings {
    #[serde(deserialize_with = "lenient", skip_serializing_if = "Option::is_none")]
    pub map_anchor: Option<MapAnchor>,
    #[serde(deserialize_with = "lenient", skip_serializing_if = "Vec::is_empty")]
    pub topo_points: Vec<TopoPoint>,
    #[serde(deserialize_with = "lenient", skip_serializing_if = "Option::is_none")]
    pub terrain_grid: Option<TerrainGrid>,
    #[serde(flatten)]
    other: Extra,
}

impl Default for CanvasSettings {
    fn default() -> Self {
        Self { map_anchor: None, topo_points: Vec::new(), terrain_grid: None, other: default_canvas_settings() }
    }
}

impl CanvasSettings {
    /// O setare simplă (număr, bifă, obiect mic), după cheia ei JSON.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.other.get(key)
    }

    /// Scrie o setare după cheia ei JSON; `null` o șterge. Cheile datelor tipizate
    /// (`mapAnchor`, `topoPoints`, `terrainGrid`) ajung în câmpurile lor.
    pub fn set(&mut self, key: &str, v: Value) {
        match key {
            "mapAnchor" => self.map_anchor = serde_json::from_value(v).ok(),
            "topoPoints" => self.topo_points = serde_json::from_value(v).unwrap_or_default(),
            "terrainGrid" => self.terrain_grid = serde_json::from_value(v).ok(),
            _ if v.is_null() => {
                self.other.remove(key);
            }
            _ => {
                self.other.insert(key.into(), v);
            }
        }
    }
}

impl ProjectData {
    /// Proiect nou cu id, date calendaristice și valori implicite NP 133.
    pub fn new(id: String, now_iso: String, today: String) -> Self {
        let mut p = Self { id, created_at: now_iso.clone(), updated_at: now_iso, ..Self::default() };
        p.general.date = today;
        p
    }

    /// Lungimea totală a tronsoanelor definite la pasul 8 [m].
    pub fn total_network_length(&self) -> f64 {
        self.network_sections.iter().map(|s| s.length).sum()
    }

    /// Documentul de export JSON: metadate (format, versiunea aplicației, data) + proiectul întreg.
    /// Formatul are versiune, ca să putem evolua fișierele fără a le strica pe cele vechi.
    pub fn to_export_json(&self, app_version: &str, exported_at: &str) -> Value {
        serde_json::json!({
            "hidrocalc": { "format": EXPORT_FORMAT, "appVersion": app_version, "exportedAt": exported_at },
            "project": self,
        })
    }

    /// Citește un fișier exportat: documentul nou (cu metadate) sau fișierul vechi, în care
    /// proiectul era scris direct. Întoarce eroarea ca `ExportImportError`.
    pub fn from_export_value(v: Value) -> Result<ProjectData, ExportImportError> {
        let project = match v.get("hidrocalc") {
            Some(meta) => {
                let format = meta.get("format").and_then(Value::as_u64).unwrap_or(0);
                if format > EXPORT_FORMAT as u64 {
                    return Err(ExportImportError::NewerFormat(format));
                }
                v.get("project").cloned().ok_or(ExportImportError::Invalid)?
            }
            None => v,
        };
        if !Self::looks_valid(&project) {
            return Err(ExportImportError::Invalid);
        }
        serde_json::from_value(project).map_err(|_| ExportImportError::Invalid)
    }

    /// Validare a unui fișier importat (echivalentul `isValidProjectData` din aplicația React).
    pub fn looks_valid(v: &Value) -> bool {
        let Some(o) = v.as_object() else { return false };
        let obj = |k: &str| o.get(k).is_some_and(Value::is_object);
        let arr = |k: &str| o.get(k).is_some_and(Value::is_array);
        o.get("id").and_then(Value::as_str).is_some_and(|s| !s.is_empty())
            && ["general", "consumers", "waterNeedConstants", "waterDemandCoefficients", "hydrants", "reservoir"].iter().all(|k| obj(k))
            && arr("networkSections")
            && arr("networkSchema")
            && o["general"].get("name").is_some_and(Value::is_string)
            && o["consumers"].get("inhabitants").is_some_and(Value::is_number)
            && o["consumers"].get("publicConsumers").is_some_and(Value::is_array)
            && o["networkSections"].as_array().is_some_and(|a| a.iter().all(Value::is_object))
    }
}

/// Versiunea formatului de export JSON (se mărește când structura documentului se schimbă).
pub const EXPORT_FORMAT: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportImportError {
    /// Fișierul nu este un proiect HidroCalc valid.
    Invalid,
    /// Fișier făcut de o versiune mai nouă a aplicației (numărul formatului).
    NewerFormat(u64),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_fields_survive_roundtrip() {
        let src = serde_json::json!({
            "id": "x",
            "general": { "name": "Test", "viitor": 1 },
            "consumers": {}, "hydrants": {}, "reservoir": { "dimPlatformaBetonataL": 5, "dimPlatformaBetonatal": 3 },
            "networkSchema": [{ "id": "a", "type": "tronson" }],
            "campNou": { "a": true }
        });
        let p: ProjectData = serde_json::from_value(src).unwrap();
        assert_eq!(p.general.name, "Test");
        assert_eq!(p.reservoir.dim_platforma_betonata_lungime, 5.0);
        assert_eq!(p.reservoir.dim_platforma_betonata_latime, 3.0);
        assert_eq!(p.consumers.q_gosp_per_capita, 120.0); // implicit
        let back = serde_json::to_value(&p).unwrap();
        assert_eq!(back["general"]["viitor"], 1);
        assert_eq!(back["campNou"]["a"], true);
        assert_eq!(back["networkSchema"][0]["type"], "tronson");
        assert_eq!(back["reservoir"]["dimPlatformaBetonatal"], 3.0);
    }

    /// Setările canvasului: datele mari sunt tipizate, cheile necunoscute și cele simple se
    /// păstrează, iar valorile cu formă greșită nu strică citirea proiectului.
    #[test]
    fn canvas_settings_are_typed_lenient_and_lossless() {
        let src = serde_json::json!({
            "id": "x", "general": {}, "consumers": {}, "hydrants": {}, "reservoir": {},
            "networkCanvasSettings": {
                "scaleRatio": 500, "viitor": [1, 2],
                "mapAnchor": { "lat": 45.7, "lng": 21.2, "worldX": 1.0, "worldY": 2.0, "tileZoom": 16.5 },
                "topoPoints": [{ "id": "t", "lat": 45.7, "lng": 21.2, "z": 91.5 }],
                "terrainGrid": null
            }
        });
        let p: ProjectData = serde_json::from_value(src).unwrap();
        let cs = &p.network_canvas_settings;
        assert_eq!(cs.map_anchor.map(|a| a.tile_zoom), Some(16.5));
        assert_eq!((cs.topo_points.len(), cs.terrain_grid.is_none()), (1, true));
        assert_eq!(cs.get("scaleRatio").and_then(Value::as_f64), Some(500.0));
        let back = serde_json::to_value(&p).unwrap();
        assert_eq!(back["networkCanvasSettings"]["viitor"], serde_json::json!([1, 2]));
        assert_eq!(back["networkCanvasSettings"]["topoPoints"][0]["z"], 91.5);
        assert_eq!(serde_json::from_value::<ProjectData>(back).unwrap(), p);
        // Setări care nu sunt un obiect (proiect stricat): se revine la cele implicite.
        let broken = serde_json::json!({ "id": "y", "networkCanvasSettings": "x", "networkSchema": [] });
        let q: ProjectData = serde_json::from_value(broken).unwrap();
        assert_eq!(q.network_canvas_settings, CanvasSettings::default());
        let mut cs = CanvasSettings::default();
        cs.set("pipeDepth", serde_json::json!(1.4));
        cs.set("pipeDepth", Value::Null);
        assert!(cs.get("pipeDepth").is_none());
    }
}

#[cfg(test)]
mod export_tests {
    use super::*;

    fn sample() -> ProjectData {
        ProjectData::new("id-1".into(), "2026-11-10T10:00:00.000Z".into(), "2026-11-10".into())
    }

    #[test]
    fn export_document_roundtrips_and_has_metadata() {
        let p = sample();
        let doc = p.to_export_json("0.1.0", "2026-11-10T10:00:00Z");
        assert_eq!(doc["hidrocalc"]["format"], 1);
        assert_eq!(doc["hidrocalc"]["appVersion"], "0.1.0");
        assert_eq!(ProjectData::from_export_value(doc).unwrap(), p);
    }

    #[test]
    fn legacy_flat_file_still_imports() {
        let p = sample();
        let flat = serde_json::to_value(&p).unwrap();
        assert_eq!(ProjectData::from_export_value(flat).unwrap(), p);
    }

    #[test]
    fn newer_format_and_garbage_are_rejected() {
        let mut doc = sample().to_export_json("9.9.9", "x");
        doc["hidrocalc"]["format"] = serde_json::json!(99);
        assert_eq!(ProjectData::from_export_value(doc), Err(ExportImportError::NewerFormat(99)));
        assert_eq!(ProjectData::from_export_value(serde_json::json!({"a": 1})), Err(ExportImportError::Invalid));
        assert_eq!(ProjectData::from_export_value(serde_json::json!({"hidrocalc": {"format": 1}})), Err(ExportImportError::Invalid));
    }
}
