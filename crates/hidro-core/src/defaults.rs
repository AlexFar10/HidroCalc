//! Valori implicite din normativele românești (NP 133).

/// Adâncimea implicită de pozare a conductelor (sub adâncimea de îngheț, NP 133) [m].
pub const DEFAULT_PIPE_DEPTH: f64 = 1.1;

#[derive(Debug, Clone, Copy)]
pub struct InstitutionDefault {
    pub kind: &'static str,
    pub unit_type: &'static str,
    /// l/unitate·zi
    pub q_specific: f64,
    pub hours_per_day: f64,
    pub kor: f64,
}

const fn inst(kind: &'static str, unit_type: &'static str, q_specific: f64, hours_per_day: f64) -> InstitutionDefault {
    InstitutionDefault { kind, unit_type, q_specific, hours_per_day, kor: 2.4 }
}

pub const INSTITUTION_DEFAULTS: &[InstitutionDefault] = &[
    inst("Primărie", "angajați", 30.0, 8.0),
    inst("Școală gimnazială", "elevi", 50.0, 8.0),
    inst("Liceu", "elevi", 50.0, 8.0),
    inst("Grădiniță", "copii", 75.0, 8.0),
    inst("Café-bar", "consumatori", 15.0, 12.0),
    inst("Bar", "consumatori", 15.0, 12.0),
    inst("Farmacie", "angajați", 30.0, 8.0),
    inst("Biserică", "enoriași", 5.0, 4.0),
    inst("Magazin alimentar", "angajați", 30.0, 10.0),
    inst("Service auto", "auto", 250.0, 8.0),
    inst("Cămin cultural", "scaune", 10.0, 6.0),
    inst("Hotel", "paturi", 200.0, 24.0),
    inst("Restaurant", "scaune", 50.0, 12.0),
    inst("Spital", "paturi", 300.0, 24.0),
    inst("Altele", "unități", 30.0, 8.0),
];

pub fn institution_default(kind: &str) -> Option<&'static InstitutionDefault> {
    INSTITUTION_DEFAULTS.iter().find(|i| i.kind == kind)
}

/// Tipuri de consumatori sugerate în tabelul consumatorilor publici.
pub const UNIT_TYPE_PRESETS: &[&str] = &["angajat", "angajați", "elevi", "copii", "consumatori", "clienți", "scaune", "paturi", "auto", "enoriași", "unități", "loc", "cap animal"];
