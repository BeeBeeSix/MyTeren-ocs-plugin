//! Styly a výjimky.
//!
//! Styl je pojmenovaná sada hodnot (klíč → text). Může být **odvozený**
//! z jiného stylu (`rodic`) – pak nese jen to, co je jinak. Objekt odkazuje
//! na styl a může mít **výjimky**: vlastní hodnoty jen pro sebe. Výsledná
//! hodnota = výjimka objektu, jinak styl, jinak jeho rodič…, jinak výchozí
//! hodnota ze schématu. Tenhle model platí pro všechny objekty pluginu
//! (trasy, profily, pohledy, pásy) od první verze, i když UI přijde později.

use std::collections::BTreeMap;

/// Jeden klíč schématu stylu.
#[derive(Clone, Copy, Debug)]
pub struct Klic {
    pub klic: &'static str,
    pub vychozi: &'static str,
    /// Český popis (překládá plugin).
    pub popis: &'static str,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Styl {
    pub id: String,
    pub nazev: String,
    pub rodic: Option<String>,
    pub hodnoty: BTreeMap<String, String>,
}

impl Styl {
    pub fn novy(id: &str, nazev: &str) -> Self {
        Self { id: id.into(), nazev: nazev.into(), ..Default::default() }
    }
    pub fn odvozeny(id: &str, nazev: &str, rodic: &str, hodnoty: &[(&str, &str)]) -> Self {
        Self {
            id: id.into(),
            nazev: nazev.into(),
            rodic: Some(rodic.into()),
            hodnoty: hodnoty.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        }
    }
}

/// Výsledné hodnoty pro jeden objekt.
#[derive(Clone, Debug)]
pub struct Hodnoty<'a> {
    schema: &'a [Klic],
    mapa: BTreeMap<String, String>,
    /// Klíče, které objekt mění výjimkou.
    pub vyjimky: Vec<String>,
}

/// Složí hodnoty: schéma ← řetěz rodičů ← styl ← výjimky objektu.
pub fn vysledne<'a>(schema: &'a [Klic], styly: &[Styl], styl: &str, vyjimky: &BTreeMap<String, String>) -> Hodnoty<'a> {
    let mut retez = Vec::new();
    let mut akt = Some(styl.to_string());
    while let Some(id) = akt {
        if retez.iter().any(|s: &&Styl| s.id == id) || retez.len() > 32 {
            break; // cyklus v dědění
        }
        match styly.iter().find(|s| s.id == id) {
            Some(s) => {
                retez.push(s);
                akt = s.rodic.clone();
            }
            None => break,
        }
    }
    let mut mapa: BTreeMap<String, String> = schema.iter().map(|k| (k.klic.to_string(), k.vychozi.to_string())).collect();
    for s in retez.iter().rev() {
        for (k, v) in &s.hodnoty {
            mapa.insert(k.clone(), v.clone());
        }
    }
    let mut seznam = Vec::new();
    for (k, v) in vyjimky {
        if mapa.get(k) != Some(v) {
            seznam.push(k.clone());
        }
        mapa.insert(k.clone(), v.clone());
    }
    Hodnoty { schema, mapa, vyjimky: seznam }
}

impl Hodnoty<'_> {
    pub fn text(&self, k: &str) -> &str {
        self.mapa.get(k).map(String::as_str).unwrap_or_else(|| {
            self.schema.iter().find(|x| x.klic == k).map_or("", |x| x.vychozi)
        })
    }
    pub fn cislo(&self, k: &str) -> f64 {
        let t = self.text(k).replace(',', ".");
        t.trim().parse().unwrap_or_else(|_| {
            self.schema.iter().find(|x| x.klic == k).and_then(|x| x.vychozi.parse().ok()).unwrap_or(0.0)
        })
    }
    pub fn ano(&self, k: &str) -> bool {
        matches!(self.text(k).trim(), "1" | "ano" | "true")
    }
}

#[cfg(test)]
mod testy {
    use super::*;

    const S: [Klic; 2] = [
        Klic { klic: "meritko", vychozi: "500", popis: "" },
        Klic { klic: "krok", vychozi: "20", popis: "" },
    ];

    #[test]
    fn dedeni_a_vyjimky() {
        let styly = vec![
            Styl { hodnoty: [("krok".to_string(), "50".to_string())].into(), ..Styl::novy("A", "A") },
            Styl::odvozeny("B", "B", "A", &[("meritko", "1000")]),
        ];
        let h = vysledne(&S, &styly, "B", &BTreeMap::new());
        assert_eq!(h.cislo("meritko"), 1000.0);
        assert_eq!(h.cislo("krok"), 50.0);
        assert!(h.vyjimky.is_empty());
        let v: BTreeMap<String, String> = [("krok".to_string(), "10".to_string())].into();
        let h = vysledne(&S, &styly, "B", &v);
        assert_eq!(h.cislo("krok"), 10.0);
        assert_eq!(h.vyjimky, vec!["krok".to_string()]);
    }

    #[test]
    fn cyklus_a_neznamy_styl() {
        let styly = vec![Styl::odvozeny("A", "A", "B", &[]), Styl::odvozeny("B", "B", "A", &[("krok", "5")])];
        assert_eq!(vysledne(&S, &styly, "A", &BTreeMap::new()).cislo("krok"), 5.0);
        assert_eq!(vysledne(&S, &styly, "X", &BTreeMap::new()).cislo("meritko"), 500.0);
    }
}
