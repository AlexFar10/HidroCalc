//! Exporturi independente de interfață: DXF și Memoriu tehnic (.docx, din șablon încorporat).
//! Textele traductibile se cer printr-un [`Tr`], ca modulul să nu depindă de aplicație.

pub mod document;
pub mod docx;
pub mod dxf;
pub mod quantities;

/// Șablonul Memoriului tehnic (Word), încorporat în aplicație. Se reconstruiește cu
/// `python3 tools/build_memoriu_template.py`.
pub const MEMORIU_TEMPLATE: &[u8] = include_bytes!("../../../../assets/templates/memoriu-tehnic.docx");

/// Funcție de traducere: `(cheie, [(parametru, valoare)])` → text.
pub type Tr<'a> = &'a dyn Fn(&str, &[(&str, &str)]) -> String;

pub(crate) fn round(v: f64, d: i32) -> f64 {
    let m = 10f64.powi(d);
    (v * m).round() / m
}

/// Nume de fișier sigur (fără caracterele interzise pe Windows/macOS).
pub fn safe_file_name(name: &str, fallback: &str) -> String {
    let n = if name.trim().is_empty() { fallback } else { name };
    n.chars().map(|c| if r#"/\:*?"<>|"#.contains(c) { '_' } else { c }).collect()
}

/// Adâncimea de pozare a conductelor (m), din setările canvasului; implicit `DEFAULT_PIPE_DEPTH`.
pub fn pipe_depth(p: &crate::model::ProjectData) -> f64 {
    p.network_canvas_settings.get("pipeDepth").and_then(serde_json::Value::as_f64).unwrap_or(crate::defaults::DEFAULT_PIPE_DEPTH)
}
