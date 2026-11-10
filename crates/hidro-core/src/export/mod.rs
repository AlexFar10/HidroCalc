//! Exporturi independente de interfață (deocamdată: DXF). Documentele scrise
//! (memoriu, liste de cantități, breviar) au fost scoase și urmează să fie refăcute.

pub mod dxf;

/// Nume de fișier sigur (fără caracterele interzise pe Windows/macOS).
pub fn safe_file_name(name: &str, fallback: &str) -> String {
    let n = if name.trim().is_empty() { fallback } else { name };
    n.chars().map(|c| if r#"/\:*?"<>|"#.contains(c) { '_' } else { c }).collect()
}

/// Adâncimea de pozare a conductelor (m), din setările canvasului; implicit `DEFAULT_PIPE_DEPTH`.
pub fn pipe_depth(p: &crate::model::ProjectData) -> f64 {
    p.network_canvas_settings
        .get("pipeDepth")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(crate::defaults::DEFAULT_PIPE_DEPTH)
}
