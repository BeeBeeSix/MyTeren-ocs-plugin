//! Pohled podélného profilu: definice (jak se ukládá do výkresu) a rozvržení
//! os – srovnávací rovina, kroky mřížky, převod staničení a výšky na souřadnice
//! pohledu.

use std::collections::BTreeMap;

use crate::pasy::{sada_sablony, vychozi_sada, Pas};
use crate::styl::{vysledne, Hodnoty, Klic, Styl};

/// Verze zápisu definice pohledu.
pub const VERZE: &str = "POHLED1";

#[derive(Clone, Debug, PartialEq)]
pub struct DefinicePohledu {
    pub id: String,
    /// Id trasy, ke které pohled patří.
    pub trasa: String,
    /// Id povrchu Terénu pro terénní profil.
    pub povrch: String,
    /// Rozsah staničení; `None` = celá trasa.
    pub od: Option<f64>,
    pub do_: Option<f64>,
    /// Srovnávací rovina; `None` = automaticky z terénu.
    pub srovnavaci: Option<f64>,
    /// Měřítko délek 1:x (velikost popisů).
    pub meritko: f64,
    /// Převýšení (měřítko výšek = měřítko délek / převýšení).
    pub prevyseni: f64,
    pub styl: String,
    pub vyjimky: BTreeMap<String, String>,
    /// Otisk trasy a povrchu, ze kterých byl pohled naposledy nakreslený
    /// (při neshodě se pohled překreslí).
    pub otisk: String,
    /// Pásy pod profilem (shora dolů).
    pub pasy: Vec<Pas>,
    /// Osy z posledního kreslení: od, do, srovnávací rovina, horní okraj,
    /// převýšení (pro klikání do pohledu bez přepočtu terénu).
    pub osy: Option<[f64; 5]>,
}

fn f(v: f64) -> String {
    let t = format!("{v:.6}");
    let t = t.trim_end_matches('0').trim_end_matches('.');
    if t.is_empty() || t == "-" { "0".into() } else { t.into() }
}

fn cti(t: &str) -> Option<f64> {
    t.trim().replace(',', ".").parse().ok()
}

impl Default for DefinicePohledu {
    fn default() -> Self {
        Self::novy("", "", "")
    }
}

impl DefinicePohledu {
    pub fn novy(id: &str, trasa: &str, povrch: &str) -> Self {
        Self {
            id: id.into(),
            trasa: trasa.into(),
            povrch: povrch.into(),
            od: None,
            do_: None,
            srovnavaci: None,
            meritko: 1000.0,
            prevyseni: 10.0,
            styl: "ZAKLAD".into(),
            vyjimky: BTreeMap::new(),
            otisk: String::new(),
            pasy: vychozi_sada(),
            osy: None,
        }
    }

    pub fn zapis(&self) -> Vec<String> {
        let mut r = vec![VERZE.to_string()];
        r.push(format!("id={}", self.id));
        r.push(format!("trasa={}", self.trasa));
        r.push(format!("povrch={}", self.povrch.chars().take(200).collect::<String>()));
        if let Some(x) = self.od {
            r.push(format!("od={}", f(x)));
        }
        if let Some(x) = self.do_ {
            r.push(format!("do={}", f(x)));
        }
        if let Some(x) = self.srovnavaci {
            r.push(format!("srovnavaci={}", f(x)));
        }
        r.push(format!("meritko={}", f(self.meritko)));
        r.push(format!("prevyseni={}", f(self.prevyseni)));
        r.push(format!("styl={}", self.styl));
        if !self.otisk.is_empty() {
            r.push(format!("otisk={}", self.otisk));
        }
        if self.pasy.is_empty() {
            r.push("pasy=zadne".into());
        }
        for p in &self.pasy {
            r.push(format!("pas={}", p.zapis()));
        }
        if let Some(o) = self.osy {
            r.push(format!("osy={}", o.map(f).join(";")));
        }
        for (k, v) in &self.vyjimky {
            r.push(format!("x.{k}={}", v.chars().take(200).collect::<String>()));
        }
        r
    }

    pub fn cti(radky: &[&str]) -> Option<Self> {
        if radky.first().copied() != Some(VERZE) {
            return None;
        }
        let mut d = DefinicePohledu::novy("", "", "");
        d.pasy.clear();
        let mut bez_pasu = false;
        for r in &radky[1..] {
            let Some((k, h)) = r.split_once('=') else { continue };
            match k {
                "id" => d.id = h.into(),
                "trasa" => d.trasa = h.into(),
                "povrch" => d.povrch = h.into(),
                "od" => d.od = cti(h),
                "do" => d.do_ = cti(h),
                "srovnavaci" => d.srovnavaci = cti(h),
                "meritko" => d.meritko = cti(h).unwrap_or(1000.0),
                "prevyseni" => d.prevyseni = cti(h).unwrap_or(10.0),
                "styl" => d.styl = h.into(),
                "otisk" => d.otisk = h.into(),
                "pas" => d.pasy.extend(Pas::cti(h)),
                "pasy" => bez_pasu = h == "zadne",
                "osy" => {
                    let c: Vec<f64> = h.split(';').filter_map(cti).collect();
                    if let [a, b, c2, d2, e] = c[..] {
                        d.osy = Some([a, b, c2, d2, e]);
                    }
                }
                k if k.starts_with("x.") => {
                    d.vyjimky.insert(k[2..].to_string(), h.to_string());
                }
                _ => {}
            }
        }
        // pohled z doby před pásy dostane výchozí sadu
        if d.pasy.is_empty() && !bez_pasu {
            d.pasy = vychozi_sada();
        }
        (!d.id.is_empty() && !d.trasa.is_empty()).then_some(d)
    }
}

/// Nastavení vzhledu pohledu (klíče stylu; pohled může mít výjimky).
pub const SCHEMA: [Klic; 8] = [
    Klic { klic: "cary_pasu", vychozi: "ramecek", popis: "Čáry pásů (rámeček / jen svislice)" },
    Klic { klic: "stupnice", vychozi: "vlevo", popis: "Výšková stupnice (vlevo / obě strany / skrýt)" },
    Klic { klic: "mrizka", vychozi: "1", popis: "Mřížka v grafu" },
    Klic { klic: "ram", vychozi: "1", popis: "Rámeček grafu" },
    Klic { klic: "staniceni", vychozi: "km", popis: "Formát staničení" },
    Klic { klic: "sklon", vychozi: "promile", popis: "Jednotka sklonu (‰ / %)" },
    Klic { klic: "pismo", vychozi: "2.5", popis: "Výška písma [mm]" },
    Klic { klic: "sablona", vychozi: "", popis: "Šablona (obor)" },
];

/// Šablony podle oboru: id stylu, český název, kód sady pásů.
pub const SABLONY: [(&str, &str, &str); 4] = [
    ("ZAKLAD", "Základní", ""),
    ("OBECNA", "Obecná – stávající a upravený terén", "obecna"),
    ("KANALIZACE", "Vodovod a kanalizace", "kanalizace"),
    ("TOK", "Vodní toky", "tok"),
];

/// Vestavěné styly pohledu (šablony podle oboru dědí ze základního).
pub fn styly_vestavene() -> Vec<Styl> {
    let cisty = [("cary_pasu", "svislice"), ("stupnice", "skryt"), ("mrizka", "0"), ("ram", "0")];
    let mut v = vec![Styl::novy("ZAKLAD", "Základní")];
    for (id, nazev, sada) in &SABLONY[1..] {
        let mut h: Vec<(&str, &str)> = cisty.to_vec();
        h.push(("sablona", sada));
        v.push(Styl::odvozeny(id, nazev, "ZAKLAD", &h));
    }
    v
}

/// Výsledné nastavení pohledu: šablona ← výjimky pohledu.
pub fn nastaveni(d: &DefinicePohledu) -> Hodnoty<'static> {
    vysledne(&SCHEMA, &styly_vestavene(), &d.styl, &d.vyjimky)
}

impl DefinicePohledu {
    /// Nový pohled podle šablony (id stylu ze `SABLONY`): styl i sada pásů.
    pub fn podle_sablony(id: &str, trasa: &str, povrch: &str, sablona: &str) -> Self {
        let mut d = Self::novy(id, trasa, povrch);
        if let Some((sid, _, sada)) = SABLONY.iter().find(|s| s.0 == sablona) {
            d.styl = sid.to_string();
            d.pasy = if sada.is_empty() { vychozi_sada() } else { sada_sablony(sada) };
        }
        d
    }
}

/// Rozvržení os pohledu.
#[derive(Clone, Debug, PartialEq)]
pub struct Osy {
    pub od: f64,
    pub do_: f64,
    /// Srovnávací rovina = spodní okraj pohledu.
    pub srovnavaci: f64,
    /// Horní okraj (výška).
    pub horni: f64,
    /// Krok vodorovných čar mřížky (výška) [m].
    pub krok_z: f64,
    /// Krok svislých čar mřížky (staničení) [m].
    pub krok_s: f64,
    pub prevyseni: f64,
}

/// Nejmenší „hezký“ krok (1, 2, 5 × 10ⁿ), který je aspoň `min`.
pub fn hezky_krok(min: f64) -> f64 {
    if min <= 0.0 || !min.is_finite() {
        return 1.0;
    }
    let e = 10f64.powf(min.log10().floor());
    for k in [1.0, 2.0, 5.0, 10.0] {
        if k * e >= min - 1e-12 {
            return k * e;
        }
    }
    10.0 * e
}

impl Osy {
    /// Rozvrhne osy: mezery mřížky aspoň 10 mm (výšky) a 15 mm (staničení)
    /// na papíře, srovnávací rovina a horní okraj s rezervou jednoho kroku.
    pub fn rozvrhni(od: f64, do_: f64, zmin: f64, zmax: f64, d: &DefinicePohledu) -> Self {
        let m = d.meritko.max(1.0);
        let prev = d.prevyseni.max(0.1);
        let krok_z = hezky_krok(10.0 * m / 1000.0 / prev);
        let krok_s = hezky_krok(15.0 * m / 1000.0);
        let srovnavaci = d.srovnavaci.unwrap_or(((zmin - krok_z) / krok_z).floor() * krok_z);
        let horni = (((zmax + krok_z) / krok_z).ceil() * krok_z).max(srovnavaci + krok_z);
        Self { od, do_, srovnavaci, horni, krok_z, krok_s, prevyseni: prev }
    }

    /// Bod pohledu (vůči levému dolnímu rohu) ze staničení a výšky.
    pub fn bod(&self, s: f64, z: f64) -> [f64; 2] {
        [s - self.od, (z - self.srovnavaci) * self.prevyseni]
    }

    pub fn sirka(&self) -> f64 {
        self.do_ - self.od
    }

    pub fn vyska(&self) -> f64 {
        (self.horni - self.srovnavaci) * self.prevyseni
    }

    /// Staničení svislých čar mřížky (násobky kroku uvnitř rozsahu).
    pub fn staniceni_mrizky(&self) -> Vec<f64> {
        let mut out = Vec::new();
        let mut s = (self.od / self.krok_s).ceil() * self.krok_s;
        while s <= self.do_ + 1e-9 && out.len() < 10_000 {
            out.push(s);
            s += self.krok_s;
        }
        out
    }

    /// Výšky vodorovných čar mřížky.
    pub fn vysky_mrizky(&self) -> Vec<f64> {
        let mut out = Vec::new();
        let mut z = self.srovnavaci;
        while z <= self.horni + 1e-9 && out.len() < 10_000 {
            out.push(z);
            z += self.krok_z;
        }
        out
    }
}

/// Další volné id pohledu („P1“, „P2“…).
pub fn volne_id<'a>(obsazena: impl IntoIterator<Item = &'a str>) -> String {
    let o: Vec<&str> = obsazena.into_iter().collect();
    (1..).map(|i| format!("P{i}")).find(|k| !o.iter().any(|x| x.eq_ignore_ascii_case(k))).unwrap_or_default()
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn zapis_cteni() {
        let mut d = DefinicePohledu::novy("P1", "T1", "Stavajici");
        d.od = Some(20.0);
        d.srovnavaci = Some(244.0);
        d.prevyseni = 5.0;
        d.otisk = "1A;2B".into();
        d.osy = Some([0.0, 330.0, 245.0, 253.0, 10.0]);
        let r = d.zapis();
        let rr: Vec<&str> = r.iter().map(String::as_str).collect();
        assert_eq!(DefinicePohledu::cti(&rr), Some(d));
    }

    #[test]
    fn sablony_a_nastaveni() {
        let d = DefinicePohledu::novy("P1", "T1", "S");
        let n = nastaveni(&d);
        assert_eq!((n.text("cary_pasu"), n.ano("mrizka")), ("ramecek", true));
        let mut k = DefinicePohledu::podle_sablony("P2", "T1", "S", "KANALIZACE");
        assert_eq!(k.styl, "KANALIZACE");
        assert!(k.pasy.iter().any(|p| p.zahlavi() == "Kóta dna potrubí"));
        let n = nastaveni(&k);
        assert_eq!((n.text("cary_pasu"), n.text("stupnice"), n.ano("ram")), ("svislice", "skryt", false));
        k.vyjimky.insert("stupnice".into(), "vlevo".into());
        assert_eq!(nastaveni(&k).text("stupnice"), "vlevo");
        let r = k.zapis();
        let rr: Vec<&str> = r.iter().map(String::as_str).collect();
        assert_eq!(DefinicePohledu::cti(&rr), Some(k));
    }

    #[test]
    fn kroky() {
        assert_eq!(hezky_krok(0.8), 1.0);
        assert_eq!(hezky_krok(1.0), 1.0);
        assert_eq!(hezky_krok(13.0), 20.0);
        assert_eq!(hezky_krok(0.3), 0.5);
    }

    #[test]
    fn rozvrzeni() {
        let d = DefinicePohledu::novy("P1", "T1", "S"); // 1:1000, převýšení 10
        let o = Osy::rozvrhni(0.0, 330.0, 246.3, 251.8, &d);
        assert_eq!(o.krok_z, 1.0);
        assert_eq!(o.krok_s, 20.0);
        assert_eq!(o.srovnavaci, 245.0);
        assert_eq!(o.horni, 253.0);
        assert_eq!(o.bod(100.0, 247.0), [100.0, 20.0]);
        assert_eq!(o.staniceni_mrizky().len(), 17);
        assert_eq!(o.vysky_mrizky().len(), 9);
    }
}
