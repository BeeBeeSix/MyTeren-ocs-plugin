//! Trasa jako objekt výkresu.
//!
//! Každá trasa je jeden blok `TRA_TRASA_<ID>` s osou, staničením a popisy,
//! vložený jednou. Na vložení je v XDATA `TRASY_TRASA` uložená definice
//! (vrcholy tečnového polygonu s poloměry, staničení, styl, výjimky). Vrcholy
//! jsou v souřadnicích bloku, takže přesun a otočení vložení běžnými příkazy
//! trasu opravdu posune a otočí. Bez pluginu je to obyčejný blok.

use std::f64::consts::PI;

use ocs_plugin_api::host::codec::entities::text::{TextHorizontalAlignment, TextVerticalAlignment};
use ocs_plugin_api::host::codec::entities::{Arc, Insert, Line, Text};
use ocs_plugin_api::host::codec::types::{Color, Vector3};
use ocs_plugin_api::host::{Document as CadDocument, EntityType, ExtendedDataRecord, Handle, LayerConfig, TableOperation, XDataValue};

use trasy::geometrie::{geometrie, DruhBodu, Geometrie, Prvek, Vrchol};
use trasy::preklad::{tr, tr_dyn};
use trasy::stanice::{cislo, staniceni, FormatStanice};
use trasy::styl::{vysledne, Hodnoty};
use trasy::trasa::{vestavene_styly, DefiniceTrasy, SCHEMA};
use trasy::trf;

use crate::vykres::Vykres;

/// XDATA s definicí trasy.
pub const APP: &str = "TRASY_TRASA";
/// XDATA s třídou objektu (název ve Vlastnostech).
pub const APP_TRIDA: &str = "TRASY_TRIDA";

pub const H_TRASY: &str = "TRA_TRASY";
pub const H_OSA: &str = "TRA_OSA";
pub const H_STANICENI: &str = "TRA_STANICENI";
pub const H_BODY: &str = "TRA_HLAVNI_BODY";
pub const H_POLYGON: &str = "TRA_POLYGON";

/// Trasa nalezená ve výkrese.
#[derive(Clone, Debug)]
pub struct ObjektTrasy {
    pub insert: Handle,
    pub blok: String,
    pub def: DefiniceTrasy,
    pub vlozeni: [f64; 2],
    pub otoceni: f64,
    /// Vložení má jiné měřítko než 1 (to plugin neumí – upozorní).
    pub meritko_jine: bool,
}

impl ObjektTrasy {
    /// Vrcholy ve světových souřadnicích (po případném přesunu a otočení).
    pub fn vrcholy(&self) -> Vec<Vrchol> {
        self.def.svetove(self.vlozeni, self.otoceni)
    }
    pub fn geometrie(&self) -> Result<Geometrie, String> {
        geometrie(&self.vrcholy(), self.def.pocatek).map_err(chyba_geometrie)
    }
}

pub fn chyba_geometrie(e: trasy::geometrie::Chyba) -> String {
    let (t, a) = e.popis();
    trasy::preklad::formatuj(tr(t), &a.iter().map(|s| s as &dyn std::fmt::Display).collect::<Vec<_>>())
}

pub(crate) fn retezce(r: &ExtendedDataRecord) -> Vec<&str> {
    r.values.iter().filter_map(|v| if let XDataValue::String(s) = v { Some(s.as_str()) } else { None }).collect()
}

/// Trasa z jedné entity (vložení bloku s definicí), jinak `None`.
pub fn z_entity(e: &EntityType) -> Option<ObjektTrasy> {
    let EntityType::Insert(i) = e else { return None };
    let r = i.common.extended_data.get_record(APP)?;
    let def = DefiniceTrasy::cti(&retezce(r))?;
    Some(ObjektTrasy {
        insert: i.common.handle,
        blok: i.block_name.clone(),
        def,
        vlozeni: [i.insert_point.x, i.insert_point.y],
        otoceni: i.rotation,
        meritko_jine: (i.x_scale() - 1.0).abs() > 1e-9 || (i.y_scale() - 1.0).abs() > 1e-9,
    })
}

/// Všechny trasy ve výkrese.
pub fn trasy(doc: &CadDocument) -> Vec<ObjektTrasy> {
    doc.model_space_entities().filter_map(z_entity).collect()
}

/// Trasy, na které ukazuje výběr (vložení bloku trasy).
pub fn z_vyberu(doc: &CadDocument, vyber: &[Handle]) -> Vec<ObjektTrasy> {
    trasy(doc).into_iter().filter(|t| vyber.contains(&t.insert)).collect()
}

/// Trasa nejblíž k bodu (do vzdálenosti 5 % délky trasy, nejméně 2 m).
pub fn nejblizsi(doc: &CadDocument, p: [f64; 2]) -> Option<ObjektTrasy> {
    let mut nej: Option<(f64, ObjektTrasy)> = None;
    for t in trasy(doc) {
        let Ok(g) = t.geometrie() else { continue };
        let Some((_, o)) = g.prumet(p) else { continue };
        let tol = (g.delka() * 0.05).max(2.0);
        if o.abs() <= tol && nej.as_ref().is_none_or(|(d, _)| o.abs() < *d) {
            nej = Some((o.abs(), t));
        }
    }
    nej.map(|(_, t)| t)
}

/// Výsledné hodnoty stylu pro definici.
pub fn hodnoty(def: &DefiniceTrasy) -> Hodnoty<'static> {
    vysledne(SCHEMA, &vestavene_styly(), &def.styl, &def.vyjimky)
}

// ── Kreslení ────────────────────────────────────────────────────────────────

pub(crate) fn na(mut e: EntityType, h: &str) -> EntityType {
    e.common_mut().layer = h.to_string();
    e
}

pub(crate) fn v3(p: [f64; 2]) -> Vector3 {
    Vector3::new(p[0], p[1], 0.0)
}

pub(crate) fn cara(a: [f64; 2], b: [f64; 2], h: &str) -> EntityType {
    na(EntityType::Line(Line::from_points(v3(a), v3(b))), h)
}

/// Bod posunutý kolmo k ose (+ vlevo).
fn kolmo(p: [f64; 2], smer: f64, o: f64) -> [f64; 2] {
    [p[0] - o * smer.sin(), p[1] + o * smer.cos()]
}

/// Text čitelný zleva nebo zdola: když by byl vzhůru nohama, otočí se o 180°
/// a zarovná z druhé strany, aby zůstal na stejném místě.
fn text(t: String, p: [f64; 2], uhel: f64, vyska: f64, h: &str, od_bodu: bool) -> EntityType {
    let mut u = uhel.rem_euclid(2.0 * PI);
    let mut zarovnani = if od_bodu { TextHorizontalAlignment::Left } else { TextHorizontalAlignment::Center };
    if u > PI / 2.0 + 1e-9 && u <= 3.0 * PI / 2.0 + 1e-9 {
        u -= PI;
        if od_bodu {
            zarovnani = TextHorizontalAlignment::Right;
        }
    }
    let mut x = Text::with_value(t, v3(p)).with_height(vyska).with_rotation(u);
    x.horizontal_alignment = zarovnani;
    x.vertical_alignment = TextVerticalAlignment::Middle;
    x.alignment_point = Some(v3(p));
    na(EntityType::Text(x), h)
}

fn oblouk(stred: [f64; 2], r: f64, uhel_a: f64, sweep: f64) -> EntityType {
    let (z, k) = if sweep >= 0.0 { (uhel_a, uhel_a + sweep) } else { (uhel_a + sweep, uhel_a) };
    na(EntityType::Arc(Arc::from_center_radius_angles(v3(stred), r, z.rem_euclid(2.0 * PI), k.rem_euclid(2.0 * PI))), H_OSA)
}

/// Objekty trasy (osa, staničení, hlavní body) ve světových souřadnicích.
pub fn objekty(def: &DefiniceTrasy, vrcholy: &[Vrchol], g: &Geometrie) -> Vec<EntityType> {
    let h = hodnoty(def);
    let vyska = h.cislo("vyska_textu").max(0.1) * h.cislo("meritko").max(1.0) / 1000.0;
    let format = FormatStanice::z_kodu(h.text("format"));
    let carka = h.ano("carka");
    let mut out = Vec::new();

    // polygon první, aby osa ležela nad ním
    if h.ano("polygon") && vrcholy.iter().any(|v| v.r > 0.0) {
        for w in vrcholy.windows(2) {
            out.push(cara(w[0].bod(), w[1].bod(), H_POLYGON));
        }
    }
    for (_, p) in &g.prvky {
        match *p {
            Prvek::Primka { a, b } => out.push(na(cara(a, b, H_OSA), H_OSA)),
            Prvek::Oblouk { stred, r, uhel_a, sweep, .. } => out.push(oblouk(stred, r, uhel_a, sweep)),
        }
    }

    // staničení: vedlejší jen značka, hlavní značka a text vlevo
    let hlavni = h.cislo("krok_hlavni");
    let vedlejsi = h.cislo("krok_vedlejsi");
    let je_hlavni = |s: f64| hlavni > 0.0 && ((s / hlavni).round() * hlavni - s).abs() < 1e-6;
    if vedlejsi > 0.0 {
        for s in g.staniceni_po_kroku(vedlejsi) {
            if je_hlavni(s) {
                continue;
            }
            if let Some((p, smer)) = g.bod(s) {
                out.push(cara(kolmo(p, smer, -0.5 * vyska), kolmo(p, smer, 0.5 * vyska), H_STANICENI));
            }
        }
    }
    if hlavni > 0.0 {
        let des = h.cislo("desetin_hlavni").max(0.0) as usize;
        for s in g.staniceni_po_kroku(hlavni) {
            if let Some((p, smer)) = g.bod(s) {
                out.push(cara(kolmo(p, smer, -vyska), kolmo(p, smer, vyska), H_STANICENI));
                let t = staniceni(s, format, des, carka);
                out.push(text(t, kolmo(p, smer, 1.4 * vyska), smer + PI / 2.0, vyska, H_STANICENI, true));
            }
        }
    }

    // hlavní body: delší značka, text vpravo od osy
    let zkratky: Vec<String> = h.text("zkratky").split(',').map(|s| s.trim().to_string()).collect();
    let zk = |i: usize, vych: &str| zkratky.get(i).filter(|s| !s.is_empty()).cloned().unwrap_or_else(|| vych.to_string());
    if h.ano("popis_body") {
        let des = h.cislo("desetin_body").max(0.0) as usize;
        for b in &g.hlavni_body {
            let nazev = match b.druh {
                DruhBodu::Zu => zk(0, "ZÚ"),
                DruhBodu::Ku => zk(1, "KÚ"),
                DruhBodu::Tk => zk(2, "TK"),
                DruhBodu::Kt => zk(3, "KT"),
                DruhBodu::V => continue,
            };
            out.push(cara(kolmo(b.bod, b.smer, -2.0 * vyska), kolmo(b.bod, b.smer, 2.0 * vyska), H_BODY));
            let t = format!("{nazev} {}", staniceni(b.staniceni, format, des, carka));
            out.push(text(t, kolmo(b.bod, b.smer, -2.4 * vyska), b.smer - PI / 2.0, vyska, H_BODY, true));
        }
    }
    if h.ano("popis_vrcholy") {
        let v_zk = zk(4, "V");
        for (i, v) in vrcholy.iter().enumerate() {
            if i == 0 || i + 1 == vrcholy.len() {
                continue;
            }
            let mut t = format!("{v_zk}{i}");
            if v.r > 0.0 {
                t.push_str(&format!("  R={}", cislo(v.r, 2, carka)));
            }
            out.push(text(t, [v.x + vyska, v.y + vyska], 0.0, vyska, H_BODY, true));
        }
    }
    out
}

// ── Uložení ─────────────────────────────────────────────────────────────────

fn zajisti_hladiny(v: &mut dyn Vykres, doc: &CadDocument) {
    for (nazev, barva) in [(H_TRASY, 7), (H_OSA, 1), (H_STANICENI, 7), (H_BODY, 3), (H_POLYGON, 8)] {
        if !doc.layers.iter().any(|l| l.name.eq_ignore_ascii_case(nazev)) {
            v.hladina(LayerConfig { name: nazev.to_string(), color: Some(Color::from_index(barva)), ..Default::default() });
        }
    }
}

fn zaznam(def: &DefiniceTrasy) -> ExtendedDataRecord {
    let mut r = ExtendedDataRecord::new(APP);
    for s in def.zapis() {
        r.add_value(XDataValue::String(s));
    }
    r
}

/// Třída objektu pro panel Vlastnosti (pod aplikací a předponou Terénu).
fn zaznam_tridy() -> ExtendedDataRecord {
    crate::povrchy::zaznam_tridy("TRASA")
}
/// Bod vložení bloku trasy. Open CAD Studio na něm ukazuje vlastní úchop
/// (posun celé trasy) – nesmí ležet na vrcholu ani na středu úseku, kde jsou
/// úchopy trasy. Bere se střed rozsahu vrcholů; když padne na tečnový
/// polygon (rovná trasa), odsune se kolmo stranou.
fn bod_vlozeni(v: &[trasy::geometrie::Vrchol]) -> [f64; 2] {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in v {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    let c = [(x0 + x1) / 2.0, (y0 + y1) / 2.0];
    let odstup = ((x1 - x0).hypot(y1 - y0) * 0.1).max(2.0);
    // nejbližší úsek polygonu
    let mut nejblizsi: Option<(f64, [f64; 2])> = None;
    for w in v.windows(2) {
        let (ax, ay, bx, by) = (w[0].x, w[0].y, w[1].x, w[1].y);
        let (dx, dy) = (bx - ax, by - ay);
        let l2 = dx * dx + dy * dy;
        if l2 < 1e-12 {
            continue;
        }
        let t = (((c[0] - ax) * dx + (c[1] - ay) * dy) / l2).clamp(0.0, 1.0);
        let d = (c[0] - ax - t * dx).hypot(c[1] - ay - t * dy);
        if nejblizsi.is_none_or(|(n, _)| d < n) {
            let l = l2.sqrt();
            nejblizsi = Some((d, [-dy / l, dx / l]));
        }
    }
    match nejblizsi {
        Some((d, n)) if d < odstup => [c[0] + n[0] * odstup, c[1] + n[1] * odstup],
        _ => c,
    }
}


/// Nakreslí trasu (novou nebo místo staré) z definice se SVĚTOVÝMI vrcholy.
/// Když geometrie nejde spočítat, výkres se nezmění.
///
/// `doc` = None u úpravy existující trasy (tažení úchopu): hladiny i blok už
/// existují, takže není potřeba stahovat celý výkres – úprava je rychlejší.
pub fn uloz(v: &mut dyn Vykres, doc: Option<&CadDocument>, stara: Option<&ObjektTrasy>, mut def: DefiniceTrasy) -> Result<Handle, String> {
    let g = geometrie(&def.vrcholy, def.pocatek).map_err(chyba_geometrie)?;
    let zaklad = bod_vlozeni(&def.vrcholy);
    def.zaklad = zaklad;
    // Objekty se do bloku dají už posunuté o základní bod a blok má základ
    // 0,0 (jako povrch v Terénu). Se základem v souřadnicích terénu by ho
    // OCS odečetl dvakrát a trasa by se kreslila u počátku souřadnic.
    let posun = Vector3::new(-zaklad[0], -zaklad[1], 0.0);
    let ent: Vec<EntityType> = objekty(&def, &def.vrcholy, &g)
        .into_iter()
        .map(|mut e| {
            e.as_entity_mut().translate(posun);
            e
        })
        .collect();
    if let Some(doc) = doc {
        zajisti_hladiny(v, doc);
    }

    let blok = format!("TRA_TRASA_{}", def.id);
    let docasny = format!("{blok}_NOVY");
    if doc.is_none_or(|d| d.block_records.iter().any(|b| b.name.eq_ignore_ascii_case(&docasny))) {
        let _ = v.tabulka(TableOperation::BlockDelete { name: docasny.clone() });
    }
    let handly = v.pridej(ent)?;
    v.tabulka(TableOperation::BlockCreate {
        name: docasny.clone(),
        entities: handly,
        base_point: [0.0, 0.0, 0.0],
        erase_originals: true,
        description: Some(trf!("Trasa {} (plugin Trasy)", def.jmeno)),
    })
    .map_err(|e| trf!("blok trasy nejde vytvořit: {}", e))?;
    if let Some(s) = stara {
        v.odeber(s.insert);
        if !s.blok.eq_ignore_ascii_case(&docasny) && doc.is_none_or(|d| d.block_records.iter().any(|b| b.name.eq_ignore_ascii_case(&s.blok))) {
            if let Err(e) = v.tabulka(TableOperation::BlockDelete { name: s.blok.clone() }) {
                v.chyba(&trf!("Trasy: starý blok trasy nejde odstranit: {}", e));
            }
        }
    }
    let jmeno_bloku = match v.tabulka(TableOperation::BlockRename { from: docasny.clone(), to: blok.clone() }) {
        Ok(_) => blok,
        Err(_) => docasny,
    };
    let mut ins = Insert::new(jmeno_bloku, v3(zaklad));
    ins.common.layer = H_TRASY.into();
    let insert = v.pridej_jednu(EntityType::Insert(ins))?;
    if !v.zapis_xdata(insert, zaznam(&def)) {
        return Err(tr("nepodařilo se uložit definici trasy").into());
    }
    v.zapis_xdata(insert, zaznam_tridy());
    v.hotovo();
    // pohledy této trasy ve stejném kroku Zpět – z výkresu staženého před
    // uložením (stažení teď by krok Zpět v OCS uzavřelo)
    if let Some(d) = doc.filter(|_| crate::tra::pohled::ma_pohledy()) {
        let nova = ObjektTrasy { insert, blok: String::new(), def: def.clone(), vlozeni: zaklad, otoceni: 0.0, meritko_jine: false };
        crate::tra::pohled::prekresli_zastarale(v, d, Some(&def.id), Some(&nova));
    }
    Ok(insert)
}

/// Uloží jen definici trasy (XDATA) – např. změněnou niveletu.
pub fn uloz_definici(v: &mut dyn Vykres, insert: Handle, def: &DefiniceTrasy) -> bool {
    v.zapis_xdata(insert, zaznam(def))
}

/// Smaže trasu (vložení i blok) – pro Správce (fáze 2).
#[allow(dead_code)]
pub fn smaz(v: &mut dyn Vykres, doc: &CadDocument, t: &ObjektTrasy) {
    v.odeber(t.insert);
    if doc.block_records.iter().any(|b| b.name.eq_ignore_ascii_case(&t.blok)) {
        let _ = v.tabulka(TableOperation::BlockDelete { name: t.blok.clone() });
    }
    v.hotovo();
}

/// Krátký popis trasy do příkazového řádku.
pub fn popis(t: &ObjektTrasy) -> String {
    let h = hodnoty(&t.def);
    let f = FormatStanice::z_kodu(h.text("format"));
    let styl = vestavene_styly().into_iter().find(|s| s.id == t.def.styl).map_or(t.def.styl.clone(), |s| tr_dyn(&s.nazev));
    match t.geometrie() {
        Ok(g) => {
            let oblouku = g.prvky.iter().filter(|(_, p)| p.je_oblouk()).count();
            trf!(
                "{} „{}“: {} – {}, délka {} m, vrcholů {}, oblouků {}, styl {}",
                t.def.id,
                t.def.jmeno,
                staniceni(g.pocatek, f, 2, true),
                staniceni(g.konec(), f, 2, true),
                cislo(g.delka(), 2, true),
                t.def.vrcholy.len(),
                oblouku,
                styl
            )
        }
        Err(e) => trf!("{} „{}“: chyba geometrie – {}", t.def.id, t.def.jmeno, e),
    }
}

#[cfg(test)]
mod testy {
    #[test]
    fn bod_vlozeni_mimo_uchopy() {
        use trasy::geometrie::Vrchol;
        // rovná trasa: střed rozsahu by ležel na trojúhelníku „vložit vrchol“
        let v = [Vrchol::new(0.0, 0.0, 0.0), Vrchol::new(100.0, 0.0, 0.0)];
        let b = bod_vlozeni(&v);
        assert!((b[0] - 50.0).abs() < 1e-9 && b[1].abs() >= 9.9, "{b:?}");
        // lomená trasa: střed rozsahu leží mimo polygon i mimo vrcholy
        let v = [Vrchol::new(0.0, 0.0, 0.0), Vrchol::new(100.0, 0.0, 30.0), Vrchol::new(100.0, 100.0, 0.0)];
        let b = bod_vlozeni(&v);
        for p in &v {
            assert!((b[0] - p.x).hypot(b[1] - p.y) > 5.0, "{b:?}");
        }
    }

    use super::*;

    #[test]
    fn popisky_se_kresli() {
        // jako červená polylinie ze zkušebního výkresu: ~330 m, dva oblouky
        let v = vec![Vrchol::new(0.0, 0.0, 0.0), Vrchol::new(100.0, 40.0, 36.0), Vrchol::new(150.0, 160.0, 36.0), Vrchol::new(260.0, 180.0, 0.0)];
        let def = DefiniceTrasy::nova("T1", "Trasa 1", v.clone(), trasy::trasa::STYL_ZAKLAD);
        let g = geometrie(&v, 0.0).unwrap();
        let e = objekty(&def, &v, &g);
        let texty: Vec<String> = e.iter().filter_map(|x| if let EntityType::Text(t) = x { Some(t.value.clone()) } else { None }).collect();
        let cary = e.iter().filter(|x| matches!(x, EntityType::Line(_))).count();
        eprintln!("délka {:.1}, čar {cary}, textů {}: {:?}", g.delka(), texty.len(), texty);
        assert!(texty.iter().any(|t| t.starts_with("TK ")));
        assert!(texty.iter().any(|t| t == "0,100"));
        if let Some(EntityType::Text(t)) = e.iter().find(|x| matches!(x, EntityType::Text(_))) {
            eprintln!("výška {} vrstva {} zarovnání {:?}", t.height, t.common.layer, t.horizontal_alignment);
        }
    }
}
