//! Traduceri RO/EN, din aceleași fișiere JSON ca aplicația React
//! (`assets/locales/{ro,en}.json`, chei cu puncte, interpolare `{{nume}}`).

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum Lang {
    #[default]
    Ro,
    En,
}

pub struct I18n {
    ro: Value,
    en: Value,
    pub lang: Lang,
}

impl I18n {
    pub fn new(lang: Lang) -> Self {
        let parse = |s: &str| serde_json::from_str(s).expect("fișier de traduceri invalid");
        Self {
            ro: parse(include_str!("../../../assets/locales/ro.json")),
            en: parse(include_str!("../../../assets/locales/en.json")),
            lang,
        }
    }

    fn lookup(&self, key: &str) -> Option<&Value> {
        let primary = if self.lang == Lang::En { &self.en } else { &self.ro };
        find(primary, key).or_else(|| find(&self.ro, key))
    }

    /// Textul tradus pentru `key`; dacă lipsește, cheia însăși (ușor de observat).
    pub fn t(&self, key: &str) -> String {
        match self.lookup(key) {
            Some(Value::String(s)) => s.clone(),
            _ => key.to_string(),
        }
    }

    /// Textul de ajutor (tooltip): scris în română în cod; în engleză, tradus din `tips`.
    pub fn tip(&self, ro: &str) -> String {
        if self.lang == Lang::En {
            if let Some(Value::String(s)) = self.en.get("tips").and_then(|t| t.get(ro)) {
                return s.clone();
            }
        }
        ro.to_string()
    }

    /// Ca [`Self::t`], cu înlocuirea `{{nume}}` → valoare.
    pub fn tf(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut s = self.t(key);
        for (k, v) in args {
            s = s.replace(&format!("{{{{{k}}}}}"), v);
        }
        s
    }

    /// O listă de texte (ex. `steps.names`).
    pub fn list(&self, key: &str) -> Vec<String> {
        match self.lookup(key) {
            Some(Value::Array(a)) => a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect(),
            _ => Vec::new(),
        }
    }
}

fn find<'a>(root: &'a Value, key: &str) -> Option<&'a Value> {
    key.split('.').try_fold(root, |v, part| match v {
        Value::Array(a) => part.parse::<usize>().ok().and_then(|i| a.get(i)),
        _ => v.get(part),
    })
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
