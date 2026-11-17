//! Trasy a podélné profily (dříve samostatný plugin Trasy, teď součást Terénu).
//!
//! Trasa jako objekt výkresu, editor trasy, úchopy, podélný profil s pásy
//! a niveletou. Výpočty dělá knihovna `trasy`; tady se čte a zapisuje výkres.
//! Terén předává příkazy `TRA_…`, notifikace a události úchopů se značkou
//! [`uchopy::ZNACKA`].

pub mod editor;
pub mod objekt;
pub mod pohled;
pub mod povrchy;
pub mod prikazy;
pub mod prospektor;
pub mod uchopy;

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::Ordering;

use ocs_plugin_api::dialog::{DialogControl, DialogDef, DialogValue};
use ocs_plugin_api::host::{EntityType, Handle, HostApi, HostNotification};
use ocs_plugin_api::ribbon::{IconKind, ModuleEvent, RibbonGroup, RibbonItem, ToolDef};

use trasy::geometrie::{geometrie, z_polylinie, Chyba, Vrchol};
use trasy::upravy::{nejblizsi_usek, nejblizsi_vrchol, smaz_vrchol, vloz_vrchol, vzdalenost};
use trasy::preklad::{tr, tr_dyn};
use trasy::stanice::{cislo, FormatStanice};
use trasy::styl::vysledne;
use trasy::trasa::{vestavene_styly, volne_id, DefiniceTrasy, SCHEMA, STYL_ZAKLAD};
use trasy::trf;

use crate::vykres::{self, PresHost, Vykres};
use objekt::ObjektTrasy;

// ── Pás karet ───────────────────────────────────────────────────────────────

macro_rules! ikona {
    ($n:literal) => {
        IconKind::Svg(include_bytes!(concat!("../../assets/", $n, ".svg")))
    };
}

fn tool(id: &'static str, label: &'static str, icon: IconKind) -> ToolDef {
    ToolDef { id, label, icon, event: ModuleEvent::Command(id.to_string()) }
}

/// Skupiny pásu karet pro trasy a profily (na záložce Terén).
pub fn skupiny() -> Vec<RibbonGroup> {
    vec![
        RibbonGroup {
            title: tr("Trasy"),
            tools: vec![
                RibbonItem::LargeTool(tool("TRA_EDITOR", tr("Editor trasy"), ikona!("tra_editor"))),
                RibbonItem::LargeDropdown {
                    id: "TRA_DD_NOVA",
                    label: tr("Nová trasa"),
                    icon: ikona!("tra_kreslit"),
                    items: vec![
                        ("TRA_KRESLIT", tr("Kreslit vrcholy"), ikona!("tra_kreslit")),
                        ("TRA_Z_POLYLINIE", tr("Z polylinie"), ikona!("tra_z_polylinie")),
                    ],
                    default: "TRA_KRESLIT",
                },
                RibbonItem::LabeledDropdown {
                    id: "TRA_DD_VRCHOLY",
                    label: tr("Vrcholy trasy"),
                    icon: ikona!("tra_vlozit"),
                    items: vec![
                        ("TRA_VLOZIT_VRCHOL", tr("Vložit vrchol"), ikona!("tra_vlozit")),
                        ("TRA_SMAZAT_VRCHOL", tr("Smazat vrchol"), ikona!("tra_smazat")),
                        ("TRA_OBLOUK", tr("Oblouk ve vrcholu"), ikona!("tra_oblouk")),
                        ("TRA_BEZ_OBLOUKU", tr("Smazat oblouk"), ikona!("tra_bez_oblouku")),
                        ("TRA_PRODLOUZIT", tr("Prodloužit trasu"), ikona!("tra_prodlouzit")),
                    ],
                    default: "TRA_VLOZIT_VRCHOL",
                },
                RibbonItem::LabeledTool(tool("TRA_VLASTNOSTI", tr("Vlastnosti trasy"), ikona!("tra_vlastnosti"))),
                RibbonItem::LabeledTool(tool("TRA_UCHOPY", tr("Úchopy tras zap/vyp"), ikona!("tra_uchopy"))),
            ],
        },
        RibbonGroup {
            title: tr("Podélný profil"),
            tools: vec![
                RibbonItem::LargeTool(tool("TRA_POHLED", tr("Podélný profil"), ikona!("tra_profil"))),
                RibbonItem::LargeTool(tool("TRA_NIVELETA", tr("Niveleta"), ikona!("tra_niveleta"))),
                RibbonItem::LabeledTool(tool("TRA_NIVELETA_TABULKA", tr("Vrcholy nivelety"), ikona!("tra_vlastnosti"))),
                RibbonItem::LabeledTool(tool("TRA_NIVELETA_SMAZAT", tr("Smazat vrchol nivelety"), ikona!("tra_smazat"))),
                RibbonItem::LabeledTool(tool("TRA_PREPOCITAT", tr("Překreslit trasy a profily"), ikona!("tra_prepocitat"))),
            ],
        },
    ]
}

// ── Pomocné ─────────────────────────────────────────────────────────────────

fn handle_z_textu(t: &str) -> Option<Handle> {
    u64::from_str_radix(t.trim(), 16).ok().map(Handle::new)
}

fn nova_definice(doc: &ocs_plugin_api::host::Document, vrcholy: Vec<trasy::geometrie::Vrchol>) -> DefiniceTrasy {
    let stavajici = objekt::trasy(doc);
    let id = volne_id(stavajici.iter().map(|t| t.def.id.as_str()));
    let jmeno = trf!("Trasa {}", id.trim_start_matches('T'));
    DefiniceTrasy::nova(&id, &jmeno, vrcholy, STYL_ZAKLAD)
}

/// Vybraná trasa: z výběru, jinak spustí výběr kliknutím s daným příkazem.
fn vybrana_trasa(host: &mut dyn HostApi, arg: &str, prikaz: &'static str) -> Option<(ObjektTrasy, ocs_plugin_api::host::Document)> {
    let mut v = PresHost(host);
    let doc = match v.dokument() {
        Ok(d) => d,
        Err(e) => {
            v.chyba(&trf!("Trasy: {}", e));
            return None;
        }
    };
    let mut casti = arg.split_whitespace();
    let vyber: Vec<Handle> = match casti.next().and_then(handle_z_textu) {
        Some(h) => vec![h],
        None => v.vybrane(),
    };
    if let Some(t) = objekt::z_vyberu(&doc, &vyber).into_iter().next() {
        return Some((t, doc));
    }
    // klik mohl trefit něco pod trasou (třeba původní polylinii) – vezme se
    // nejbližší trasa k místu kliknutí
    let bod: Vec<f64> = casti.filter_map(|c| c.parse().ok()).collect();
    if bod.len() == 2 {
        if let Some(t) = objekt::nejblizsi(&doc, [bod[0], bod[1]]) {
            return Some((t, doc));
        }
    }
    if !arg.is_empty() {
        v.chyba(tr("Trasy: vybraný objekt není trasa."));
        return None;
    }
    let vse = objekt::trasy(&doc);
    if vse.is_empty() {
        v.chyba(tr("Trasy: ve výkrese není žádná trasa."));
        return None;
    }
    if prikaz != "TRA_EDITOR_VYBRAT" {
        // Dialog ani kreslení nejde spustit po klikacím výběru (OCS je zruší),
        // proto: trasa z editoru, nebo jediná trasa ve výkrese.
        let id = editor::aktivni();
        if let Some(t) = vse.iter().find(|t| Some(&t.def.id) == id.as_ref()).or(if vse.len() == 1 { vse.first() } else { None }) {
            return Some((t.clone(), doc));
        }
        v.info(tr("Trasy: nejdřív klikněte na trasu (vybere se) a pak příkaz spusťte znovu."));
        return None;
    }
    host.start_interactive(vykres::hlidany(Box::new(prikazy::VyberObjektu { vyzva: "Trasy: klikněte na trasu", prikaz })));
    None
}

// ── Příkazy ─────────────────────────────────────────────────────────────────

fn z_polylinie_cmd(host: &mut dyn HostApi, arg: &str) {
    let mut v = PresHost(host);
    let doc = match v.dokument() {
        Ok(d) => d,
        Err(e) => return v.chyba(&trf!("Trasy: {}", e)),
    };
    let kandidati: Vec<Handle> = match arg.split_whitespace().next().and_then(handle_z_textu) {
        Some(h) if !arg.is_empty() => vec![h],
        _ => v.vybrane(),
    };
    let pl = doc.model_space_entities().find_map(|e| match e {
        EntityType::LwPolyline(p) if kandidati.contains(&p.common.handle) => Some(p.clone()),
        _ => None,
    });
    // klik trefil něco jiného (třeba povrch pod polylinií) → polylinie nejblíž kliknutí
    let pl = pl.or_else(|| {
        let p = bod_z_argumentu(arg)?;
        doc.model_space_entities()
            .filter_map(|e| match e {
                EntityType::LwPolyline(l) if l.vertices.len() >= 2 => {
                    // vzdálenost ke skutečné ose (s oblouky), jinak ke spojnicím vrcholů
                    let body: Vec<([f64; 2], f64)> = l.vertices.iter().map(|x| ([x.location.x, x.location.y], x.bulge)).collect();
                    let d = z_polylinie(&body, l.is_closed)
                        .ok()
                        .and_then(|v| geometrie(&v, 0.0).ok().map(|g| vzdalenost(&v, Some(&g), p)))
                        .or_else(|| {
                            let v: Vec<Vrchol> = l.vertices.iter().map(|x| Vrchol::new(x.location.x, x.location.y, 0.0)).collect();
                            nejblizsi_usek(&v, p).map(|(_, d, _)| d)
                        })?;
                    Some((d, l.clone()))
                }
                _ => None,
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .filter(|(d, _)| *d < 15.0)
            .map(|(_, l)| l)
    });
    let Some(pl) = pl else {
        if !arg.is_empty() {
            return v.chyba(tr("Trasy: vybraný objekt není polylinie (2D křivka)."));
        }
        host.start_interactive(vykres::hlidany(Box::new(prikazy::VyberObjektu {
            vyzva: "Trasy: klikněte na polylinii osy",
            prikaz: "TRA_Z_POLYLINIE",
        })));
        return;
    };
    let body: Vec<([f64; 2], f64)> = pl.vertices.iter().map(|x| ([x.location.x, x.location.y], x.bulge)).collect();
    let vrcholy = match z_polylinie(&body, pl.is_closed) {
        Ok(x) => x,
        Err(e) => {
            let (t, a) = e.popis();
            let t = trasy::preklad::formatuj(tr(t), &a.iter().map(|s| s as &dyn std::fmt::Display).collect::<Vec<_>>());
            return v.chyba(&trf!("Trasy: z této polylinie trasu udělat nejde – {}", t));
        }
    };
    let def = nova_definice(&doc, vrcholy);
    v.zpet(tr("Trasa z polylinie"));
    match objekt::uloz(&mut v, Some(&doc), None, def.clone()) {
        Ok(h) => {
            v.vyber(vec![h]);
            if let Ok(d) = v.dokument() {
                if let Some(t) = objekt::trasy(&d).into_iter().find(|t| t.insert == h) {
                    v.info(&trf!("Trasy: vytvořena {}", objekt::popis(&t)));
                }
            }
            v.info(tr("Trasy: polylinie zůstala ve výkrese (můžete ji smazat nebo nechat jako podklad)."));
        }
        Err(e) => v.chyba(&trf!("Trasy: {}", e)),
    }
}

/// Otevře panel editoru (zaregistruje ho, když ještě není) a obnoví ho.
fn otevri_editor(host: &mut dyn HostApi) {
    if !editor::otevren() {
        host.register_panel(editor::definice());
    }
    host.toggle_panel(editor::ID, Some(true));
    if let Ok(doc) = PresHost(host).dokument() {
        editor::obnov(host, &doc);
    }
}

/// Po změně trasy: obnoví panel editoru a úchopy.
fn po_zmene(host: &mut dyn HostApi) {
    let Ok(doc) = PresHost(host).dokument() else { return };
    editor::obnov(host, &doc);
    if uchopy::ZAPNUTO.load(Ordering::SeqCst) {
        let _ = host.set_plugin_grips(crate::MANIFEST.id, crate::uchopy::spoj_trasy(uchopy::uchopy(&doc)));
    }
}

/// Uloží nové vrcholy trasy jedním krokem Zpět. Vrací true při úspěchu.
fn uloz_vrcholy(host: &mut dyn HostApi, doc: &ocs_plugin_api::host::Document, t: &ObjektTrasy, vrcholy: Vec<Vrchol>, krok: &str) -> bool {
    if let Err(e) = geometrie(&vrcholy, t.def.pocatek) {
        host.push_error(&trf!("Trasy: změna se nepřijala – {}", objekt::chyba_geometrie(e)));
        return false;
    }
    let mut def = t.def.clone();
    def.vrcholy = vrcholy;
    let mut v = PresHost(host);
    v.zpet(krok);
    match objekt::uloz(&mut v, Some(doc), Some(t), def) {
        Ok(_) => {
            editor::nastav_aktivni(&t.def.id);
            true
        }
        Err(e) => {
            v.chyba(&trf!("Trasy: {}", e));
            false
        }
    }
}

fn bod_z_argumentu(arg: &str) -> Option<[f64; 2]> {
    let c: Vec<f64> = arg.split_whitespace().filter_map(cti_cislo).collect();
    (c.len() >= 2).then(|| [c[c.len() - 2], c[c.len() - 1]])
}

/// Trasa nejblíž bodu (polygon nebo osa).
fn trasa_u_bodu(doc: &ocs_plugin_api::host::Document, p: [f64; 2]) -> Option<ObjektTrasy> {
    objekt::trasy(doc)
        .into_iter()
        .map(|t| {
            let v = t.vrcholy();
            let g = t.geometrie().ok();
            (vzdalenost(&v, g.as_ref(), p), t)
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, t)| t)
}

fn kreslit_cmd(host: &mut dyn HostApi) {
    otevri_editor(host);
    host.push_info(&trf!(
        "Trasy: klikejte vrcholy trasy. Poloměr dalšího oblouku je v panelu Editor trasy (teď R = {} m) a jde měnit i během kreslení.",
        editor::polomer()
    ));
    host.start_interactive(vykres::hlidany(Box::new(prikazy::KresleniTrasy::nova())));
}

fn prodlouzit_cmd(host: &mut dyn HostApi, arg: &str) {
    let t = if arg.is_empty() {
        match PresHost(host).dokument() {
            Ok(doc) => editor::aktivni().and_then(|id| objekt::trasy(&doc).into_iter().find(|t| t.def.id == id)),
            Err(_) => None,
        }
    } else {
        None
    };
    let t = match t {
        Some(t) => t,
        None => match vybrana_trasa(host, arg, "TRA_PRODLOUZIT") {
            Some((t, _)) => t,
            None => return,
        },
    };
    otevri_editor(host);
    editor::nastav_aktivni(&t.def.id);
    host.push_info(&trf!("Trasy: prodloužení trasy {} – klikejte další vrcholy od konce, Enter = hotovo.", t.def.jmeno));
    host.start_interactive(vykres::hlidany(Box::new(prikazy::KresleniTrasy::prodlouzeni(t.def.id.clone(), &t.vrcholy()))));
}

fn kreslit_hotovo(host: &mut dyn HostApi) {
    let Some((mut vrcholy, prodlouzena)) = prikazy::NAKRESLENO.lock().unwrap_or_else(|e| e.into_inner()).take() else { return };
    let mut v = PresHost(host);
    let doc = match v.dokument() {
        Ok(d) => d,
        Err(e) => return v.chyba(&trf!("Trasy: {}", e)),
    };
    // oblouky, které se nevejdou, dostanou poloměr 0 (vrchol zůstane jako lom)
    let mut bez_oblouku = Vec::new();
    loop {
        match geometrie(&vrcholy, 0.0) {
            Ok(_) => break,
            Err(Chyba::Prekryv { a, b }) => {
                let i = if vrcholy[b].r > 0.0 && b + 1 < vrcholy.len() { b } else { a };
                if vrcholy[i].r <= 0.0 {
                    return v.chyba(tr("Trasy: trasu nejde vytvořit."));
                }
                vrcholy[i].r = 0.0;
                bez_oblouku.push(i + 1);
            }
            Err(e) => return v.chyba(&trf!("Trasy: {}", objekt::chyba_geometrie(e))),
        }
    }
    let stara = prodlouzena.and_then(|id| objekt::trasy(&doc).into_iter().find(|t| t.def.id == id));
    let def = match &stara {
        Some(t) => {
            let mut d = t.def.clone();
            d.vrcholy = vrcholy;
            d
        }
        None => nova_definice(&doc, vrcholy),
    };
    let id = def.id.clone();
    v.zpet(if stara.is_some() { tr("Prodloužení trasy") } else { tr("Kreslení trasy") });
    match objekt::uloz(&mut v, Some(&doc), stara.as_ref(), def) {
        Ok(h) => {
            editor::nastav_aktivni(&id);
            v.vyber(vec![h]);
            if let Ok(d) = v.dokument() {
                if let Some(t) = objekt::trasy(&d).into_iter().find(|t| t.insert == h) {
                    v.info(&trf!("Trasy: {}", objekt::popis(&t)));
                }
            }
            if !bez_oblouku.is_empty() {
                let s: Vec<String> = bez_oblouku.iter().map(|i| i.to_string()).collect();
                v.info(&trf!(
                    "Trasy: ve vrcholech {} se oblouk nevešel, jsou bez oblouku – zmenšete R a použijte Oblouk ve vrcholu.",
                    s.join(", ")
                ));
            }
        }
        Err(e) => v.chyba(&trf!("Trasy: {}", e)),
    }
    po_zmene(host);
}

/// Společný průběh nástrojů vrcholu: bez bodu spustí klik, s bodem najde trasu.
fn nastroj_vrcholu(
    host: &mut dyn HostApi,
    arg: &str,
    prikaz: &'static str,
    vyzva: &'static str,
) -> Option<(ObjektTrasy, ocs_plugin_api::host::Document, [f64; 2])> {
    let Some(p) = bod_z_argumentu(arg) else {
        otevri_editor(host);
        host.start_interactive(vykres::hlidany(Box::new(prikazy::VyberBodu { vyzva, prikaz })));
        return None;
    };
    let doc = match PresHost(host).dokument() {
        Ok(d) => d,
        Err(e) => {
            host.push_error(&trf!("Trasy: {}", e));
            return None;
        }
    };
    match trasa_u_bodu(&doc, p) {
        Some(t) => Some((t, doc, p)),
        None => {
            host.push_error(tr("Trasy: ve výkrese není žádná trasa."));
            None
        }
    }
}

fn vlozit_vrchol_cmd(host: &mut dyn HostApi, arg: &str) {
    let Some((t, doc, p)) = nastroj_vrcholu(host, arg, "TRA_VLOZIT_VRCHOL", "Trasy: klikněte na tečnový polygon, kam vložit vrchol")
    else {
        return;
    };
    let Some((v, i)) = vloz_vrchol(&t.vrcholy(), p, editor::polomer()) else { return };
    if uloz_vrcholy(host, &doc, &t, v, tr("Vložení vrcholu")) {
        host.push_info(&trf!(
            "Trasy: do trasy {} vložen vrchol {} – posuňte ho úchopem (Úchopy zap/vyp), oblouk R = {} m se objeví, až vrchol vybočí z přímky.",
            t.def.jmeno,
            i + 1,
            editor::polomer()
        ));
    }
    po_zmene(host);
}

fn smazat_vrchol_cmd(host: &mut dyn HostApi, arg: &str) {
    let Some((t, doc, p)) = nastroj_vrcholu(host, arg, "TRA_SMAZAT_VRCHOL", "Trasy: klikněte na vrchol, který smazat") else {
        return;
    };
    let v = t.vrcholy();
    let Some((i, _)) = nejblizsi_vrchol(&v, p) else { return };
    let Some(n) = smaz_vrchol(&v, i) else {
        host.push_error(tr("Trasy: trasa musí mít aspoň dva vrcholy."));
        return;
    };
    if uloz_vrcholy(host, &doc, &t, n, tr("Smazání vrcholu")) {
        host.push_info(&trf!("Trasy: z trasy {} smazán vrchol {}.", t.def.jmeno, i + 1));
    }
    po_zmene(host);
}

fn oblouk_cmd(host: &mut dyn HostApi) {
    let doc = match PresHost(host).dokument() {
        Ok(d) => d,
        Err(e) => return host.push_error(&trf!("Trasy: {}", e)),
    };
    let trasy: Vec<(Handle, Vec<Vrchol>, f64)> =
        objekt::trasy(&doc).into_iter().map(|t| (t.insert, t.vrcholy(), t.def.pocatek)).collect();
    if trasy.is_empty() {
        return host.push_error(tr("Trasy: ve výkrese není žádná trasa."));
    }
    host.start_interactive(vykres::hlidany(Box::new(prikazy::ObloukVeVrcholu { trasy, vybrany: None })));
}

fn oblouk_nastav_cmd(host: &mut dyn HostApi, arg: &str) {
    let mut c = arg.split_whitespace();
    let (Some(h), Some(i), Some(r)) = (
        c.next().and_then(handle_z_textu),
        c.next().and_then(|x| x.parse::<usize>().ok()),
        c.next().and_then(cti_cislo),
    ) else {
        return;
    };
    nastav_polomer(host, h, i, r);
}

/// Nastaví vrcholu poloměr (z dialogu Oblouku ve vrcholu, nebo 0 u Smazat oblouk).
fn nastav_polomer(host: &mut dyn HostApi, h: Handle, i: usize, r: f64) {
    let doc = match PresHost(host).dokument() {
        Ok(d) => d,
        Err(e) => return host.push_error(&trf!("Trasy: {}", e)),
    };
    let Some(t) = objekt::trasy(&doc).into_iter().find(|t| t.insert == h) else { return };
    let mut v = t.vrcholy();
    if i == 0 || i + 1 >= v.len() {
        return;
    }
    v[i].r = r.max(0.0);
    let krok = if r > 0.0 { tr("Oblouk ve vrcholu") } else { tr("Smazání oblouku") };
    if uloz_vrcholy(host, &doc, &t, v, krok) {
        if r > 0.0 {
            host.push_info(&trf!("Trasy: vrchol {} trasy {} má poloměr R = {} m.", i + 1, t.def.jmeno, r));
        } else {
            host.push_info(&trf!("Trasy: ve vrcholu {} trasy {} už není oblouk.", i + 1, t.def.jmeno));
        }
    }
    po_zmene(host);
}

fn bez_oblouku_cmd(host: &mut dyn HostApi, arg: &str) {
    let Some((t, _doc, p)) = nastroj_vrcholu(host, arg, "TRA_BEZ_OBLOUKU", "Trasy: klikněte na vrchol, ve kterém zrušit oblouk") else {
        return;
    };
    let v = t.vrcholy();
    let Some((i, _)) = nejblizsi_vrchol(&v, p) else { return };
    if i == 0 || i + 1 == v.len() || v[i].r <= 0.0 {
        host.push_info(tr("Trasy: v tomto vrcholu oblouk není."));
        return;
    }
    nastav_polomer(host, t.insert, i, 0.0);
}

fn editor_pouzit_cmd(host: &mut dyn HostApi) {
    let prvky = editor::vezmi_prvky();
    let vrcholy = editor::vezmi_tabulku();
    if prvky.is_none() && vrcholy.is_none() {
        host.push_info(tr("Trasy: v tabulkách se nic nezměnilo."));
        return;
    }
    let doc = match PresHost(host).dokument() {
        Ok(d) => d,
        Err(e) => return host.push_error(&trf!("Trasy: {}", e)),
    };
    let Some(t) = editor::trasa_editoru(&doc) else { return };
    let Some(radky) = vrcholy else {
        // jen tabulka prvků: nové poloměry oblouků
        let Some(radky) = prvky else { return };
        let Ok(g) = t.geometrie() else { return };
        match trasy::upravy::z_tabulky_prvku(&t.vrcholy(), &g, &radky) {
            Ok(v) => {
                if uloz_vrcholy(host, &doc, &t, v, tr("Úprava oblouků trasy")) {
                    host.push_info(&trf!("Trasy: oblouky trasy {} upraveny.", t.def.jmeno));
                }
            }
            Err(0) => host.push_error(tr("Trasy: tabulka prvků už neodpovídá trase – nic se nezměnilo.")),
            Err(k) => host.push_error(&trf!("Trasy: řádek {} tabulky prvků nemá platné R nebo délku – nic se nezměnilo.", k)),
        }
        po_zmene(host);
        return;
    };
    let mut nove = Vec::new();
    for (i, r) in radky.iter().enumerate() {
        if r.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        let x = r.first().and_then(|c| cti_cislo(c));
        let y = r.get(1).and_then(|c| cti_cislo(c));
        let rr = r.get(2).and_then(|c| cti_cislo(c)).unwrap_or(0.0);
        match (x, y) {
            (Some(x), Some(y)) => nove.push(Vrchol::new(x, y, rr)),
            _ => {
                host.push_error(&trf!("Trasy: řádek {} tabulky vrcholů nemá platná čísla X a Y – nic se nezměnilo.", i + 1));
                return;
            }
        }
    }
    if uloz_vrcholy(host, &doc, &t, nove, tr("Úprava vrcholů trasy")) {
        host.push_info(&trf!("Trasy: vrcholy trasy {} upraveny.", t.def.jmeno));
    }
    po_zmene(host);
}

fn editor_vybrat_cmd(host: &mut dyn HostApi, arg: &str) {
    let Some((t, doc)) = vybrana_trasa(host, arg, "TRA_EDITOR_VYBRAT") else { return };
    editor::nastav_aktivni(&t.def.id);
    otevri_editor(host);
    editor::obnov(host, &doc);
}

/// Hodnota stylu bez výjimek objektu (pro porovnání, co je výjimka).
fn hodnota_stylu(styl: &str, klic: &str) -> String {
    vysledne(SCHEMA, &vestavene_styly(), styl, &BTreeMap::new()).text(klic).to_string()
}

fn cti_cislo(t: &str) -> Option<f64> {
    t.trim().replace(',', ".").replace(' ', "").parse().ok()
}

fn vlastnosti_cmd(host: &mut dyn HostApi, arg: &str) {
    let Some((t, _doc)) = vybrana_trasa(host, arg, "TRA_VLASTNOSTI") else { return };
    let h = objekt::hodnoty(&t.def);
    let styly = vestavene_styly();
    let i_stylu = styly.iter().position(|s| s.id == t.def.styl).unwrap_or(0);
    let formaty = FormatStanice::VSE;
    let i_formatu = formaty.iter().position(|f| f.kod() == h.text("format")).unwrap_or(0);
    let oznac = |k: &str, popis: &'static str| -> String {
        if h.vyjimky.iter().any(|x| x == k) { format!("{} ●", tr(popis)) } else { tr(popis).to_string() }
    };
    let vrcholy = t.vrcholy();
    let radky: Vec<Vec<String>> =
        vrcholy.iter().map(|x| vec![cislo(x.x, 3, false), cislo(x.y, 3, false), cislo(x.r, 3, false)]).collect();
    let mut d = DialogDef::new(trf!("Vlastnosti trasy {}", t.def.jmeno))
        .with_control(DialogControl::Text { id: "jmeno".into(), label: tr("Název").into(), default: t.def.jmeno.clone() })
        .with_control(DialogControl::Number {
            id: "pocatek".into(),
            label: tr("Počáteční staničení [m]").into(),
            default: t.def.pocatek,
            min: None,
            max: None,
            step: Some(1.0),
        })
        .with_control(DialogControl::Choice {
            id: "styl".into(),
            label: tr("Styl").into(),
            options: styly.iter().map(|s| tr_dyn(&s.nazev)).collect(),
            default_index: i_stylu,
        })
        .with_control(DialogControl::Fields {
            id: "vzhled".into(),
            title: Some(tr("Vzhled jen této trasy (● = výjimka proti stylu)").into()),
            collapsed: false,
            controls: vec![
                DialogControl::Number {
                    id: "krok_hlavni".into(),
                    label: oznac("krok_hlavni", "Krok hlavního staničení [m]"),
                    default: h.cislo("krok_hlavni"),
                    min: Some(0.0),
                    max: None,
                    step: Some(10.0),
                },
                DialogControl::Number {
                    id: "krok_vedlejsi".into(),
                    label: oznac("krok_vedlejsi", "Krok vedlejšího staničení [m] (0 = nekreslit)"),
                    default: h.cislo("krok_vedlejsi"),
                    min: Some(0.0),
                    max: None,
                    step: Some(5.0),
                },
                DialogControl::Number {
                    id: "vyska_textu".into(),
                    label: oznac("vyska_textu", "Výška textu [mm na papíře]"),
                    default: h.cislo("vyska_textu"),
                    min: Some(0.5),
                    max: Some(20.0),
                    step: Some(0.5),
                },
                DialogControl::Choice {
                    id: "format".into(),
                    label: oznac("format", "Formát staničení"),
                    options: formaty.iter().map(|f| f.ukazka().to_string()).collect(),
                    default_index: i_formatu,
                },
                DialogControl::Checkbox {
                    id: "popis_body".into(),
                    label: oznac("popis_body", "Popisovat hlavní body (ZÚ, KÚ, TK, KT)"),
                    default: h.ano("popis_body"),
                },
                DialogControl::Checkbox {
                    id: "popis_vrcholy".into(),
                    label: oznac("popis_vrcholy", "Popisovat vrcholy (V) a poloměry"),
                    default: h.ano("popis_vrcholy"),
                },
                DialogControl::Checkbox {
                    id: "polygon".into(),
                    label: oznac("polygon", "Kreslit tečnový polygon"),
                    default: h.ano("polygon"),
                },
                DialogControl::Checkbox { id: "na_styl".into(), label: tr("Vrátit vzhled na styl (zrušit výjimky)").into(), default: false },
            ],
        })
        .with_control(DialogControl::Table {
            id: "vrcholy".into(),
            label: Some(tr("Vrcholy tečnového polygonu (souřadnice výkresu; R = poloměr oblouku, 0 = bez oblouku)").into()),
            headers: vec!["X".into(), "Y".into(), "R".into()],
            rows: radky,
            allow_add: true,
            allow_delete: true,
        })
        .submit_label(tr("Použít"))
        .cancel_label(tr("Zrušit"))
        .width(620.0);
    if let Ok(g) = t.geometrie() {
        d = d.with_control(DialogControl::InfoText {
            text: trf!("Délka trasy {} m, oblouků {}.", cislo(g.delka(), 2, true), g.prvky.iter().filter(|(_, p)| p.je_oblouk()).count()),
            align_h: None,
        });
    }
    let prikaz = format!("TRA_VLASTNOSTI_HOTOVO {:X}", t.insert.value());
    host.start_interactive(vykres::hlidany(Box::new(prikazy::DialogPrikaz::new(d, prikaz))));
}

/// Použije odpověď dialogu Vlastnosti trasy (trasa podle vložení v `arg`).
fn vlastnosti_hotovo(host: &mut dyn HostApi, arg: &str) {
    let Some(odp) = prikazy::vezmi_odpoved() else { return };
    let doc = match PresHost(host).dokument() {
        Ok(d) => d,
        Err(e) => return host.push_error(&trf!("Trasy: {}", e)),
    };
    let Some(h) = handle_z_textu(arg) else { return };
    let Some(t) = objekt::trasy(&doc).into_iter().find(|t| t.insert == h) else {
        return host.push_error(tr("Trasy: vybraný objekt není trasa."));
    };
    let h = objekt::hodnoty(&t.def);
    let styly = vestavene_styly();
    let formaty = FormatStanice::VSE;
    let vrcholy = t.vrcholy();
    let mut def = t.def.clone();
    if let Some(DialogValue::Text(s)) = odp.get("jmeno") {
        if !s.trim().is_empty() {
            def.jmeno = s.trim().to_string();
        }
    }
    if let Some(DialogValue::Number(x)) = odp.get("pocatek") {
        def.pocatek = *x;
    }
    if let Some(DialogValue::Choice { index, .. }) = odp.get("styl") {
        if let Some(s) = styly.get(*index) {
            def.styl = s.id.clone();
        }
    }
    // pole se mohou vrátit ploše i ve skupině Fields
    let mut pole: HashMap<String, DialogValue> = HashMap::new();
    for (k, x) in &odp {
        if let DialogValue::Fields(m) = x {
            pole.extend(m.clone());
        } else {
            pole.insert(k.clone(), x.clone());
        }
    }
    let na_styl = matches!(pole.get("na_styl"), Some(DialogValue::Bool(true)));
    if na_styl {
        def.vyjimky.clear();
    } else {
        let mut nastav = |k: &str, hodnota: String| {
            if hodnota_stylu(&def.styl, k) == hodnota {
                def.vyjimky.remove(k);
            } else {
                def.vyjimky.insert(k.to_string(), hodnota);
            }
        };
        for k in ["krok_hlavni", "krok_vedlejsi", "vyska_textu"] {
            if let Some(DialogValue::Number(x)) = pole.get(k) {
                let puvodni = h.cislo(k);
                if (x - puvodni).abs() > 1e-9 || t.def.vyjimky.contains_key(k) {
                    nastav(k, cislo(*x, 3, false).trim_end_matches('0').trim_end_matches('.').to_string());
                }
            }
        }
        if let Some(DialogValue::Choice { index, .. }) = pole.get("format") {
            if let Some(f) = formaty.get(*index) {
                nastav("format", f.kod().to_string());
            }
        }
        for k in ["popis_body", "popis_vrcholy", "polygon"] {
            if let Some(DialogValue::Bool(b)) = pole.get(k) {
                nastav(k, if *b { "1" } else { "0" }.to_string());
            }
        }
    }
    let mut vrcholy = vrcholy;
    if let Some(DialogValue::Table(rows)) = odp.get("vrcholy").or_else(|| pole.get("vrcholy")) {
        let mut nove = Vec::new();
        for (i, r) in rows.iter().enumerate() {
            if r.iter().all(|c| c.trim().is_empty()) {
                continue;
            }
            let x = r.first().and_then(|c| cti_cislo(c));
            let y = r.get(1).and_then(|c| cti_cislo(c));
            let rr = r.get(2).and_then(|c| cti_cislo(c)).unwrap_or(0.0);
            match (x, y) {
                (Some(x), Some(y)) => nove.push(trasy::geometrie::Vrchol::new(x, y, rr)),
                _ => {
                    host.push_error(&trf!("Trasy: řádek {} tabulky vrcholů nemá platná čísla X a Y – nic se nezměnilo.", i + 1));
                    return;
                }
            }
        }
        vrcholy = nove;
    }
    def.vrcholy = vrcholy;
    if let Err(e) = geometrie(&def.vrcholy, def.pocatek) {
        host.push_error(&trf!("Trasy: změna se nepřijala – {}", objekt::chyba_geometrie(e)));
        return;
    }
    let mut v = PresHost(host);
    v.zpet(tr("Vlastnosti trasy"));
    match objekt::uloz(&mut v, Some(&doc), Some(&t), def) {
        Ok(h) => {
            v.vyber(vec![h]);
            if let Ok(d) = v.dokument() {
                if let Some(t) = objekt::trasy(&d).into_iter().find(|t| t.insert == h) {
                    v.info(&trf!("Trasy: {}", objekt::popis(&t)));
                }
                if uchopy::ZAPNUTO.load(Ordering::SeqCst) {
                    let _ = host.set_plugin_grips(crate::MANIFEST.id, crate::uchopy::spoj_trasy(uchopy::uchopy(&d)));
                }
            }
        }
        Err(e) => v.chyba(&trf!("Trasy: {}", e)),
    }
}

/// Volby dialogu nového pohledu: id tras a povrchů v pořadí nabídek.
static VOLBY_POHLEDU: std::sync::Mutex<Option<(Vec<String>, Vec<String>)>> = std::sync::Mutex::new(None);
/// Šablona posledního nového pohledu (výchozí volba v dialogu).
static POSLEDNI_SABLONA: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

fn pohled_cmd(host: &mut dyn HostApi) {
    let mut v = PresHost(host);
    let doc = match v.dokument() {
        Ok(d) => d,
        Err(e) => return v.chyba(&trf!("Trasy: {}", e)),
    };
    let trasy = objekt::trasy(&doc);
    if trasy.is_empty() {
        return v.chyba(tr("Trasy: ve výkrese není žádná trasa."));
    }
    let povrchy = povrchy::povrchy(&doc);
    if povrchy.is_empty() {
        return v.chyba(tr("Trasy: ve výkrese není žádný povrch Terénu – profil není z čeho spočítat."));
    }
    let vybrana = objekt::z_vyberu(&doc, &v.vybrane()).into_iter().next().map(|t| t.def.id).or_else(editor::aktivni);
    let i_trasy = vybrana.and_then(|id| trasy.iter().position(|t| t.def.id == id)).unwrap_or(0);
    *VOLBY_POHLEDU.lock().unwrap_or_else(|e| e.into_inner()) =
        Some((trasy.iter().map(|t| t.def.id.clone()).collect(), povrchy.iter().map(|p| p.id.clone()).collect()));
    let styly = crate::styly::styly_profilu(&doc);
    let posledni = POSLEDNI_SABLONA.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let posledni = if posledni.is_empty() { "OBECNA".to_string() } else { posledni };
    let i_sablony = styly.iter().position(|x| x.id == posledni).unwrap_or(0);
    let d = DialogDef::new(tr("Nový podélný profil"))
        .with_control(DialogControl::Choice {
            id: "sablona".into(),
            label: tr("Styl profilu").into(),
            options: styly.iter().map(|x| trasy::preklad::tr_dyn(&x.jmeno)).collect(),
            default_index: i_sablony,
        })
        .with_control(DialogControl::Choice {
            id: "trasa".into(),
            label: tr("Trasa").into(),
            options: trasy.iter().map(|t| format!("{} – {}", t.def.id, t.def.jmeno)).collect(),
            default_index: i_trasy,
        })
        .with_control(DialogControl::Choice {
            id: "povrch".into(),
            label: tr("Povrch (Terén)").into(),
            options: povrchy.iter().map(|p| p.jmeno.clone()).collect(),
            default_index: 0,
        })
        .with_control(DialogControl::Number {
            id: "meritko".into(),
            label: tr("Měřítko délek 1:").into(),
            default: 1000.0,
            min: Some(10.0),
            max: Some(100000.0),
            step: Some(50.0),
        })
        .with_control(DialogControl::Number {
            id: "prevyseni".into(),
            label: tr("Převýšení (výšky ×)").into(),
            default: 10.0,
            min: Some(1.0),
            max: Some(100.0),
            step: Some(1.0),
        })
        .with_control(DialogControl::Number {
            id: "srovnavaci".into(),
            label: tr("Srovnávací rovina [m n. m.] (0 = automaticky)").into(),
            default: 0.0,
            min: None,
            max: None,
            step: Some(1.0),
        })
        .with_control(DialogControl::InfoText { text: tr("Po potvrzení klikněte do výkresu, kam umístit levý dolní roh pohledu.").into(), align_h: None })
        .submit_label(tr("Vložit"))
        .cancel_label(tr("Zrušit"))
        .width(440.0);
    host.start_interactive(vykres::hlidany(Box::new(prikazy::DialogPrikaz::s_klikem(
        d,
        "TRA_POHLED_VLOZ".into(),
        "Trasy: klikněte, kam umístit levý dolní roh pohledu",
    ))));
}

fn pohled_vloz_cmd(host: &mut dyn HostApi, arg: &str) {
    let Some(odp) = prikazy::vezmi_odpoved() else { return };
    let Some((trasy, povrchy)) = VOLBY_POHLEDU.lock().unwrap_or_else(|e| e.into_inner()).take() else { return };
    let Some(bod) = bod_z_argumentu(arg) else { return };
    let volba = |k: &str| match odp.get(k) {
        Some(DialogValue::Choice { index, .. }) => *index,
        _ => 0,
    };
    let cislo_z = |k: &str| match odp.get(k) {
        Some(DialogValue::Number(x)) => Some(*x),
        Some(DialogValue::Text(t)) => cti_cislo(t),
        _ => None,
    };
    let mut v = PresHost(host);
    let doc = match v.dokument() {
        Ok(d) => d,
        Err(e) => return v.chyba(&trf!("Trasy: {}", e)),
    };
    let (Some(trasa), Some(povrch)) = (trasy.get(volba("trasa")), povrchy.get(volba("povrch"))) else { return };
    let id = trasy::pohled::volne_id(pohled::pohledy(&doc).iter().map(|p| p.def.id.as_str()));
    let styly = crate::styly::styly_profilu(&doc);
    let styl = styly.get(volba("sablona")).cloned().unwrap_or_else(|| styly[0].clone());
    *POSLEDNI_SABLONA.lock().unwrap_or_else(|e| e.into_inner()) = styl.id.clone();
    let mut def = trasy::pohled::DefinicePohledu::podle_stylu(&id, trasa, povrch, &styl);
    def.meritko = cislo_z("meritko").filter(|x| *x >= 10.0).unwrap_or(1000.0);
    def.prevyseni = cislo_z("prevyseni").unwrap_or(10.0).max(0.5);
    def.srovnavaci = cislo_z("srovnavaci").filter(|x| *x != 0.0);
    v.zpet(tr("Podélný profil"));
    match pohled::uloz(&mut v, &doc, None, &def, bod) {
        Ok((_, s)) => v.info(&trf!(
            "Trasy: pohled {} – terén z {} trojúhelníků, úseků profilu {}, srovnávací rovina {} m, mřížka po {} m / {} m.",
            id,
            s.trojuhelniku,
            s.useku,
            cislo(s.osy.srovnavaci, 2, true),
            cislo(s.osy.krok_s, 0, true),
            cislo(s.osy.krok_z, 2, true)
        )),
        Err(e) => v.chyba(&trf!("Trasy: profil nejde nakreslit – {}", e)),
    }
}

/// Překreslí pohledy (po změně trasy nebo povrchu).
fn prekresli_pohledy(v: &mut dyn Vykres) -> usize {
    let Ok(doc) = v.dokument() else { return 0 };
    let mut n = 0;
    for p in pohled::pohledy(&doc) {
        let doc = v.dokument().unwrap_or_else(|_| doc.clone());
        match pohled::uloz(v, &doc, Some(&p), &p.def, p.vlozeni) {
            Ok(_) => n += 1,
            Err(e) => v.chyba(&trf!("Trasy: pohled {}: {}", p.def.id, e)),
        }
    }
    n
}

fn niveleta_cmd(host: &mut dyn HostApi) {
    let doc = match PresHost(host).dokument() {
        Ok(d) => d,
        Err(e) => return host.push_error(&trf!("Trasy: {}", e)),
    };
    let pohledy: Vec<prikazy::PohledKresleni> = pohled::pohledy(&doc)
        .into_iter()
        .filter_map(|p| p.def.osy.map(|osy| prikazy::PohledKresleni { id: p.def.id.clone(), vlozeni: p.vlozeni, osy }))
        .collect();
    if pohledy.is_empty() {
        return host.push_error(tr("Trasy: nejdřív vložte podélný profil (tlačítko Podélný profil), niveleta se kreslí do něj."));
    }
    host.start_interactive(vykres::hlidany(Box::new(prikazy::KresleniNivelety { pohledy, vybrany: None, body: Vec::new() })));
}

fn niveleta_hotovo(host: &mut dyn HostApi) {
    let Some((id_pohledu, mut vrcholy)) = prikazy::NAKRESLENA_NIVELETA.lock().unwrap_or_else(|e| e.into_inner()).take() else { return };
    let mut v = PresHost(host);
    let doc = match v.dokument() {
        Ok(d) => d,
        Err(e) => return v.chyba(&trf!("Trasy: {}", e)),
    };
    let Some(p) = pohled::pohledy(&doc).into_iter().find(|p| p.def.id == id_pohledu) else { return };
    let Some(t) = objekt::trasy(&doc).into_iter().find(|t| t.def.id == p.def.trasa) else { return };
    // oblouky, které se nevejdou, dostanou poloměr 0
    loop {
        let n = trasy::niveleta::Niveleta::nova("N1", "", vrcholy.clone());
        match n.kontrola() {
            Ok(()) => break,
            Err(trasy::niveleta::ChybaNivelety::Prekryv { a, b }) => {
                let i = if vrcholy[b].r > 0.0 && b + 1 < vrcholy.len() { b } else { a };
                if vrcholy[i].r <= 0.0 {
                    return v.chyba(tr("Trasy: niveletu nejde vytvořit."));
                }
                vrcholy[i].r = 0.0;
            }
            Err(e) => {
                let (txt, a) = e.popis();
                return v.chyba(&trf!(
                    "Trasy: {}",
                    trasy::preklad::formatuj(tr(txt), &a.iter().map(|x| x as &dyn std::fmt::Display).collect::<Vec<_>>())
                ));
            }
        }
    }
    let mut def = t.def.clone();
    let niv = trasy::niveleta::Niveleta::nova("N1", &trf!("Niveleta {}", t.def.jmeno), vrcholy);
    def.nivelety.retain(|n| n.id != "N1");
    def.nivelety.insert(0, niv);
    v.zpet(tr("Niveleta"));
    if !objekt::uloz_definici(&mut v, t.insert, &def) {
        return v.chyba(tr("Trasy: niveletu se nepodařilo uložit."));
    }
    let nova = ObjektTrasy { def: def.clone(), ..t.clone() };
    let n = pohled::prekresli_zastarale(&mut v, &doc, Some(&t.def.id), Some(&nova));
    v.hotovo();
    v.info(&trf!("Trasy: niveleta trasy {} uložena ({} vrcholů), překresleno pohledů: {}.", t.def.jmeno, def.nivelety[0].vrcholy.len(), n));
}

/// Uloží niveletu trasy a překreslí pohledy (výkres `doc` stažený před krokem Zpět).
fn uloz_niveletu_cmd(
    v: &mut PresHost,
    doc: &ocs_plugin_api::host::Document,
    t: &ObjektTrasy,
    vrcholy: Vec<trasy::niveleta::VrcholNivelety>,
    krok: &str,
) -> Result<usize, String> {
    let mut def = t.def.clone();
    let Some(n) = def.nivelety.first_mut() else { return Err(tr("trasa nemá niveletu").into()) };
    n.vrcholy = vrcholy;
    if let Err(e) = n.kontrola() {
        let (txt, a) = e.popis();
        return Err(trasy::preklad::formatuj(tr(txt), &a.iter().map(|x| x as &dyn std::fmt::Display).collect::<Vec<_>>()));
    }
    v.zpet(krok);
    if !objekt::uloz_definici(v, t.insert, &def) {
        return Err(tr("niveletu se nepodařilo uložit").into());
    }
    let nova = ObjektTrasy { def, ..t.clone() };
    let n = pohled::prekresli_zastarale(v, doc, Some(&t.def.id), Some(&nova));
    v.hotovo();
    Ok(n)
}

/// Trasa s niveletou pro příkazy nivelety: z vybraného pohledu nebo trasy,
/// jinak trasa z editoru, jinak jediná trasa s niveletou.
fn trasa_s_niveletou(host: &mut dyn HostApi, doc: &ocs_plugin_api::host::Document) -> Option<ObjektTrasy> {
    let vyber = host.selection();
    let trasy: Vec<ObjektTrasy> = objekt::trasy(doc).into_iter().filter(|t| !t.def.nivelety.is_empty()).collect();
    let id = pohled::pohledy(doc)
        .into_iter()
        .find(|p| vyber.contains(&p.insert))
        .map(|p| p.def.trasa)
        .or_else(|| trasy.iter().find(|t| vyber.contains(&t.insert)).map(|t| t.def.id.clone()))
        .or_else(editor::aktivni);
    id.and_then(|id| trasy.iter().find(|t| t.def.id == id).cloned()).or_else(|| (trasy.len() == 1).then(|| trasy[0].clone()))
}

fn niveleta_tabulka_cmd(host: &mut dyn HostApi) {
    let doc = match PresHost(host).dokument() {
        Ok(d) => d,
        Err(e) => return host.push_error(&trf!("Trasy: {}", e)),
    };
    let Some(t) = trasa_s_niveletou(host, &doc) else {
        return host.push_error(tr("Trasy: nejdřív klikněte na pohled profilu s niveletou (nebo niveletu nakreslete tlačítkem Niveleta)."));
    };
    let n = &t.def.nivelety[0];
    let radky: Vec<Vec<String>> = n.vrcholy.iter().map(|v| vec![cislo(v.s, 3, false), cislo(v.z, 3, false), cislo(v.r, 1, false)]).collect();
    let sklony: Vec<String> = n.sklony().iter().map(|i| format!("{} ‰", cislo(i * 1000.0, 2, true))).collect();
    let d = DialogDef::new(trf!("Vrcholy nivelety – trasa {}", t.def.jmeno))
        .with_control(DialogControl::Table {
            id: "vrcholy".into(),
            label: Some(tr("Vrcholy: staničení [m], výška [m n. m.], R zakružovacího oblouku [m] (0 = bez oblouku)").into()),
            headers: vec![tr("Staničení").into(), tr("Výška").into(), "R".into()],
            rows: radky,
            allow_add: true,
            allow_delete: true,
        })
        .with_control(DialogControl::InfoText { text: trf!("Sklony úseků: {}", sklony.join(" · ")), align_h: None })
        .submit_label(tr("Použít"))
        .cancel_label(tr("Zrušit"))
        .width(520.0);
    host.start_interactive(vykres::hlidany(Box::new(prikazy::DialogPrikaz::new(d, format!("TRA_NIVELETA_TABULKA_HOTOVO {}", t.def.id)))));
}

fn niveleta_tabulka_hotovo(host: &mut dyn HostApi, arg: &str) {
    let Some(odp) = prikazy::vezmi_odpoved() else { return };
    let Some(DialogValue::Table(rows)) = odp.get("vrcholy") else { return };
    let mut vrcholy = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        if r.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        match (r.first().and_then(|c| cti_cislo(c)), r.get(1).and_then(|c| cti_cislo(c))) {
            (Some(s), Some(z)) => vrcholy.push(trasy::niveleta::VrcholNivelety::new(s, z, r.get(2).and_then(|c| cti_cislo(c)).unwrap_or(0.0).max(0.0))),
            _ => return host.push_error(&trf!("Trasy: řádek {} nemá platné staničení a výšku – nic se nezměnilo.", i + 1)),
        }
    }
    vrcholy.sort_by(|a, b| a.s.total_cmp(&b.s));
    let mut v = PresHost(host);
    let doc = match v.dokument() {
        Ok(d) => d,
        Err(e) => return v.chyba(&trf!("Trasy: {}", e)),
    };
    let Some(t) = objekt::trasy(&doc).into_iter().find(|t| t.def.id == arg.trim()) else { return };
    match uloz_niveletu_cmd(&mut v, &doc, &t, vrcholy, tr("Vrcholy nivelety")) {
        Ok(n) => v.info(&trf!("Trasy: niveleta trasy {} upravena, překresleno pohledů: {}.", t.def.jmeno, n)),
        Err(e) => v.chyba(&trf!("Trasy: změna se nepřijala – {}", e)),
    }
}

fn niveleta_smazat_cmd(host: &mut dyn HostApi, arg: &str) {
    let Some(p) = bod_z_argumentu(arg) else {
        host.start_interactive(vykres::hlidany(Box::new(prikazy::VyberBodu {
            vyzva: "Trasy: klikněte v pohledu profilu na vrchol nivelety, který smazat",
            prikaz: "TRA_NIVELETA_SMAZAT",
        })));
        return;
    };
    let mut v = PresHost(host);
    let doc = match v.dokument() {
        Ok(d) => d,
        Err(e) => return v.chyba(&trf!("Trasy: {}", e)),
    };
    let Some(ph) = pohled::pohledy(&doc).into_iter().find(|x| x.obsahuje(p)) else {
        return v.chyba(tr("Trasy: klikněte do rámu pohledu profilu."));
    };
    let Some(t) = objekt::trasy(&doc).into_iter().find(|t| t.def.id == ph.def.trasa) else { return };
    let Some(n) = t.def.nivelety.first() else { return v.chyba(tr("Trasy: trasa tohoto pohledu nemá niveletu.")) };
    let i = n
        .vrcholy
        .iter()
        .enumerate()
        .filter_map(|(i, x)| ph.na_vykres(x.s, x.z).map(|q| (i, (q[0] - p[0]).hypot(q[1] - p[1]))))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|x| x.0);
    let Some(i) = i else { return };
    if n.vrcholy.len() <= 2 {
        return v.chyba(tr("Trasy: niveleta musí mít aspoň dva vrcholy."));
    }
    let mut vrcholy = n.vrcholy.clone();
    vrcholy.remove(i);
    if let Some(x) = vrcholy.first_mut() {
        x.r = 0.0;
    }
    if let Some(x) = vrcholy.last_mut() {
        x.r = 0.0;
    }
    match uloz_niveletu_cmd(&mut v, &doc, &t, vrcholy, tr("Smazání vrcholu nivelety")) {
        Ok(_) => v.info(&trf!("Trasy: z nivelety trasy {} smazán vrchol {}.", t.def.jmeno, i + 1)),
        Err(e) => v.chyba(&trf!("Trasy: změna se nepřijala – {}", e)),
    }
}

fn uchopy_cmd(host: &mut dyn HostApi) {
    let zap = !uchopy::ZAPNUTO.load(Ordering::SeqCst);
    uchopy::ZAPNUTO.store(zap, Ordering::SeqCst);
    let doc = match PresHost(host).dokument() {
        Ok(d) => d,
        Err(_) => host.document().clone(),
    };
    let g = uchopy::uchopy(&doc);
    let n = g.len();
    let _ = host.set_plugin_grips(crate::MANIFEST.id, crate::uchopy::spoj_trasy(g));
    if zap {
        host.push_info(&trf!(
            "Trasy: úchopy zapnuté ({}) – čtverečky posouvají vrcholy, kolečka uprostřed oblouků mění poloměr. Znovu stejným tlačítkem vypnete.",
            n
        ));
    } else {
        host.push_info(tr("Trasy: úchopy vypnuté."));
    }
}

fn prepocitat_cmd(host: &mut dyn HostApi) {
    let mut v = PresHost(host);
    let doc = match v.dokument() {
        Ok(d) => d,
        Err(e) => return v.chyba(&trf!("Trasy: {}", e)),
    };
    let vse = objekt::trasy(&doc);
    if vse.is_empty() {
        return v.info(tr("Trasy: ve výkrese není žádná trasa."));
    }
    v.zpet(tr("Překreslit trasy"));
    let mut ok = 0;
    for t in &vse {
        if t.meritko_jine {
            v.chyba(&trf!("Trasy: trasa {} má vložení se změněným měřítkem – měřítko se ignoruje.", t.def.jmeno));
        }
        let mut def = t.def.clone();
        def.vrcholy = t.vrcholy();
        let doc = v.dokument().unwrap_or_else(|_| doc.clone());
        match objekt::uloz(&mut v, Some(&doc), Some(t), def) {
            Ok(_) => ok += 1,
            Err(e) => v.chyba(&trf!("Trasy: {}: {}", t.def.jmeno, e)),
        }
    }
    v.info(&trf!("Trasy: překresleno tras: {}.", ok));
    let n = prekresli_pohledy(&mut v);
    if n > 0 {
        v.info(&trf!("Trasy: překresleno pohledů: {}.", n));
    }
    if uchopy::ZAPNUTO.load(Ordering::SeqCst) {
        if let Ok(d) = v.dokument() {
            let _ = host.set_plugin_grips(crate::MANIFEST.id, crate::uchopy::spoj_trasy(uchopy::uchopy(&d)));
        }
    }
}

fn seznam_cmd(host: &mut dyn HostApi) {
    let mut v = PresHost(host);
    let doc = match v.dokument() {
        Ok(d) => d,
        Err(e) => return v.chyba(&trf!("Trasy: {}", e)),
    };
    let vse = objekt::trasy(&doc);
    if vse.is_empty() {
        return v.info(tr("Trasy: ve výkrese není žádná trasa."));
    }
    v.info(&trf!("Trasy ve výkrese: {}", vse.len()));
    for t in &vse {
        v.vystup(&format!("  {}", objekt::popis(t)));
    }
}

fn jazyk_cmd(host: &mut dyn HostApi, arg: &str) {
    let j = match arg.trim().to_ascii_lowercase().as_str() {
        "cs" | "en" | "auto" => arg.trim().to_ascii_lowercase(),
        _ => return host.push_info(tr("Trasy: použití TRA_JAZYK cs | en | auto (platí po restartu Open CAD Studia).")),
    };
    let Some(base) = std::env::var_os("APPDATA").map(std::path::PathBuf::from) else { return };
    let dir = base.join("OpenCADStudio");
    let _ = std::fs::create_dir_all(&dir);
    match std::fs::write(dir.join("trasy_nastaveni.txt"), format!("jazyk={j}\n")) {
        Ok(_) => host.push_info(tr("Trasy: jazyk uložen, projeví se po restartu Open CAD Studia.")),
        Err(e) => host.push_error(&trf!("Trasy: nastavení nejde uložit: {}", e)),
    }
}

fn napoveda_cmd(host: &mut dyn HostApi) {
    for r in [
        "Trasy – beta 1 (neoficiální). Co umí:",
        "  Z polylinie – vyberte 2D křivku z úseček a tečných oblouků (začíná i končí přímou).",
        "  Kreslit vrcholy – klikáte vrcholy tečnového polygonu, oblouky mají zvolený poloměr.",
        "  Úchopy zap/vyp – čtverečky posouvají vrcholy, kolečka mění poloměr; náhled je živý, červená = nevejde se.",
        "  Vlastnosti trasy – název, počáteční staničení, styl, výjimky vzhledu a tabulka vrcholů (X, Y, R).",
        "  Přesun a otočení celé trasy – běžnými příkazy na bloku trasy; Překreslit trasy srovná popisky.",
        "  Jazyk: příkaz TRA_JAZYK cs | en | auto.",
    ] {
        host.push_output(tr(r));
    }
}

// ── Napojení na plugin Terén ─────────────────────────────────────────────────

/// Po načtení pluginu: kanál výkresu pro pracovní vlákno Tras.
pub fn on_load(host: &mut dyn HostApi) {
    if let Some(s) = host.plugin_request_sender() {
        uchopy::registruj_kanal(host.tab_id(), s);
    }
}

/// Upozornění Open CAD Studia (volá Terén pro každé upozornění).
pub fn on_notification(n: &HostNotification) {
    match n.clone() {
        HostNotification::DrawingChanged { tab_id, .. } | HostNotification::DocumentChangedV4 { tab_id, .. } => {
            uchopy::zmena_vykresu(tab_id)
        }
        HostNotification::DocumentOpened { tab_id } => uchopy::vykres_otevren(tab_id),
        HostNotification::SelectionChangedV4 { tab_id, handles } => uchopy::vyber_zmenen(tab_id, handles),
        HostNotification::PanelAction { panel_id, control_id, action, value } if panel_id == editor::ID => {
            editor::akce(&control_id, &action, value)
        }
        _ => {}
    }
}

/// Událost úchopu se značkou Tras.
pub fn grip_event(host: &mut dyn HostApi, ev: ocs_plugin_api::host::CustomGripEvent) -> ocs_plugin_api::host::CustomGripResponse {
    uchopy::udalost(host, ev)
}

/// Příkaz `TRA_…` (volá Terén).
pub fn dispatch(host: &mut dyn HostApi, cmd: &str) -> bool {
    if let Some(s) = host.plugin_request_sender() {
        uchopy::registruj_kanal(host.tab_id(), s);
    }
        let cmd = cmd.trim();
        let (nazev, zbytek) = match cmd.find(char::is_whitespace) {
            Some(i) => (&cmd[..i], cmd[i..].trim()),
            None => (cmd, ""),
        };
        let r = match nazev.to_ascii_uppercase().as_str() {
            "TRA_Z_POLYLINIE" => {
                z_polylinie_cmd(host, zbytek);
                true
            }
            "TRA_KRESLIT" => {
                kreslit_cmd(host);
                true
            }
            "TRA_KRESLIT_HOTOVO" => {
                kreslit_hotovo(host);
                true
            }
            "TRA_EDITOR" => {
                otevri_editor(host);
                true
            }
            "TRA_PRODLOUZIT" => {
                prodlouzit_cmd(host, zbytek);
                true
            }
            "TRA_VLOZIT_VRCHOL" => {
                vlozit_vrchol_cmd(host, zbytek);
                true
            }
            "TRA_SMAZAT_VRCHOL" => {
                smazat_vrchol_cmd(host, zbytek);
                true
            }
            "TRA_NIVELETA_TABULKA" => {
                niveleta_tabulka_cmd(host);
                true
            }
            "TRA_NIVELETA_TABULKA_HOTOVO" => {
                niveleta_tabulka_hotovo(host, zbytek);
                true
            }
            "TRA_NIVELETA_SMAZAT" => {
                niveleta_smazat_cmd(host, zbytek);
                true
            }
            "TRA_NIVELETA" => {
                niveleta_cmd(host);
                true
            }
            "TRA_NIVELETA_HOTOVO" => {
                niveleta_hotovo(host);
                true
            }
            "TRA_POHLED" => {
                pohled_cmd(host);
                true
            }
            "TRA_POHLED_VLOZ" => {
                pohled_vloz_cmd(host, zbytek);
                true
            }
            "TRA_OBLOUK_NASTAV" => {
                oblouk_nastav_cmd(host, zbytek);
                true
            }
            "TRA_BEZ_OBLOUKU" => {
                bez_oblouku_cmd(host, zbytek);
                true
            }
            "TRA_VLASTNOSTI_HOTOVO" => {
                vlastnosti_hotovo(host, zbytek);
                true
            }
            "TRA_OBLOUK" => {
                oblouk_cmd(host);
                true
            }
            "TRA_EDITOR_POUZIT" => {
                editor_pouzit_cmd(host);
                true
            }
            "TRA_EDITOR_VYBRAT" => {
                editor_vybrat_cmd(host, zbytek);
                true
            }
            "TRA_VLASTNOSTI" => {
                vlastnosti_cmd(host, zbytek);
                true
            }
            "TRA_UCHOPY" => {
                uchopy_cmd(host);
                true
            }
            "TRA_PREPOCITAT" => {
                prepocitat_cmd(host);
                true
            }
            "TRA_SEZNAM" => {
                seznam_cmd(host);
                true
            }
            "TRA_JAZYK" => {
                jazyk_cmd(host, zbytek);
                true
            }
            "TRA_NAPOVEDA" => {
                napoveda_cmd(host);
                true
            }
            _ => false,
        };
        host.end_undo_group();
        r
}
