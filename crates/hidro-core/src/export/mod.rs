//! Exporturi independente de interfață: date pentru documente, cantități, DXF, XLSX.
//!
//! Textele traductibile se cer printr-un [`Tr`] (cheie + parametri), ca modulul
//! să nu depindă de sistemul de traduceri al aplicației.

pub mod breviar;
pub mod document;
pub mod docx;
pub mod dxf;
pub mod pdf;
pub mod quantities;
pub mod report;
pub mod schema_pdf;
pub mod xlsx;
pub mod xlsx_template;

/// Funcție de traducere: `(cheie, [(parametru, valoare)])` → text.
pub type Tr<'a> = &'a dyn Fn(&str, &[(&str, &str)]) -> String;

/// Nume de fișier sigur (fără caracterele interzise pe Windows/macOS).
pub fn safe_file_name(name: &str, fallback: &str) -> String {
    let n = if name.trim().is_empty() { fallback } else { name };
    n.chars().map(|c| if r#"/\:*?"<>|"#.contains(c) { '_' } else { c }).collect()
}

pub(crate) fn round(v: f64, d: i32) -> f64 {
    let m = 10f64.powi(d);
    (v * m).round() / m
}
