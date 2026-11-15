//! Panel „Editor trasy“ v doku Open CAD Studia (API v7).
//!
//! Lišta nástrojů návrhu trasy: poloměr dalšího oblouku, nová trasa,
//! prodloužení, vložení a smazání vrcholu, oblouk ve vrcholu, vrácení
//! posledního vrcholu při kreslení, tabulka prvků a tabulka vrcholů.
//!
//! Tlačítka s příkazem spouští příkazy `TRA_…` (ty mají přístup k výkresu).
//! Změny polí chodí jako upozornění `PanelAction` bez přístupu k výkresu –
//! jen se zapamatují (poloměr, rozepsaná tabulka) a použijí v příkazu.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use ocs_plugin_api::dialog::{DialogControl, DialogValue, PanelDef};
use ocs_plugin_api::host::{Document as CadDocument, HostApi};
use trasy::preklad::tr;
use trasy::stanice::{cislo, staniceni, FormatStanice};
use trasy::trf;
use trasy::upravy::tabulka_prvku;

use crate::tra::objekt::{self, ObjektTrasy};

pub const ID: &str = "trasy_editor";
pub const PRIKAZ: &str = "TRA_EDITOR";

struct Stav {
    polomer: f64,
    /// Id trasy, se kterou editor pracuje.
    aktivni: Option<String>,
    /// Tabulka vrcholů, jak ji uživatel rozepsal (použije `TRA_EDITOR_POUZIT`).
    tabulka: Option<Vec<Vec<String>>>,
    /// Tabulka prvků, jak ji uživatel rozepsal (R a délky oblouků).
    prvky: Option<Vec<Vec<String>>>,
}

static STAV: Mutex<Stav> = Mutex::new(Stav { polomer: 100.0, aktivni: None, tabulka: None, prvky: None });
static OTEVREN: AtomicBool = AtomicBool::new(false);
/// Požadavek „vrátit poslední vrchol“ pro běžící kreslení.
static VRATIT: AtomicBool = AtomicBool::new(false);

fn stav<R>(f: impl FnOnce(&mut Stav) -> R) -> R {
    f(&mut STAV.lock().unwrap_or_else(|e| e.into_inner()))
}

pub fn otevren() -> bool {
    OTEVREN.load(Ordering::SeqCst)
}
pub fn polomer() -> f64 {
    stav(|s| s.polomer)
}
pub fn aktivni() -> Option<String> {
    stav(|s| s.aktivni.clone())
}
pub fn nastav_aktivni(id: &str) {
    stav(|s| {
        if s.aktivni.as_deref() != Some(id) {
            s.tabulka = None;
            s.prvky = None;
        }
        s.aktivni = Some(id.to_string());
    });
}
pub fn vezmi_tabulku() -> Option<Vec<Vec<String>>> {
    stav(|s| s.tabulka.take())
}
pub fn vezmi_prvky() -> Option<Vec<Vec<String>>> {
    stav(|s| s.prvky.take())
}
/// Má kreslení vrátit poslední vrchol? (jednou)
pub fn vezmi_vratit() -> bool {
    VRATIT.swap(false, Ordering::SeqCst)
}

fn tlacitko(id: &str, label: &str, prikaz: Option<&str>) -> DialogControl {
    DialogControl::Button { id: id.into(), label: label.into(), action: prikaz.map(Into::into), align_h: None, width: None }
}

fn radek(c: Vec<DialogControl>) -> DialogControl {
    DialogControl::Row { controls: c, align_h: None, align_v: None }
}

pub fn definice() -> PanelDef {
    OTEVREN.store(true, Ordering::SeqCst);
    PanelDef::new(ID, tr("Editor trasy"))
        .with_command(PRIKAZ)
        .with_width(340.0)
        .with_control(DialogControl::Text { id: "trasa".into(), label: tr("Upravovaná trasa").into(), default: String::new() })
        .with_control(DialogControl::Number {
            id: "r".into(),
            label: tr("Poloměr dalšího oblouku R [m]").into(),
            default: polomer(),
            min: Some(0.0),
            max: None,
            step: Some(10.0),
        })
        .with_control(radek(vec![
            tlacitko("nova", tr("Nová trasa"), Some("TRA_KRESLIT")),
            tlacitko("prodlouzit", tr("Prodloužit"), Some("TRA_PRODLOUZIT")),
            tlacitko("zpet", tr("Vrátit vrchol"), None),
        ]))
        .with_control(radek(vec![
            tlacitko("vlozit", tr("Vložit vrchol"), Some("TRA_VLOZIT_VRCHOL")),
            tlacitko("smazat", tr("Smazat vrchol"), Some("TRA_SMAZAT_VRCHOL")),
            tlacitko("oblouk", tr("Oblouk ve vrcholu"), Some("TRA_OBLOUK")),
        ]))
        .with_control(radek(vec![tlacitko("bez_oblouku", tr("Smazat oblouk"), Some("TRA_BEZ_OBLOUKU"))]))
        .with_control(DialogControl::InfoText {
            text: tr("Úpravy myší: klikněte na trasu – čtverečky posouvají vrcholy, kolečka mění poloměr (chyťte kolečko, napište R a Enter = přesný poloměr), trojúhelníky uprostřed přímých přidají vrchol (s R z pole výše). Kreslení: klikejte vrcholy, oblouk dostane poloměr R zadaný v okamžiku kliknutí. Enter = hotovo, Esc = zrušit. Vložit/Smazat vrchol a Oblouk ve vrcholu pracují s trasou nejblíž kliknutí; Oblouk ve vrcholu: klik na vrchol, pak napište R do příkazového řádku a Enter (jen Enter = R z pole výše); Smazat oblouk = klik na vrchol.").into(),
            align_h: None,
        })
        .with_control(DialogControl::Table {
            id: "prvky".into(),
            label: Some(tr("Prvky trasy – R nebo délku oblouku přepište a klikněte Použít změny").into()),
            headers: vec![tr("Prvek").into(), tr("Od").into(), tr("Do").into(), tr("Délka").into(), "R".into()],
            rows: Vec::new(),
            allow_add: false,
            allow_delete: false,
        })
        .with_control(DialogControl::Table {
            id: "vrcholy".into(),
            label: Some(tr("Vrcholy (X, Y, R) – přepište a klikněte Použít změny").into()),
            headers: vec!["X".into(), "Y".into(), "R".into()],
            rows: Vec::new(),
            allow_add: true,
            allow_delete: true,
        })
        .with_control(radek(vec![
            tlacitko("pouzit", tr("Použít změny"), Some("TRA_EDITOR_POUZIT")),
            tlacitko("vybrat", tr("Jiná trasa"), Some("TRA_EDITOR_VYBRAT")),
            tlacitko("vlastnosti", tr("Vlastnosti"), Some("TRA_VLASTNOSTI")),
        ]))
}

/// Trasa, kterou editor ukazuje: aktivní, jinak první ve výkrese.
pub fn trasa_editoru(doc: &CadDocument) -> Option<ObjektTrasy> {
    let vse = objekt::trasy(doc);
    let id = aktivni();
    vse.iter().find(|t| Some(&t.def.id) == id.as_ref()).cloned().or_else(|| vse.into_iter().next())
}

/// Hodnoty panelu (bez pole poloměru – to patří uživateli).
pub fn hodnoty(doc: &CadDocument) -> HashMap<String, DialogValue> {
    hodnoty_trasy(trasa_editoru(doc).as_ref())
}

/// Hodnoty panelu pro danou trasu (bez stahování výkresu).
pub fn hodnoty_trasy(t: Option<&ObjektTrasy>) -> HashMap<String, DialogValue> {
    stav(|s| {
        s.tabulka = None;
        s.prvky = None;
    });
    let mut m = HashMap::new();
    let Some(t) = t else {
        m.insert("trasa".into(), DialogValue::Text(tr("žádná – nakreslete Novou trasu").into()));
        m.insert("prvky".into(), DialogValue::Table(Vec::new()));
        m.insert("vrcholy".into(), DialogValue::Table(Vec::new()));
        return m;
    };
    let h = objekt::hodnoty(&t.def);
    let f = FormatStanice::z_kodu(h.text("format"));
    let vrcholy = t.vrcholy();
    match t.geometrie() {
        Ok(g) => {
            m.insert(
                "trasa".into(),
                DialogValue::Text(trf!("{} – {}, délka {} m", t.def.id, t.def.jmeno, cislo(g.delka(), 2, true))),
            );
            let radky = tabulka_prvku(&g)
                .iter()
                .map(|r| {
                    vec![
                        if r.oblouk { tr("Oblouk") } else { tr("Přímá") }.to_string(),
                        staniceni(r.od, f, 2, true),
                        staniceni(r.do_, f, 2, true),
                        cislo(r.delka, 2, true),
                        if r.oblouk {
                            format!("{} {}", cislo(r.polomer.abs(), 2, true), if r.polomer > 0.0 { tr("L") } else { tr("P") })
                        } else {
                            String::new()
                        },
                    ]
                })
                .collect();
            m.insert("prvky".into(), DialogValue::Table(radky));
        }
        Err(e) => {
            m.insert("trasa".into(), DialogValue::Text(trf!("{} – {}: {}", t.def.id, t.def.jmeno, e)));
            m.insert("prvky".into(), DialogValue::Table(Vec::new()));
        }
    }
    let rv = vrcholy.iter().map(|v| vec![cislo(v.x, 3, false), cislo(v.y, 3, false), cislo(v.r, 2, false)]).collect();
    m.insert("vrcholy".into(), DialogValue::Table(rv));
    m
}

/// Obnoví panel (když je otevřený) – z příkazu, který má výkres.
pub fn obnov(host: &mut dyn HostApi, doc: &CadDocument) {
    if otevren() {
        host.update_panel(ID, hodnoty(doc));
    }
}

/// Změna pole v panelu (upozornění bez přístupu k výkresu).
pub fn akce(control_id: &str, _action: &str, value: Option<DialogValue>) {
    match (control_id, value) {
        ("r", Some(DialogValue::Number(x))) => stav(|s| s.polomer = x.max(0.0)),
        ("r", Some(DialogValue::Text(t))) => {
            if let Ok(x) = t.trim().replace(',', ".").parse::<f64>() {
                stav(|s| s.polomer = x.max(0.0));
            }
        }
        ("vrcholy", Some(DialogValue::Table(t))) => stav(|s| s.tabulka = Some(t)),
        ("prvky", Some(DialogValue::Table(t))) => stav(|s| s.prvky = Some(t)),
        ("zpet", _) => VRATIT.store(true, Ordering::SeqCst),
        _ => {}
    }
}
