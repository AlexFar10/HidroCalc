//! Registrul implicit „Liste de cantități” (fără șablon), ca `buildDefaultWorkbook`.

use super::Tr;
use rust_xlsxwriter::{Color, Format, FormatAlign, Workbook, XlsxError};
use serde_json::Value;

struct Col<'a> {
    header: String,
    key: &'a str,
    width: f64,
    number: bool,
}

fn sheet(wb: &mut Workbook, name: &str, title: Option<&str>, cols: &[Col], rows: &[Value]) -> Result<(), XlsxError> {
    let ws = wb.add_worksheet();
    ws.set_name(name)?;
    let head = Format::new().set_bold().set_font_color(Color::White).set_background_color(Color::RGB(0x0E7490)).set_text_wrap().set_align(FormatAlign::VerticalCenter);
    let num = Format::new().set_num_format("#,##0.00");
    let mut r0 = 0;
    if let Some(t) = title {
        ws.write_string_with_format(0, 0, t, &Format::new().set_bold().set_font_size(13))?;
        r0 = 1;
    }
    for (c, col) in cols.iter().enumerate() {
        ws.set_column_width(c as u16, col.width)?;
        ws.write_string_with_format(r0, c as u16, &col.header, &head)?;
    }
    for (i, row) in rows.iter().enumerate() {
        let r = r0 + 1 + i as u32;
        for (c, col) in cols.iter().enumerate() {
            match row.get(col.key) {
                Some(Value::Number(n)) => {
                    let v = n.as_f64().unwrap_or(0.0);
                    if col.number {
                        ws.write_number_with_format(r, c as u16, v, &num)?;
                    } else {
                        ws.write_number(r, c as u16, v)?;
                    }
                }
                Some(Value::String(s)) => {
                    ws.write_string(r, c as u16, s)?;
                }
                Some(Value::Bool(b)) => {
                    ws.write_boolean(r, c as u16, *b)?;
                }
                _ => {}
            }
        }
    }
    ws.set_freeze_panes(r0 + 1, 0)?;
    Ok(())
}

/// `data` = rezultatul lui [`super::document::build`].
pub fn default_workbook(data: &Value, t: Tr) -> Result<Vec<u8>, XlsxError> {
    let tt = |k: &str| t(k, &[]);
    let arr = |v: Option<&Value>| v.and_then(Value::as_array).cloned().unwrap_or_default();
    let mut wb = Workbook::new();
    let p = &data["proiect"];
    let title = format!(
        "{} — {}, jud. {}",
        p["nume"].as_str().unwrap_or(""),
        p["localitate"].as_str().unwrap_or(""),
        p["judet"].as_str().unwrap_or("")
    );
    let c = |h: String, key, width, number| Col { header: h, key, width, number };
    sheet(
        &mut wb,
        &tt("export.xlsx.sheetQuantities"),
        Some(&title),
        &[
            c(tt("export.xlsx.colNr"), "nr", 6.0, false),
            c(tt("export.xlsx.colCategory"), "categorie", 22.0, false),
            c(tt("export.xlsx.colItem"), "denumire", 60.0, false),
            c(tt("export.xlsx.colUm"), "um", 8.0, false),
            c(tt("export.xlsx.colQty"), "cantitate", 14.0, true),
            c(tt("export.xlsx.colNotes"), "observatii", 40.0, false),
        ],
        &arr(data.get("cantitati")),
    )?;
    sheet(
        &mut wb,
        &tt("export.xlsx.sheetDiameters"),
        None,
        &[
            c("De [mm]".into(), "de", 10.0, false),
            c("Di [mm]".into(), "di", 10.0, false),
            c("L [m]".into(), "lungime", 12.0, true),
            c(tt("export.xlsx.colSections"), "tronsoane", 60.0, false),
        ],
        &arr(data["retea"].get("diametre")),
    )?;
    sheet(
        &mut wb,
        &tt("export.xlsx.sheetSections"),
        None,
        &[
            c(tt("export.xlsx.colNr"), "nr", 6.0, false),
            c(tt("export.xlsx.colSection"), "nume", 12.0, false),
            c(tt("export.xlsx.colUpstream"), "amonte", 12.0, false),
            c("L [m]".into(), "lungime", 10.0, false),
            c("Qc [l/s]".into(), "q_c", 10.0, false),
            c("De [mm]".into(), "de", 9.0, false),
            c("Di [mm]".into(), "di", 9.0, false),
            c("v [m/s]".into(), "v", 9.0, false),
            c("h_tot [m]".into(), "h_tot", 11.0, false),
            c(tt("export.xlsx.colGroundUp"), "cota_amonte", 12.0, false),
            c(tt("export.xlsx.colGroundDown"), "cota_aval", 12.0, false),
        ],
        &arr(data["retea"].get("tronsoane")),
    )?;
    wb.save_to_buffer()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calculations::network::NetworkSection;
    use crate::model::ProjectData;
    use crate::pipeline;

    #[test]
    fn workbook_from_sample_project() {
        let mut p = ProjectData::default();
        p.general.name = "Test".into();
        p.consumers.inhabitants = 500.0;
        let mut a = NetworkSection::new("a".into(), "Tr1".into());
        a.length = 120.0;
        a.q_linear = 0.01;
        p.network_sections = vec![a];
        p.network_sections = pipeline::compute_network(&p).unwrap();
        let res = pipeline::compute(&p);
        let t = |k: &str, _: &[(&str, &str)]| k.to_string();
        let data = crate::export::document::build(&p, &res, &p.network_sections, "05.11.2026", &t);
        assert_eq!(data["retea"]["nr_tronsoane"], 1);
        assert!(data["cantitati"].as_array().unwrap().len() >= 5);
        assert_eq!(data["terasamente"]["latimeMedie"], 0.6);
        let bytes = default_workbook(&data, &t).unwrap();
        assert_eq!(&bytes[..2], b"PK"); // arhivă zip validă
    }
}
