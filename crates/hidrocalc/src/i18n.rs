//! Traduceri RO/EN, din aceleași fișiere JSON ca aplicația React
//! (`assets/locales/{ro,en}.json`, chei cu puncte, interpolare `{{nume}}`).

use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum Lang {
    #[default]
    Ro,
    En,
}

/// Textele unei limbi, aplatizate la pornire: o căutare = o singură interogare de tabel.
#[derive(Default)]
struct Table {
    texts: HashMap<&'static str, &'static str>,
    lists: HashMap<&'static str, Vec<&'static str>>,
    /// Doar în engleză: textele de ajutor, indexate după textul românesc din cod.
    tips: HashMap<&'static str, &'static str>,
}

fn leak(s: &str) -> &'static str {
    Box::leak(s.to_owned().into_boxed_str())
}

impl Table {
    fn parse(json: &str) -> Self {
        let root: Value = serde_json::from_str(json).expect("fișier de traduceri invalid");
        let mut t = Table::default();
        if let Some(Value::Object(tips)) = root.get("tips") {
            t.tips = tips.iter().filter_map(|(k, v)| Some((leak(k), leak(v.as_str()?)))).collect();
        }
        t.walk("", &root);
        t
    }

    fn walk(&mut self, path: &str, v: &Value) {
        match v {
            Value::String(s) => {
                self.texts.insert(leak(path), leak(s));
            }
            Value::Array(a) => {
                self.lists.insert(leak(path), a.iter().filter_map(Value::as_str).map(leak).collect());
                for (i, item) in a.iter().enumerate() {
                    self.walk(&format!("{path}.{i}"), item);
                }
            }
            Value::Object(o) if path != "tips" => {
                for (k, item) in o {
                    let child = if path.is_empty() { k.clone() } else { format!("{path}.{k}") };
                    self.walk(&child, item);
                }
            }
            _ => {}
        }
    }
}

/// Tabelele celor două limbi, construite o singură dată (textele trăiesc cât aplicația).
fn tables() -> &'static (Table, Table) {
    static TABLES: OnceLock<(Table, Table)> = OnceLock::new();
    TABLES.get_or_init(|| (Table::parse(include_str!("../../../assets/locales/ro.json")), Table::parse(include_str!("../../../assets/locales/en.json"))))
}

/// Cheile fără traducere se afișează ca atare; fiecare se păstrează o singură dată.
fn missing_key(key: &str) -> &'static str {
    static MISSING: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let mut set = MISSING.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    match set.get(key) {
        Some(k) => k,
        None => {
            let k = leak(key);
            set.insert(k);
            k
        }
    }
}

pub struct I18n {
    pub lang: Lang,
}

impl I18n {
    pub fn new(lang: Lang) -> Self {
        tables();
        Self { lang }
    }

    /// Textul tradus pentru `key`, fără alocare; dacă lipsește, cheia însăși (ușor de observat).
    pub fn t(&self, key: &str) -> &'static str {
        let (ro, en) = tables();
        let primary = if self.lang == Lang::En { en } else { ro };
        primary.texts.get(key).or_else(|| ro.texts.get(key)).copied().unwrap_or_else(|| missing_key(key))
    }

    /// Textul de ajutor (tooltip): scris în română în cod; în engleză, tradus din `tips`.
    pub fn tip<'a>(&self, ro: &'a str) -> &'a str {
        if self.lang == Lang::En {
            if let Some(s) = tables().1.tips.get(ro) {
                return s;
            }
        }
        ro
    }

    /// Ca [`Self::t`], cu înlocuirea `{{nume}}` → valoare.
    pub fn tf(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut s = self.t(key).to_string();
        for (k, v) in args {
            s = s.replace(&format!("{{{{{k}}}}}"), v);
        }
        s
    }

    /// O listă de texte (ex. `steps.names`).
    pub fn list(&self, key: &str) -> &'static [&'static str] {
        let (ro, en) = tables();
        let primary = if self.lang == Lang::En { en } else { ro };
        primary.lists.get(key).or_else(|| ro.lists.get(key)).map_or(&[], Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_and_interpolation() {
        let i = I18n::new(Lang::Ro);
        assert_eq!(i.list("steps.names").len(), 9);
        assert!(i.tf("step7.crWarning", &[("diff", "3.00")]).contains("3.00"));
        assert_eq!(i.t("nu.exista"), "nu.exista");
        let en = I18n::new(Lang::En);
        assert_ne!(en.t("home.newProject"), i.t("home.newProject"));
    }

    /// Orice text de ajutor din cod trebuie să aibă traducere în engleză.
    #[test]
    fn every_tip_has_an_english_translation() {
        let en = I18n::new(Lang::En);
        let sources = [include_str!("steps.rs"), include_str!("network_ui.rs")];
        let mut n = 0;
        for src in sources {
            for part in src.split("t.tip(\"").skip(1) {
                let ro = part.split("\")").next().unwrap();
                assert_ne!(en.tip(ro), ro, "lipsește traducerea EN pentru: {ro}");
                n += 1;
            }
        }
        assert!(n >= 30, "au fost găsite doar {n} texte de ajutor");
    }
}
