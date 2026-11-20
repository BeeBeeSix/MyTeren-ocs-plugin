//! Panel „Editor nivelety“ v doku Open CAD Studia.
//!
//! Hlavička (trasa, pohled, délka, počet vrcholů), tabulka vrcholů se sloupci
//! R | L | K (přepsaný sloupec = zadání, ostatní se dopočtou; hvězdička =
//! zadaná hodnota), detail vybraného vrcholu (staničení, výška, sklon před
//! a za, oblouk, T a y), nástroje a kontrola.
//!
//! Změny polí chodí jako upozornění `PanelAction` při každém stisku klávesy
//! a bez přístupu k výkresu. Panel si je proto jen zapamatuje a správce
//! (pracovní vlákno s kanálem do výkresu) je po krátké pauze v psaní
//! zapíše do výkresu a panel obnoví. Tah úchopem panel obnoví přes
//! `uchopy::spust_obnovu`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use ocs_plugin_api::dialog::{DialogControl, DialogValue, PanelDef, TextStyle};
use ocs_plugin_api::host::Document as CadDocument;
use trasy::niveleta::{self, Niveleta, VrcholNivelety, Zadani};
use trasy::preklad::tr;
use trasy::stanice::cislo;
use trasy::trf;

use crate::tra::objekt::{self, ObjektTrasy};
use crate::vykres::{PresKanal, Vykres};

pub const ID: &str = "trasy_editor_nivelety";
pub const PRIKAZ: &str = "TRA_EDITOR_NIVELETY";

/// Pauza v psaní, po které se změna z panelu zapíše do výkresu.
const PAUZA: Duration = Duration::from_millis(900);

/// Typy zadání oblouku v nabídce detailu (pořadí = index volby).
const TYPY: [&str; 3] = ["R", "L", "K"];

#[derive(Clone, Debug)]
enum Zmena {
    Tabulka(Vec<Vec<String>>),
    /// Pole detailu vybraného vrcholu (id, text).
    Pole(String, String),
    /// Typ zadání oblouku vybraného vrcholu (index do `TYPY`).
    Typ(usize),
}

struct Stav {
    /// Id trasy, jejíž niveletu panel ukazuje.
    trasa: Option<String>,
    /// Vybraný vrchol (index).
    vybrany: usize,
    zmena: Option<(Zmena, Instant)>,
    /// Panel obnovit hned (změna výběru vrcholu).
    obnovit: bool,
    /// Pohled, pro který se panel naposledy sám otevřel (klik na niveletu).
    otevren_pro: Option<u64>,
}

static STAV: Mutex<Stav> = Mutex::new(Stav { trasa: None, vybrany: 0, zmena: None, obnovit: false, otevren_pro: None });
static OTEVREN: AtomicBool = AtomicBool::new(false);

fn stav<R>(f: impl FnOnce(&mut Stav) -> R) -> R {
    f(&mut STAV.lock().unwrap_or_else(|e| e.into_inner()))
}

pub fn otevren() -> bool {
    OTEVREN.load(Ordering::SeqCst)
}

pub fn aktivni() -> Option<String> {
    stav(|s| s.trasa.clone())
}

pub fn vybrany() -> usize {
    stav(|s| s.vybrany)
}

/// Panel bude ukazovat niveletu trasy `id` (jiná trasa = zahodit rozepsané změny).
pub fn nastav_aktivni(id: &str) {
    stav(|s| {
        if s.trasa.as_deref() != Some(id) {
            s.zmena = None;
            s.vybrany = 0;
        }
        s.trasa = Some(id.to_string());
    });
}

pub fn nastav_vybrany(i: usize) {
    stav(|s| s.vybrany = i);
}

/// Má se panel sám otevřít pro vybraný pohled? (jen poprvé pro daný pohled)
pub fn otevrit_pro(pohled: Option<u64>) -> bool {
    stav(|s| {
        let nove = pohled.is_some() && s.otevren_pro != pohled;
        s.otevren_pro = pohled;
        nove
    })
}

/// Číslo z buňky nebo pole (desetinná čárka, mezery, hvězdička zadané hodnoty, ‰).
pub fn cti(t: &str) -> Option<f64> {
    let t: String = t.chars().filter(|c| !matches!(c, '*' | ' ' | '\u{a0}' | '‰')).collect();
    t.replace(',', ".").parse().ok().filter(|x: &f64| x.is_finite())
}

fn tlacitko(id: &str, label: &str, prikaz: &str) -> DialogControl {
    DialogControl::Button { id: id.into(), label: label.into(), action: Some(prikaz.into()), align_h: None, width: None }
}

fn radek(c: Vec<DialogControl>) -> DialogControl {
    DialogControl::Row { controls: c, align_h: None, align_v: None }
}

fn nadpis(t: &str) -> DialogControl {
    DialogControl::StyledText { text: t.into(), style: TextStyle::Heading, align_h: None }
}

fn text(id: &str, label: &str) -> DialogControl {
    DialogControl::Text { id: id.into(), label: label.into(), default: String::new() }
}

pub fn definice() -> PanelDef {
    OTEVREN.store(true, Ordering::SeqCst);
    PanelDef::new(ID, tr("Editor nivelety"))
        .with_command(PRIKAZ)
        .with_width(470.0)
        .with_control(DialogControl::Label { id: "info".into(), text: String::new(), align_h: None })
        .with_control(nadpis(tr("Vrcholy")))
        .with_control(DialogControl::Table {
            id: "vrcholy".into(),
            label: None,
            headers: vec!["#".into(), tr("Staničení").into(), tr("Výška").into(), "R".into(), "L".into(), "K".into(), tr("Sklon za").into()],
            rows: Vec::new(),
            allow_add: false,
            allow_delete: false,
        })
        .with_control(DialogControl::InfoText {
            text: tr("Oblouk zadáte přepsáním R, L nebo K – ostatní se dopočítají (* = zadaná hodnota, ta platí i po změně sklonů). 0 = bez oblouku. Změna se do výkresu zapíše chvíli po dopsání.").into(),
            align_h: None,
        })
        .with_control(nadpis(tr("Vybraný vrchol")))
        .with_control(DialogControl::Number { id: "vrchol".into(), label: tr("Vrchol č.").into(), default: 1.0, min: Some(1.0), max: None, step: Some(1.0) })
        .with_control(DialogControl::Fields {
            id: "detail".into(),
            title: None,
            controls: vec![
                text("d_s", tr("Staničení [m]")),
                text("d_z", tr("Výška [m n. m.]")),
                text("d_ip", tr("Sklon před [‰]")),
                text("d_iz", tr("Sklon za [‰]")),
                DialogControl::Choice {
                    id: "d_typ".into(),
                    label: tr("Oblouk zadán").into(),
                    options: vec![tr("poloměrem R").into(), tr("délkou L").into(), tr("hodnotou K").into()],
                    default_index: 0,
                },
                text("d_hod", tr("Hodnota (R / L / K)")),
                DialogControl::Label { id: "d_ty".into(), text: String::new(), align_h: None },
            ],
            collapsed: false,
        })
        .with_control(nadpis(tr("Nástroje")))
        .with_control(radek(vec![
            tlacitko("sklon", tr("Vrchol sklonem"), "TRA_NIV_SKLON"),
            tlacitko("teren", tr("Konce na terén"), "TRA_NIV_TEREN"),
            tlacitko("smazat", tr("Smazat vrchol"), "TRA_NIVELETA_SMAZAT"),
        ]))
        .with_control(nadpis(tr("Kontrola")))
        .with_control(DialogControl::Label { id: "kontrola".into(), text: String::new(), align_h: None })
}

/// Trasa, jejíž niveletu panel ukazuje: aktivní, jinak první s niveletou.
pub fn trasa_panelu(doc: &CadDocument) -> Option<ObjektTrasy> {
    let vse: Vec<ObjektTrasy> = objekt::trasy(doc).into_iter().filter(|t| !t.def.nivelety.is_empty()).collect();
    let id = aktivni();
    vse.iter().find(|t| Some(&t.def.id) == id.as_ref()).cloned().or_else(|| vse.into_iter().next())
}

/// Hodnoty R, L, K vnitřního vrcholu (bez oblouku nuly; délka bez lomu zůstává).
fn rlk(n: &Niveleta, k: usize) -> [f64; 3] {
    match n.oblouky().into_iter().find(|o| o.vrchol == k) {
        Some(o) => [o.r, o.delka, o.k],
        None => match n.vrcholy[k].zadani {
            Zadani::L(l) => [0.0, l, 0.0],
            _ => [0.0, 0.0, 0.0],
        },
    }
}

fn index_zadani(z: Zadani) -> usize {
    match z {
        Zadani::R => 0,
        Zadani::L(_) => 1,
        Zadani::K => 2,
    }
}

/// Buňky R, L, K vrcholu; zadaná hodnota má hvězdičku. Krajní vrchol prázdné.
pub fn bunky_rlk(n: &Niveleta, k: usize) -> [String; 3] {
    if k == 0 || k + 1 >= n.vrcholy.len() {
        return [String::new(), String::new(), String::new()];
    }
    let h = rlk(n, k);
    let zad = index_zadani(n.vrcholy[k].zadani);
    let des = [1, 2, 2];
    std::array::from_fn(|i| {
        let t = cislo(h[i], des[i], false);
        if i == zad && h[i] > 0.0 { format!("{t} *") } else { t }
    })
}

/// Vrchol podle buněk R, L, K: platí sloupec, který se proti původním buňkám
/// změnil; beze změny zůstane původní zadání. Bez původního vrcholu platí
/// první vyplněný sloupec.
pub fn vrchol_z_bunek(s: f64, z: f64, nove: &[String], puvodni: Option<(&VrcholNivelety, &[String])>) -> VrcholNivelety {
    let bunka = |b: &[String], i: usize| b.get(i).and_then(|c| cti(c));
    match puvodni {
        Some((v, puv)) => {
            let zmena = (0..3).find(|&i| match (bunka(nove, i), bunka(puv, i)) {
                (Some(a), Some(b)) => (a - b).abs() > 1e-9,
                (a, b) => a.is_some() != b.is_some(),
            });
            match zmena {
                Some(i) => VrcholNivelety::zadany(s, z, TYPY[i], bunka(nove, i).unwrap_or(0.0)),
                None => VrcholNivelety { s, z, ..*v },
            }
        }
        None => match (0..3).find(|&i| bunka(nove, i).is_some_and(|h| h > 0.0)) {
            Some(i) => VrcholNivelety::zadany(s, z, TYPY[i], bunka(nove, i).unwrap_or(0.0)),
            None => VrcholNivelety::new(s, z, 0.0),
        },
    }
}

fn promile(i: Option<f64>) -> String {
    i.map_or(String::new(), |i| cislo(i * 1000.0, 2, false))
}

/// Řádky tabulky panelu: #, staničení, výška, R, L, K, sklon za.
pub fn radky(n: &Niveleta) -> Vec<Vec<String>> {
    (0..n.vrcholy.len())
        .map(|k| {
            let v = n.vrcholy[k];
            let [r, l, kk] = bunky_rlk(n, k);
            let sz = niveleta::sklon_za(&n.vrcholy, k).map_or(String::new(), |i| format!("{} ‰", cislo(i * 1000.0, 2, false)));
            vec![(k + 1).to_string(), cislo(v.s, 3, false), cislo(v.z, 3, false), r, l, kk, sz]
        })
        .collect()
}

/// Text kontroly nivelety.
pub fn kontrola(n: &Niveleta) -> String {
    match n.kontrola() {
        Ok(()) => tr("✓ V pořádku – oblouky se vejdou.").to_string(),
        Err(e) => {
            let (t, a) = e.popis();
            format!("✕ {}", trasy::preklad::formatuj(tr(t), &a.iter().map(|x| x as &dyn std::fmt::Display).collect::<Vec<_>>()))
        }
    }
}

/// Hodnoty panelu pro trasu (bez stahování výkresu).
pub fn hodnoty_trasy(doc: &CadDocument, t: Option<&ObjektTrasy>) -> HashMap<String, DialogValue> {
    let mut m = HashMap::new();
    let Some((t, n)) = t.and_then(|t| t.def.nivelety.first().map(|n| (t, n))) else {
        m.insert("info".into(), DialogValue::Text(tr("Žádná niveleta – klikněte na pohled profilu s niveletou, nebo ji nakreslete tlačítkem Niveleta.").into()));
        m.insert("vrcholy".into(), DialogValue::Table(Vec::new()));
        return m;
    };
    nastav_aktivni(&t.def.id);
    let k = vybrany().min(n.vrcholy.len().saturating_sub(1));
    nastav_vybrany(k);
    let pohled = crate::tra::pohled::pohledy(doc).into_iter().find(|p| p.def.trasa == t.def.id);
    let delka = n.vrcholy.last().map_or(0.0, |v| v.s) - n.vrcholy.first().map_or(0.0, |v| v.s);
    m.insert(
        "info".into(),
        DialogValue::Text(trf!(
            "Trasa {} · pohled {} · délka {} m · vrcholů {}",
            t.def.jmeno,
            pohled.as_ref().map_or("–".to_string(), |p| p.def.id.clone()),
            cislo(delka, 2, true),
            n.vrcholy.len()
        )),
    );
    m.insert("vrcholy".into(), DialogValue::Table(radky(n)));
    m.insert("vrchol".into(), DialogValue::Number((k + 1) as f64));
    let v = n.vrcholy[k];
    let vnitrni = k > 0 && k + 1 < n.vrcholy.len();
    m.insert("d_s".into(), DialogValue::Text(cislo(v.s, 3, false)));
    m.insert("d_z".into(), DialogValue::Text(cislo(v.z, 3, false)));
    m.insert("d_ip".into(), DialogValue::Text(promile(niveleta::sklon_pred(&n.vrcholy, k))));
    m.insert("d_iz".into(), DialogValue::Text(promile(niveleta::sklon_za(&n.vrcholy, k))));
    let zi = index_zadani(v.zadani);
    let moznosti = [tr("poloměrem R"), tr("délkou L"), tr("hodnotou K")];
    m.insert("d_typ".into(), DialogValue::Choice { index: zi, text: moznosti[zi].into() });
    let h = if vnitrni { cislo(rlk(n, k)[zi], 2, false) } else { String::new() };
    m.insert("d_hod".into(), DialogValue::Text(h));
    let ty = if !vnitrni {
        tr("Krajní vrchol – bez oblouku.").to_string()
    } else if let Some(o) = n.oblouky().into_iter().find(|o| o.vrchol == k) {
        trf!(
            "{} oblouk: R = {} m · L = {} m · K = {} · T = {} m · y = {} m",
            if o.vypukly { tr("Vypuklý") } else { tr("Údolnicový") },
            cislo(o.r, 1, true),
            cislo(o.delka, 2, true),
            cislo(o.k, 2, true),
            cislo(o.tecna, 2, true),
            cislo(o.vzepeti, 3, true)
        )
    } else {
        tr("Bez oblouku.").to_string()
    };
    m.insert("d_ty".into(), DialogValue::Text(ty));
    m.insert("kontrola".into(), DialogValue::Text(kontrola(n)));
    m
}

pub fn hodnoty(doc: &CadDocument) -> HashMap<String, DialogValue> {
    hodnoty_trasy(doc, trasa_panelu(doc).as_ref())
}

/// Změna pole v panelu (upozornění bez přístupu k výkresu).
pub fn akce(control_id: &str, _action: &str, value: Option<DialogValue>) {
    let ted = Instant::now();
    match (control_id, value) {
        ("vrcholy", Some(DialogValue::Table(t))) => stav(|s| s.zmena = Some((Zmena::Tabulka(t), ted))),
        ("vrchol", Some(v)) => {
            let x = match v {
                DialogValue::Number(x) => Some(x),
                DialogValue::Text(t) => cti(&t),
                _ => None,
            };
            if let Some(x) = x.filter(|x| *x >= 1.0) {
                stav(|s| {
                    s.vybrany = x.round() as usize - 1;
                    s.obnovit = true;
                });
            }
        }
        ("d_typ", Some(DialogValue::Choice { index, .. })) => stav(|s| s.zmena = Some((Zmena::Typ(index), ted - PAUZA))),
        (id @ ("d_s" | "d_z" | "d_ip" | "d_iz" | "d_hod"), Some(DialogValue::Text(t))) => {
            stav(|s| s.zmena = Some((Zmena::Pole(id.to_string(), t), ted)))
        }
        _ => {}
    }
}

/// Je co zapsat nebo obnovit? (pauza v psaní uplynula)
pub fn je_cas() -> bool {
    stav(|s| s.obnovit || s.zmena.as_ref().is_some_and(|(_, t)| t.elapsed() >= PAUZA))
}

/// Nové vrcholy nivelety podle změny z panelu (`None` = nic se nemění).
fn nove_vrcholy(n: &Niveleta, k: usize, z: &Zmena) -> Result<Option<Vec<VrcholNivelety>>, String> {
    let mut v = n.vrcholy.clone();
    match z {
        Zmena::Tabulka(rows) => {
            if rows.len() != v.len() {
                return Ok(None);
            }
            let puvodni = radky(n);
            for (i, r) in rows.iter().enumerate() {
                let (Some(s), Some(zz)) = (r.get(1).and_then(|c| cti(c)), r.get(2).and_then(|c| cti(c))) else {
                    return Err(trf!("řádek {} nemá platné staničení a výšku", i + 1));
                };
                // nepřepsaná buňka drží přesnou hodnotu (tabulka ukazuje jen 3 desetinná místa)
                let s = if r[1] == puvodni[i][1] { n.vrcholy[i].s } else { s };
                let zz = if r[2] == puvodni[i][2] { n.vrcholy[i].z } else { zz };
                let nove = r.get(3..6).unwrap_or(&[]);
                v[i] = vrchol_z_bunek(s, zz, nove, Some((&n.vrcholy[i], &puvodni[i][3..6])));
            }
        }
        Zmena::Pole(id, t) => {
            let Some(x) = cti(t) else { return Ok(None) };
            match id.as_str() {
                "d_s" => v[k].s = x,
                "d_z" => v[k].z = x,
                "d_ip" => niveleta::nastav_sklon_pred(&mut v, k, x / 1000.0),
                "d_iz" => niveleta::nastav_sklon_za(&mut v, k, x / 1000.0),
                "d_hod" => {
                    let typ = TYPY[index_zadani(v[k].zadani)];
                    v[k] = VrcholNivelety::zadany(v[k].s, v[k].z, typ, x);
                }
                _ => return Ok(None),
            }
        }
        Zmena::Typ(i) => {
            if k == 0 || k + 1 >= v.len() {
                return Ok(None);
            }
            let h = rlk(n, k);
            v[k] = VrcholNivelety::zadany(v[k].s, v[k].z, TYPY[(*i).min(2)], h[(*i).min(2)]);
        }
    }
    if v == n.vrcholy {
        return Ok(None);
    }
    v.sort_by(|a, b| a.s.total_cmp(&b.s));
    Ok(Some(v))
}

/// Zapíše rozepsanou změnu z panelu do výkresu a panel obnoví (správce po pauze v psaní).
pub fn proved(k: &mut PresKanal) {
    let (zmena, _) = stav(|s| {
        s.obnovit = false;
        (s.zmena.take(), ())
    });
    let Ok(doc) = k.dokument() else { return };
    let Some(t) = trasa_panelu(&doc) else { return };
    if let Some((z, _)) = zmena {
        let n = &t.def.nivelety[0];
        let i = vybrany().min(n.vrcholy.len().saturating_sub(1));
        match nove_vrcholy(n, i, &z) {
            Ok(Some(v)) => {
                let s_vybrany = v.get(i).map(|x| x.s);
                match crate::tra::uloz_niveletu_cmd(k, &doc, &t, v.clone(), tr("Úprava nivelety v editoru")) {
                    Ok(p) => {
                        // vybraný vrchol po přeřazení podle staničení
                        if let Some(s) = s_vybrany {
                            if let Some(j) = v.iter().position(|x| x.s == s) {
                                nastav_vybrany(j);
                            }
                        }
                        k.info(&trf!("Trasy: niveleta trasy {} upravena, překresleno pohledů: {}.", t.def.jmeno, p));
                    }
                    Err(e) => k.chyba(&trf!("Trasy: změna se nepřijala – {}", e)),
                }
            }
            Ok(None) => {}
            Err(e) => k.chyba(&trf!("Trasy: změna se nepřijala – {}", e)),
        }
    }
    let doc = k.dokument().unwrap_or(doc);
    let t = trasa_panelu(&doc);
    k.panel_id(ID, hodnoty_trasy(&doc, t.as_ref()));
}

/// Výška terénu (profil povrchu pohledu) ve staničení `s`.
pub fn vyska_terenu(useky: &[Vec<[f64; 2]>], s: f64) -> Option<f64> {
    for u in useky {
        for w in u.windows(2) {
            let (a, b) = (w[0], w[1]);
            if s >= a[0].min(b[0]) - 1e-9 && s <= a[0].max(b[0]) + 1e-9 {
                if (b[0] - a[0]).abs() < 1e-12 {
                    return Some(a[1]);
                }
                return Some(a[1] + (b[1] - a[1]) * (s - a[0]) / (b[0] - a[0]));
            }
        }
    }
    None
}

#[cfg(test)]
mod testy {
    use super::*;

    fn n() -> Niveleta {
        Niveleta::nova("N", "", vec![
            VrcholNivelety::new(0.0, 100.0, 0.0),
            VrcholNivelety::new(100.0, 102.0, 2000.0),
            VrcholNivelety::new(200.0, 101.0, 0.0),
        ])
    }

    #[test]
    fn tabulka_r_l_k() {
        let n = n();
        let r = radky(&n);
        assert_eq!(r[1][3..6], ["2000.0 *".to_string(), "60.00".into(), "20.00".into()]);
        assert_eq!(r[0][3], "");
        assert_eq!(r[0][6], "20.00 ‰");
        // přepsaná délka L platí, hvězdička se při čtení ignoruje
        let mut rows = r.clone();
        rows[1][4] = "90".into();
        let v = nove_vrcholy(&n, 0, &Zmena::Tabulka(rows)).unwrap().unwrap();
        assert_eq!(v[1].zadani, Zadani::L(90.0));
        // beze změny nic – ani zaokrouhlení výšek na 3 desetinná místa
        assert!(nove_vrcholy(&n, 0, &Zmena::Tabulka(r)).unwrap().is_none());
        let mut n2 = n.clone();
        n2.vrcholy[0].z = 100.123_456_7;
        let mut rows = radky(&n2);
        rows[1][4] = "90".into();
        let v = nove_vrcholy(&n2, 0, &Zmena::Tabulka(rows)).unwrap().unwrap();
        assert_eq!(v[0].z, 100.123_456_7);
    }

    #[test]
    fn detail_sklon_a_typ() {
        let n = n();
        let v = nove_vrcholy(&n, 1, &Zmena::Pole("d_iz".into(), "-30".into())).unwrap().unwrap();
        assert!((v[2].z - 99.0).abs() < 1e-9);
        let v = nove_vrcholy(&n, 1, &Zmena::Typ(1)).unwrap().unwrap();
        assert_eq!(v[1].zadani, Zadani::L(60.0));
        let v = nove_vrcholy(&n, 1, &Zmena::Pole("d_hod".into(), "1500".into())).unwrap().unwrap();
        assert_eq!((v[1].zadani, v[1].r), (Zadani::R, 1500.0));
    }

    #[test]
    fn teren_ve_staniceni() {
        let u = vec![vec![[0.0, 10.0], [10.0, 20.0]], vec![[20.0, 5.0], [30.0, 5.0]]];
        assert_eq!(vyska_terenu(&u, 5.0), Some(15.0));
        assert_eq!(vyska_terenu(&u, 25.0), Some(5.0));
        assert_eq!(vyska_terenu(&u, 15.0), None);
    }
}
