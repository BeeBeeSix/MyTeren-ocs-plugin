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
    /// Pásy se berou ze stylu (`pasy` je jen záloha); `false` = vlastní pásy pohledu.
    pub pasy_ze_stylu: bool,
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
            pasy_ze_stylu: false,
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
        if self.pasy_ze_stylu {
            r.push("pasy_ze_stylu=1".into());
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
                "pasy_ze_stylu" => d.pasy_ze_stylu = h == "1",
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
/// Výchozí hodnoty odpovídají předloze vzhledu „Výkresový“ (viz `vzhled::predloha`):
/// písmo ISOCPEUR, čísla v ose svislic.
pub const SCHEMA: &[Klic] = &[
    Klic { klic: "cary_pasu", vychozi: "vykres", popis: "Čáry pásů (rámeček / jen svislice)" },
    Klic { klic: "stupnice", vychozi: "skryt", popis: "Výšková stupnice (vlevo / obě strany / skrýt)" },
    Klic { klic: "mrizka", vychozi: "0", popis: "Mřížka v grafu" },
    Klic { klic: "ram", vychozi: "0", popis: "Rámeček grafu" },
    Klic { klic: "staniceni", vychozi: "km", popis: "Formát staničení" },
    Klic { klic: "sklon", vychozi: "promile", popis: "Jednotka sklonu (‰ / %)" },
    Klic { klic: "sablona", vychozi: "", popis: "Šablona (obor)" },
    Klic { klic: "predloha", vychozi: "vykresovy", popis: "Předloha vzhledu" },
    Klic { klic: "p.nadpis", vychozi: "ISOCPEUR;3.5;1;0;0;#e00000", popis: "Písmo nadpisu" },
    Klic { klic: "p.zahlavi", vychozi: "ISOCPEUR;2.5;1;0;0;", popis: "Písmo záhlaví pásů" },
    Klic { klic: "p.hodnota", vychozi: "ISOCPEUR;2.5;1;0;0;", popis: "Písmo hodnot v pásech" },
    Klic { klic: "p.usek", vychozi: "ISOCPEUR;2.5;1;0;0;", popis: "Písmo textů úseků" },
    Klic { klic: "p.graf", vychozi: "ISOCPEUR;2.5;1;0;0;", popis: "Písmo popisů v grafu" },
    Klic { klic: "p.legenda", vychozi: "ISOCPEUR;2.5;1;0;0;", popis: "Písmo legendy a měřítka" },
    Klic { klic: "tl.tenka", vychozi: "0.13", popis: "Tenká čára [mm]" },
    Klic { klic: "tl.stredni", vychozi: "0.25", popis: "Střední čára [mm]" },
    Klic { klic: "tl.silna", vychozi: "0.5", popis: "Silná čára [mm]" },
    Klic { klic: "c.teren", vychozi: "#2fb82f;stredni", popis: "Čára terénu" },
    Klic { klic: "c.niveleta", vychozi: "#ff2020;silna", popis: "Čára nivelety" },
    Klic { klic: "c.polygon", vychozi: "#8f97a1;tenka", popis: "Tečnový polygon" },
    Klic { klic: "c.povrchy", vychozi: "#e040e0;stredni", popis: "Další povrchy" },
    Klic { klic: "c.pasy", vychozi: ";tenka", popis: "Čáry pásů" },
    Klic { klic: "c.mrizka", vychozi: ";tenka", popis: "Mřížka a svislice" },
    Klic { klic: "c.text", vychozi: "", popis: "Barva textů (prázdná = podle hladiny)" },
    Klic { klic: "podbarvit_zahlavi", vychozi: "0", popis: "Podbarvit sloupec záhlaví" },
    Klic { klic: "vypln_terenu", vychozi: "zadna", popis: "Plocha pod terénem (plna / zadna)" },
    Klic { klic: "svislice", vychozi: "1", popis: "Svislice z lomových bodů do grafu" },
    Klic { klic: "svislice_pasu", vychozi: "0", popis: "Svislice v pásech s hodnotami (vypnuto = čísla v ose staničení)" },
    Klic { klic: "legenda", vychozi: "1", popis: "Legenda čar" },
    Klic { klic: "stitek", vychozi: "0", popis: "Měřítko ve štítku" },
    Klic { klic: "sirka_zahlavi", vychozi: "75", popis: "Šířka záhlaví [mm]" },
    Klic { klic: "hustota", vychozi: "1", popis: "Násobek výšky pásů" },
    Klic { klic: "nadpis", vychozi: "Podélný profil: {trasa}", popis: "Nadpis ({trasa}, {povrch}, {meritko})" },
    Klic { klic: "povrch_upraveny", vychozi: "", popis: "Upravený terén – id povrchu (prázdné = automaticky)" },
    Klic { klic: "nazev_nivelety", vychozi: "Niveleta", popis: "Název nivelety v legendě" },
];

/// Šablony podle oboru: id stylu, český název, kód sady pásů.
pub const SABLONY: [(&str, &str, &str); 3] = [
    ("JEDNODUCHY", "Jednoduchý – terén a pozemky", "jednoduchy"),
    ("KANALIZACE", "Vodovod a kanalizace", "kanalizace"),
    ("TEREN_VYKOP", "Terén a výkop", "teren_vykop"),
];

/// Dřívější vestavěné styly: v nabídkách už nejsou, ale pohledy ze starších
/// výkresů s nimi dál fungují (nastavení se skládá z nich).
const STARSI_STYLY: [(&str, &str, &str); 3] = [
    ("ZAKLAD", "Základní", ""),
    ("OBECNA", "Obecná – stávající a upravený terén", "obecna"),
    ("TOK", "Vodní toky", "tok"),
];

/// Vestavěné styly pohledu (šablony podle oboru dědí ze základního).
pub fn styly_vestavene() -> Vec<Styl> {
    // základ = výchozí hodnoty schématu (výkresový vzhled, ISOCPEUR)
    let mut v = vec![Styl::novy("ZAKLAD", "Základní")];
    for (id, nazev, sada) in SABLONY.iter().chain(STARSI_STYLY[1..].iter()) {
        let mut h: Vec<(&str, &str)> = vec![("sablona", sada)];
        if matches!(*sada, "jednoduchy" | "kanalizace" | "teren_vykop") {
            h.push(("staniceni", "m"));
            // niveleta představuje upravený terén
            h.push(("nazev_nivelety", "Upravený terén"));
            h.push(("c.niveleta", "#e040e0;stredni"));
        }
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
            d.pasy_ze_stylu = true;
            d.prevyseni = 1.0;
        }
        d
    }

    /// Nový pohled podle stylu profilu (pásy, měřítko a převýšení ze stylu).
    pub fn podle_stylu(id: &str, trasa: &str, povrch: &str, s: &StylProfilu) -> Self {
        let mut d = Self::novy(id, trasa, povrch);
        d.styl = s.id.clone();
        d.pasy = s.pasy.clone();
        d.pasy_ze_stylu = true;
        d.meritko = s.meritko;
        d.prevyseni = s.prevyseni;
        d
    }
}

/// Styl profilu: pojmenované nastavení vzhledu a sada pásů (vzor pro pohledy).
#[derive(Clone, Debug, PartialEq)]
pub struct StylProfilu {
    pub id: String,
    pub jmeno: String,
    /// Hodnoty klíčů `SCHEMA` (chybějící = výchozí hodnota schématu).
    pub hodnoty: BTreeMap<String, String>,
    pub pasy: Vec<Pas>,
    /// Výchozí měřítko a převýšení nového pohledu.
    pub meritko: f64,
    pub prevyseni: f64,
}

impl StylProfilu {
    /// Vestavěné styly (šablony podle oboru).
    pub fn vestavene() -> Vec<Self> {
        let styly = styly_vestavene();
        SABLONY
            .iter()
            .map(|(id, jmeno, sada)| {
                let h = vysledne(&SCHEMA, &styly, id, &BTreeMap::new());
                let hodnoty = SCHEMA
                    .iter()
                    .filter(|k| h.text(k.klic) != k.vychozi)
                    .map(|k| (k.klic.to_string(), h.text(k.klic).to_string()))
                    .collect();
                Self {
                    id: id.to_string(),
                    jmeno: jmeno.to_string(),
                    hodnoty,
                    pasy: if sada.is_empty() { vychozi_sada() } else { sada_sablony(sada) },
                    meritko: 1000.0,
                    // jako ruční výkresy: výšky ve stejném měřítku jako délky
                    prevyseni: 1.0,
                }
            })
            .collect()
    }

    pub fn je_vestaveny(&self) -> bool {
        SABLONY.iter().any(|s| s.0 == self.id)
    }

    /// Řádky pro uložení do výkresu.
    pub fn zapis(&self) -> Vec<String> {
        let mut r = vec![
            format!("id={}", self.id),
            format!("jmeno={}", self.jmeno),
            format!("meritko={}", f(self.meritko)),
            format!("prevyseni={}", f(self.prevyseni)),
        ];
        for (k, v) in &self.hodnoty {
            r.push(format!("k.{k}={v}"));
        }
        if self.pasy.is_empty() {
            r.push("pasy=zadne".into());
        }
        for p in &self.pasy {
            r.push(format!("pas={}", p.zapis()));
        }
        r
    }

    pub fn precti<S: AsRef<str>>(radky: &[S]) -> Option<Self> {
        let mut s = Self { id: String::new(), jmeno: String::new(), hodnoty: BTreeMap::new(), pasy: Vec::new(), meritko: 1000.0, prevyseni: 10.0 };
        for r in radky {
            let Some((k, h)) = r.as_ref().split_once('=') else { continue };
            match k {
                "id" => s.id = h.into(),
                "jmeno" => s.jmeno = h.into(),
                "meritko" => s.meritko = cti(h).filter(|x| *x >= 10.0).unwrap_or(1000.0),
                "prevyseni" => s.prevyseni = cti(h).filter(|x| *x > 0.0).unwrap_or(10.0),
                "pas" => s.pasy.extend(Pas::cti(h)),
                k if k.starts_with("k.") => {
                    s.hodnoty.insert(k[2..].to_string(), h.to_string());
                }
                _ => {}
            }
        }
        (!s.id.is_empty()).then_some(s)
    }

    /// Styl pro skládání hodnot (`vysledne`).
    pub fn jako_styl(&self) -> Styl {
        Styl { id: self.id.clone(), nazev: self.jmeno.clone(), rodic: None, hodnoty: self.hodnoty.clone() }
    }

    /// Otisk obsahu (pohledy podle něj poznají, že se styl změnil).
    pub fn otisk(&self) -> String {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.zapis().hash(&mut h);
        format!("{:016X}", h.finish())
    }
}

/// Výsledné nastavení pohledu se styly výkresu.
pub fn nastaveni_s(d: &DefinicePohledu, styly: &[StylProfilu]) -> Hodnoty<'static> {
    let s: Vec<Styl> = styly.iter().map(StylProfilu::jako_styl).collect();
    vysledne(&SCHEMA, &s, &d.styl, &d.vyjimky)
}

/// Definice, podle které se pohled kreslí: pásy ze stylu, je-li to tak nastavené.
pub fn efektivni(d: &DefinicePohledu, styly: &[StylProfilu]) -> DefinicePohledu {
    let mut e = d.clone();
    if d.pasy_ze_stylu {
        if let Some(s) = styly.iter().find(|s| s.id == d.styl) {
            e.pasy = s.pasy.clone();
        }
    }
    e
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
        // výchozí vzhled je výkresový (ISOCPEUR, čísla v ose)
        assert_eq!((n.text("cary_pasu"), n.ano("mrizka")), ("vykres", false));
        assert!(n.text("p.hodnota").starts_with("ISOCPEUR"));
        assert!(!n.ano("svislice_pasu"));
        // pohled se starším stylem (už není v nabídce) dál funguje
        let mut o = DefinicePohledu::novy("P3", "T1", "S");
        o.styl = "OBECNA".into();
        assert_eq!(nastaveni(&o).text("sablona"), "obecna");
        let mut k = DefinicePohledu::podle_sablony("P2", "T1", "S", "KANALIZACE");
        assert_eq!(k.styl, "KANALIZACE");
        assert!(k.pasy.iter().any(|p| p.zahlavi() == "Kóta dna potrubí"));
        let n = nastaveni(&k);
        // kanalizace má výkresový vzhled
        assert_eq!((n.text("cary_pasu"), n.text("stupnice"), n.ano("ram")), ("vykres", "skryt", false));
        assert_eq!(n.text("predloha"), "vykresovy");
        k.vyjimky.insert("stupnice".into(), "vlevo".into());
        assert_eq!(nastaveni(&k).text("stupnice"), "vlevo");
        let r = k.zapis();
        let rr: Vec<&str> = r.iter().map(String::as_str).collect();
        assert_eq!(DefinicePohledu::cti(&rr), Some(k));
    }

    #[test]
    fn styly_profilu() {
        let v = StylProfilu::vestavene();
        assert_eq!(v.len(), SABLONY.len());
        let k = v.iter().find(|s| s.id == "KANALIZACE").unwrap();
        assert_eq!(k.hodnoty.get("staniceni").map(String::as_str), Some("m"));
        assert_eq!(StylProfilu::precti(&k.zapis()).as_ref(), Some(k));
        // pohled podle stylu bere pásy ze stylu, nastavení se skládá přes styly výkresu
        let mut vlastni = k.clone();
        vlastni.id = "MUJ".into();
        vlastni.pasy.truncate(2);
        vlastni.hodnoty.insert("stupnice".into(), "obe".into());
        let styly = vec![v[0].clone(), vlastni.clone()];
        let mut d = DefinicePohledu::podle_stylu("P1", "T1", "S", &vlastni);
        d.pasy.clear();
        assert_eq!(efektivni(&d, &styly).pasy.len(), 2);
        assert_eq!(nastaveni_s(&d, &styly).text("stupnice"), "obe");
        d.pasy_ze_stylu = false;
        assert!(efektivni(&d, &styly).pasy.is_empty());
        // zápis pohledu drží příznak
        d.pasy_ze_stylu = true;
        let r = d.zapis();
        let rr: Vec<&str> = r.iter().map(String::as_str).collect();
        assert!(DefinicePohledu::cti(&rr).unwrap().pasy_ze_stylu);
        assert_ne!(k.otisk(), vlastni.otisk());
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
