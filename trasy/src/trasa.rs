//! Definice trasy, jak se ukládá do výkresu, a styly tras.

use std::collections::BTreeMap;

use crate::geometrie::Vrchol;
use crate::niveleta::Niveleta;
use crate::styl::{Klic, Styl};

/// Verze zápisu definice.
pub const VERZE: &str = "TRASY1";

#[derive(Clone, Debug, PartialEq)]
pub struct DefiniceTrasy {
    /// Identifikátor bez diakritiky (část názvu bloku).
    pub id: String,
    pub jmeno: String,
    /// Počáteční staničení [m].
    pub pocatek: f64,
    pub styl: String,
    /// Výjimky objektu proti stylu.
    pub vyjimky: BTreeMap<String, String>,
    /// Vrcholy tečnového polygonu v souřadnicích bloku.
    pub vrcholy: Vec<Vrchol>,
    /// Základní bod bloku (vložení s nulovým posunem a otočením leží tady).
    pub zaklad: [f64; 2],
    /// Rovnice staničení (staničení zpět, staničení vpřed). Zatím se jen
    /// ukládá – výpočet a UI přijdou později.
    pub rovnice: Vec<(f64, f64)>,
    /// Nivelety (návrhové profily) trasy.
    pub nivelety: Vec<Niveleta>,
}

fn f(v: f64) -> String {
    // dost míst, aby se poloha neposouvala opakovaným ukládáním
    let t = format!("{v:.6}");
    let t = t.trim_end_matches('0').trim_end_matches('.');
    if t.is_empty() || t == "-" { "0".into() } else { t.into() }
}

fn cti(t: &str) -> Option<f64> {
    t.trim().replace(',', ".").parse().ok()
}

impl DefiniceTrasy {
    pub fn nova(id: &str, jmeno: &str, vrcholy: Vec<Vrchol>, styl: &str) -> Self {
        let zaklad = vrcholy.first().map_or([0.0, 0.0], |v| [v.x, v.y]);
        Self {
            id: id.into(),
            jmeno: jmeno.into(),
            pocatek: 0.0,
            styl: styl.into(),
            vyjimky: BTreeMap::new(),
            vrcholy,
            zaklad,
            rovnice: Vec::new(),
            nivelety: Vec::new(),
        }
    }

    /// Řádky pro uložení (každý kratší než 255 znaků – limit XDATA).
    pub fn zapis(&self) -> Vec<String> {
        let mut r = vec![VERZE.to_string()];
        r.push(format!("id={}", self.id));
        r.push(format!("jmeno={}", self.jmeno.chars().take(200).collect::<String>()));
        r.push(format!("pocatek={}", f(self.pocatek)));
        r.push(format!("styl={}", self.styl));
        r.push(format!("zaklad={};{}", f(self.zaklad[0]), f(self.zaklad[1])));
        for v in &self.vrcholy {
            r.push(format!("v={};{};{}", f(v.x), f(v.y), f(v.r)));
        }
        for (a, b) in &self.rovnice {
            r.push(format!("rovnice={};{}", f(*a), f(*b)));
        }
        for (k, v) in &self.vyjimky {
            r.push(format!("x.{k}={}", v.chars().take(200).collect::<String>()));
        }
        for n in &self.nivelety {
            r.extend(n.zapis());
        }
        r
    }

    /// Načte definici z řádků; `None`, když to není definice trasy.
    pub fn cti(radky: &[&str]) -> Option<Self> {
        if radky.first().copied() != Some(VERZE) {
            return None;
        }
        let mut d = DefiniceTrasy::nova("", "", Vec::new(), "");
        let mut zaklad = None;
        for r in &radky[1..] {
            let Some((k, h)) = r.split_once('=') else { continue };
            match k {
                "id" => d.id = h.into(),
                "jmeno" => d.jmeno = h.into(),
                "pocatek" => d.pocatek = cti(h).unwrap_or(0.0),
                "styl" => d.styl = h.into(),
                "zaklad" => {
                    let c: Vec<f64> = h.split(';').filter_map(cti).collect();
                    if c.len() == 2 {
                        zaklad = Some([c[0], c[1]]);
                    }
                }
                "v" => {
                    let c: Vec<f64> = h.split(';').filter_map(cti).collect();
                    if c.len() >= 2 {
                        d.vrcholy.push(Vrchol::new(c[0], c[1], c.get(2).copied().unwrap_or(0.0)));
                    }
                }
                "rovnice" => {
                    let c: Vec<f64> = h.split(';').filter_map(cti).collect();
                    if c.len() == 2 {
                        d.rovnice.push((c[0], c[1]));
                    }
                }
                k if k.starts_with("x.") => {
                    d.vyjimky.insert(k[2..].to_string(), h.to_string());
                }
                _ => {} // neznámé klíče novějších verzí se přeskočí
            }
        }
        if d.id.is_empty() || d.vrcholy.len() < 2 {
            return None;
        }
        d.nivelety = Niveleta::cti_vse(&radky[1..]);
        d.zaklad = zaklad.unwrap_or([d.vrcholy[0].x, d.vrcholy[0].y]);
        Some(d)
    }

    /// Vrcholy ve světových souřadnicích při daném vložení bloku
    /// (bod vložení, otočení; měřítko vložení se neuvažuje).
    pub fn svetove(&self, vlozeni: [f64; 2], otoceni: f64) -> Vec<Vrchol> {
        let (s, c) = otoceni.sin_cos();
        self.vrcholy
            .iter()
            .map(|v| {
                let (dx, dy) = (v.x - self.zaklad[0], v.y - self.zaklad[1]);
                Vrchol::new(vlozeni[0] + dx * c - dy * s, vlozeni[1] + dx * s + dy * c, v.r)
            })
            .collect()
    }
}

// ── Styl trasy ──────────────────────────────────────────────────────────────

/// Klíče stylu trasy (výchozí hodnoty = základní styl).
pub const SCHEMA: &[Klic] = &[
    Klic { klic: "meritko", vychozi: "500", popis: "Měřítko výkresu 1:" },
    Klic { klic: "vyska_textu", vychozi: "2", popis: "Výška textu [mm na papíře]" },
    Klic { klic: "krok_hlavni", vychozi: "100", popis: "Krok hlavního staničení [m]" },
    Klic { klic: "krok_vedlejsi", vychozi: "20", popis: "Krok vedlejšího staničení [m] (0 = nekreslit)" },
    Klic { klic: "format", vychozi: "km", popis: "Formát staničení" },
    Klic { klic: "desetin_hlavni", vychozi: "0", popis: "Desetinná místa hlavního staničení" },
    Klic { klic: "desetin_body", vychozi: "2", popis: "Desetinná místa u hlavních bodů" },
    Klic { klic: "carka", vychozi: "1", popis: "Desetinná čárka" },
    Klic { klic: "popis_body", vychozi: "1", popis: "Popisovat hlavní body (ZÚ, KÚ, TK, KT)" },
    Klic { klic: "popis_vrcholy", vychozi: "0", popis: "Popisovat vrcholy (V) a poloměry" },
    Klic { klic: "zkratky", vychozi: "ZÚ,KÚ,TK,KT,V", popis: "Zkratky bodů ZÚ,KÚ,TK,KT,V" },
    Klic { klic: "polygon", vychozi: "1", popis: "Kreslit tečnový polygon" },
];

pub const STYL_ZAKLAD: &str = "SITUACE_500";

/// Vestavěné styly tras (v kódu; uživatelské styly ve výkrese přijdou ve fázi 2).
pub fn vestavene_styly() -> Vec<Styl> {
    vec![
        Styl::novy(STYL_ZAKLAD, "Situace 1:500"),
        Styl::odvozeny("SITUACE_250", "Situace 1:250", STYL_ZAKLAD, &[("meritko", "250"), ("krok_vedlejsi", "10")]),
        Styl::odvozeny("SITUACE_1000", "Situace 1:1000", STYL_ZAKLAD, &[("meritko", "1000"), ("krok_vedlejsi", "50")]),
        Styl::odvozeny(
            "JEN_OSA",
            "Jen osa bez popisů",
            STYL_ZAKLAD,
            &[("krok_hlavni", "0"), ("krok_vedlejsi", "0"), ("popis_body", "0"), ("polygon", "0")],
        ),
    ]
}

/// Další volné id trasy („T1“, „T2“…).
pub fn volne_id<'a>(obsazena: impl IntoIterator<Item = &'a str>) -> String {
    let o: Vec<&str> = obsazena.into_iter().collect();
    (1..).map(|i| format!("T{i}")).find(|k| !o.iter().any(|x| x.eq_ignore_ascii_case(k))).unwrap_or_default()
}

#[cfg(test)]
mod testy {
    use super::*;
    use crate::styl::vysledne;

    #[test]
    fn zapis_a_cteni() {
        let mut d = DefiniceTrasy::nova(
            "T1",
            "Ulice Nová = hlavní",
            vec![Vrchol::new(-740123.456, -1045678.9, 0.0), Vrchol::new(-740000.0, -1045600.0, 150.0), Vrchol::new(-739900.0, -1045700.0, 0.0)],
            STYL_ZAKLAD,
        );
        d.pocatek = 120.5;
        d.vyjimky.insert("krok_vedlejsi".into(), "10".into());
        d.rovnice.push((300.0, 310.0));
        d.nivelety.push(crate::niveleta::Niveleta::nova("N1", "Niveleta", vec![
            crate::niveleta::VrcholNivelety::new(0.0, 250.0, 0.0),
            crate::niveleta::VrcholNivelety::new(120.0, 251.5, 3000.0),
            crate::niveleta::VrcholNivelety::new(300.0, 249.0, 0.0),
        ]));
        let r = d.zapis();
        assert!(r.iter().all(|x| x.len() < 255));
        let rr: Vec<&str> = r.iter().map(String::as_str).collect();
        assert_eq!(DefiniceTrasy::cti(&rr), Some(d));
    }

    #[test]
    fn cizi_zaznam() {
        assert_eq!(DefiniceTrasy::cti(&["TEREN2", "id=X"]), None);
    }

    #[test]
    fn posun_a_otoceni() {
        let d = DefiniceTrasy::nova("T1", "A", vec![Vrchol::new(10.0, 0.0, 0.0), Vrchol::new(20.0, 0.0, 0.0)], STYL_ZAKLAD);
        let w = d.svetove([110.0, 50.0], std::f64::consts::FRAC_PI_2);
        assert!((w[0].x - 110.0).abs() < 1e-9 && (w[0].y - 50.0).abs() < 1e-9);
        assert!((w[1].x - 110.0).abs() < 1e-9 && (w[1].y - 60.0).abs() < 1e-9);
    }

    #[test]
    fn odvozeny_styl() {
        let h = vysledne(SCHEMA, &vestavene_styly(), "SITUACE_1000", &BTreeMap::new());
        assert_eq!(h.cislo("meritko"), 1000.0);
        assert_eq!(h.cislo("krok_hlavni"), 100.0);
    }

    #[test]
    fn id() {
        assert_eq!(volne_id(["T1", "t2"]), "T3");
    }
}
