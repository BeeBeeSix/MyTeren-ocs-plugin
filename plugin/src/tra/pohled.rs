//! Pohled podélného profilu jako objekt výkresu.
//!
//! Blok `TRA_POHLED_<ID>` (rám, mřížka, popisy os, srovnávací rovina,
//! titulek, terénní profil) vložený v levém dolním rohu pohledu. Definice
//! je v XDATA `TRASY_POHLED` na vložení. Terénní profil se počítá při
//! kreslení z trasy a z trojúhelníků povrchu Terénu.

use ocs_plugin_api::host::codec::entities::text::{TextHorizontalAlignment, TextVerticalAlignment};
use ocs_plugin_api::host::codec::entities::{Insert, LwPolyline, Text};
use ocs_plugin_api::host::codec::types::{Color, Vector2};
use ocs_plugin_api::host::{Document as CadDocument, EntityType, ExtendedDataRecord, Handle, LayerConfig, TableOperation, XDataValue};

use trasy::geometrie::{DruhBodu, Geometrie, Prvek};
use trasy::pasy::{mista_bez_kolizi, vyska_v, DruhPasu, Pas, Poloha, Zdroj};
use trasy::pohled::{nastaveni, DefinicePohledu, Osy};
use trasy::preklad::tr;
use trasy::profil::{profil_terenu, rozsah_vysek, Povrch};
use trasy::stanice::{cislo, staniceni, FormatStanice};
use trasy::trf;

use crate::tra::objekt::{self, cara, na, retezce, v3};
use crate::tra::povrchy;
use crate::vykres::Vykres;

pub const APP: &str = "TRASY_POHLED";

/// Ve výkrese jsou pohledy – pracovní vlákno je po změně výkresu hlídá.
pub static MA_POHLEDY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn ma_pohledy() -> bool {
    MA_POHLEDY.load(std::sync::atomic::Ordering::SeqCst)
}

/// Otisk zdrojů pohledu: vložení trasy a povrchu (handle se mění s každým
/// přestavěním, poloha a otočení s přesunem).
pub fn otisk(doc: &CadDocument, def: &DefinicePohledu) -> String {
    otisk_s(doc, def, None)
}

/// Najde trasu pohledu: nově uloženou (`nova`), jinak ve výkrese.
fn trasa_pohledu(doc: &CadDocument, def: &DefinicePohledu, nova: Option<&objekt::ObjektTrasy>) -> Option<objekt::ObjektTrasy> {
    match nova {
        Some(t) if t.def.id == def.trasa => Some(t.clone()),
        _ => objekt::trasy(doc).into_iter().find(|t| t.def.id == def.trasa),
    }
}

/// Otisk s nově uloženou trasou (výkres může být ještě ten před uložením).
pub fn otisk_s(doc: &CadDocument, def: &DefinicePohledu, nova: Option<&objekt::ObjektTrasy>) -> String {
    let t = trasa_pohledu(doc, def, nova);
    let p = povrchy::povrchy(doc).into_iter().find(|p| p.id == def.povrch);
    let t = t.map_or("-".to_string(), |t| {
        // nivelety se mění bez přestavění bloku trasy → do otisku jejich součet
        let niv: String = t.def.nivelety.iter().flat_map(|n| n.zapis()).collect::<Vec<_>>().join("|");
        let h = niv.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3));
        format!("{:X}@{:.3},{:.3},{:.6}#{:X}", t.insert.value(), t.vlozeni[0], t.vlozeni[1], t.otoceni, h)
    });
    let p = p.map_or("-".to_string(), |p| format!("{:X}@{:.3},{:.3}", p.insert.value(), p.vlozeni[0], p.vlozeni[1]));
    format!("{t};{p}")
}

/// Překreslí pohledy, jejichž trasa nebo povrch se od posledního kreslení
/// změnily. Vrací počet překreslených.
///
/// `doc` musí být stažený PŘED otevřením kroku Zpět – stažení výkresu uvnitř
/// kroku ho v OCS uzavře. Nově uloženou trasu (ještě není v `doc`) předá `nova`.
pub fn prekresli_zastarale(v: &mut dyn Vykres, doc: &CadDocument, jen_trasa: Option<&str>, nova: Option<&objekt::ObjektTrasy>) -> usize {
    prekresli_zastarale_h(v, doc, jen_trasa, nova).len()
}

/// Jako [`prekresli_zastarale`], vrací dvojice (staré vložení, nové vložení).
pub fn prekresli_zastarale_h(
    v: &mut dyn Vykres,
    doc: &CadDocument,
    jen_trasa: Option<&str>,
    nova: Option<&objekt::ObjektTrasy>,
) -> Vec<(Handle, Handle)> {
    let vse = pohledy(doc);
    MA_POHLEDY.store(!vse.is_empty(), std::sync::atomic::Ordering::SeqCst);
    let mut out = Vec::new();
    for p in vse {
        if jen_trasa.is_some_and(|id| id != p.def.trasa) || otisk_s(doc, &p.def, nova) == p.def.otisk {
            continue;
        }
        match uloz_s(v, doc, Some(&p), &p.def, p.vlozeni, nova) {
            Ok((h, _)) => out.push((p.insert, h)),
            Err(e) => v.chyba(&trf!("Trasy: pohled {}: {}", p.def.id, e)),
        }
    }
    out
}

pub const H_POHLED: &str = "TRA_POHLED";
pub const H_RAM: &str = "TRA_POHLED_RAM";
pub const H_MRIZKA: &str = "TRA_POHLED_MRIZKA";
pub const H_POPIS: &str = "TRA_POHLED_POPIS";
pub const H_TEREN: &str = "TRA_PROFIL_TEREN";
pub const H_PASY: &str = "TRA_POHLED_PASY";
pub const H_NIVELETA: &str = "TRA_PROFIL_NIVELETA";
pub const H_NIV_POLYGON: &str = "TRA_PROFIL_NIVELETA_POLYGON";

#[derive(Clone, Debug)]
pub struct ObjektPohledu {
    pub insert: Handle,
    pub blok: String,
    pub def: DefinicePohledu,
    pub vlozeni: [f64; 2],
}

pub fn pohledy(doc: &CadDocument) -> Vec<ObjektPohledu> {
    doc.model_space_entities().filter_map(z_entity).collect()
}

/// Pohled z jedné entity (vložení s definicí pohledu).
pub fn z_entity(e: &EntityType) -> Option<ObjektPohledu> {
    let EntityType::Insert(i) = e else { return None };
    let def = DefinicePohledu::cti(&retezce(i.common.extended_data.get_record(APP)?))?;
    Some(ObjektPohledu { insert: i.common.handle, blok: i.block_name.clone(), def, vlozeni: [i.insert_point.x, i.insert_point.y] })
}

impl ObjektPohledu {
    /// (staničení, výška) → bod výkresu (podle os z posledního kreslení).
    pub fn na_vykres(&self, s: f64, z: f64) -> Option<[f64; 3]> {
        let [od, _, sr, _, prev] = self.def.osy?;
        Some([self.vlozeni[0] + s - od, self.vlozeni[1] + (z - sr) * prev, 0.0])
    }
    /// Bod výkresu → (staničení, výška).
    pub fn na_profil(&self, p: [f64; 3]) -> Option<(f64, f64)> {
        let [od, _, sr, _, prev] = self.def.osy?;
        Some((od + p[0] - self.vlozeni[0], sr + (p[1] - self.vlozeni[1]) / prev))
    }
    /// Leží bod v rámu pohledu?
    pub fn obsahuje(&self, p: [f64; 2]) -> bool {
        let Some([od, do_, sr, horni, prev]) = self.def.osy else { return false };
        let (x, y) = (p[0] - self.vlozeni[0], p[1] - self.vlozeni[1]);
        x >= -1e-6 && x <= do_ - od + 1e-6 && y >= -1e-6 && y <= (horni - sr) * prev + 1e-6
    }
}

fn text(t: String, p: [f64; 2], vyska: f64, h: &str, vodorovne: TextHorizontalAlignment, svisle: TextVerticalAlignment) -> EntityType {
    let mut x = Text::with_value(t, v3(p)).with_height(vyska);
    x.horizontal_alignment = vodorovne;
    x.vertical_alignment = svisle;
    x.alignment_point = Some(v3(p));
    na(EntityType::Text(x), h)
}

/// Výsledek sestavení pohledu (objekty v souřadnicích pohledu a souhrn).
pub struct Sestaveny {
    pub objekty: Vec<EntityType>,
    pub trojuhelniku: usize,
    pub useku: usize,
    pub osy: Osy,
}

/// Spočte terénní profil a objekty pohledu (souřadnice vůči levému dolnímu rohu).
pub fn sestav(doc: &CadDocument, def: &DefinicePohledu, nova: Option<&objekt::ObjektTrasy>) -> Result<Sestaveny, String> {
    let trasa = trasa_pohledu(doc, def, nova).ok_or_else(|| trf!("trasa {} už ve výkrese není", def.trasa))?;
    let g = trasa.geometrie()?;
    let povrch = povrchy::povrchy(doc).into_iter().find(|p| p.id == def.povrch).ok_or_else(|| trf!("povrch {} už ve výkrese není", def.povrch))?;
    let troj = povrchy::trojuhelniky(doc, &povrch);
    if troj.is_empty() {
        return Err(trf!(
            "povrch „{}“ nemá ve výkrese trojúhelníky – v Terénu mu nastavte styl se zobrazeným TIN a zkuste to znovu",
            povrch.jmeno
        ));
    }
    let pocet = troj.len();
    let index = Povrch::new(troj);
    let od = def.od.unwrap_or(g.pocatek).max(g.pocatek);
    let do_ = def.do_.unwrap_or(g.konec()).min(g.konec());
    let useky = profil_terenu(&g, &index, od, do_);
    let (mut zmin, mut zmax) = rozsah_vysek(&useky).ok_or_else(|| trf!("trasa {} nepřechází přes povrch „{}“", trasa.def.jmeno, povrch.jmeno))?;
    let niveleta = trasa.def.nivelety.first().filter(|n| n.kontrola().is_ok()).cloned();
    if let Some(n) = &niveleta {
        for v in &n.vrcholy {
            if v.s >= od - 1e-6 && v.s <= do_ + 1e-6 {
                zmin = zmin.min(v.z);
                zmax = zmax.max(v.z);
            }
        }
    }
    let osy = Osy::rozvrhni(od, do_, zmin, zmax, def);

    let nast = nastaveni(def);
    let svislice = nast.text("cary_pasu") == "svislice";
    let fmt = FormatStanice::z_kodu(nast.text("staniceni"));
    let sablona = nast.text("sablona").to_string();
    let h = nast.cislo("pismo").clamp(1.0, 10.0) * def.meritko / 1000.0;
    let (w, v) = (osy.sirka(), osy.vyska());
    let mut ent = Vec::new();
    // mřížka
    let ma_pas_staniceni = def.pasy.iter().any(|p| p.druh == DruhPasu::Staniceni);
    let mrizka = nast.ano("mrizka");
    for s in osy.staniceni_mrizky() {
        let x = s - osy.od;
        if mrizka {
            ent.push(cara([x, 0.0], [x, v], H_MRIZKA));
        }
        if ma_pas_staniceni || !mrizka {
            continue;
        }
        ent.push(text(staniceni(s, fmt, 0, true), [x, -1.2 * h], h, H_POPIS, TextHorizontalAlignment::Center, TextVerticalAlignment::Top));
    }
    let stupnice = nast.text("stupnice").to_string();
    for z in osy.vysky_mrizky() {
        let y = (z - osy.srovnavaci) * osy.prevyseni;
        if mrizka {
            ent.push(cara([0.0, y], [w, y], H_MRIZKA));
        }
        // u srovnávací roviny je s pásy popis „S.R. …“ (viz níže)
        if !def.pasy.is_empty() && (z - osy.srovnavaci).abs() < 1e-6 {
            continue;
        }
        if stupnice != "skryt" {
            ent.push(text(cislo(z, 2, true), [-0.8 * h, y], h, H_POPIS, TextHorizontalAlignment::Right, TextVerticalAlignment::Middle));
        }
        if stupnice == "obe" {
            ent.push(text(cislo(z, 2, true), [w + 0.8 * h, y], h, H_POPIS, TextHorizontalAlignment::Left, TextVerticalAlignment::Middle));
        }
    }
    // rám grafu (bez rámu aspoň srovnávací rovina)
    if nast.ano("ram") {
        for (a, b) in [([0.0, 0.0], [w, 0.0]), ([w, 0.0], [w, v]), ([w, v], [0.0, v]), ([0.0, v], [0.0, 0.0])] {
            ent.push(cara(a, b, H_RAM));
        }
    } else {
        ent.push(cara([0.0, 0.0], [w, 0.0], H_RAM));
    }
    // srovnávací rovina
    let sirka_zahlavi = 38.0 * def.meritko / 1000.0;
    if def.pasy.is_empty() {
        ent.push(text(
            trf!("S.R. {} m n. m.", cislo(osy.srovnavaci, 2, true)),
            [0.0, -3.4 * h],
            h,
            H_POPIS,
            TextHorizontalAlignment::Left,
            TextVerticalAlignment::Top,
        ));
    } else {
        ent.push(text(
            trf!("S.R. {} m n. m.", cislo(osy.srovnavaci, 2, true)),
            [-0.8 * h, 0.0],
            h,
            H_POPIS,
            TextHorizontalAlignment::Right,
            TextVerticalAlignment::Middle,
        ));
    }
    // terén
    for u in &useky {
        let body: Vec<Vector2> = u
            .iter()
            .map(|p| {
                let q = osy.bod(p[0], p[1]);
                Vector2::new(q[0], q[1])
            })
            .collect();
        ent.push(na(EntityType::LwPolyline(LwPolyline::from_points(body)), H_TEREN));
    }
    // niveleta: tečnový polygon (šedě, je-li zakružení) a niveleta s oblouky
    if let Some(n) = &niveleta {
        if !n.oblouky().is_empty() {
            let body: Vec<Vector2> = n.vrcholy.iter().map(|v| { let q = osy.bod(v.s, v.z); Vector2::new(q[0], q[1]) }).collect();
            ent.push(na(EntityType::LwPolyline(LwPolyline::from_points(body)), H_NIV_POLYGON));
        }
        let body: Vec<Vector2> = n
            .body()
            .into_iter()
            .filter(|p| p[0] >= osy.od - 1e-6 && p[0] <= osy.do_ + 1e-6)
            .map(|p| { let q = osy.bod(p[0], p[1]); Vector2::new(q[0], q[1]) })
            .collect();
        if body.len() >= 2 {
            ent.push(na(EntityType::LwPolyline(LwPolyline::from_points(body)), H_NIVELETA));
        }
    }
    // profily dalších povrchů, ze kterých čtou pásy
    let mut dalsi: Vec<(String, Vec<Vec<[f64; 2]>>)> = Vec::new();
    for p in &def.pasy {
        for z in [&p.zdroj, &p.zdroj2] {
            let Zdroj::Povrch(id) = z else { continue };
            if *id == def.povrch || dalsi.iter().any(|(x, _)| x == id) {
                continue;
            }
            let u = povrchy::povrchy(doc)
                .into_iter()
                .find(|x| x.id == *id)
                .map(|x| profil_terenu(&g, &Povrch::new(povrchy::trojuhelniky(doc, &x)), osy.od, osy.do_))
                .unwrap_or_default();
            dalsi.push((id.clone(), u));
        }
    }
    let kontext = KontextPasu {
        def,
        osy: &osy,
        g: &g,
        useky: &useky,
        dalsi: &dalsi,
        niveleta: niveleta.as_ref(),
        h,
        sirka_zahlavi,
        svislice,
        fmt,
        procenta: nast.text("sklon") == "procento",
        lomy_z_nivelety: matches!(sablona.as_str(), "kanalizace" | "tok"),
    };
    let (pasy_ent, vyska_nad) = pasy(&kontext);
    ent.extend(pasy_ent);
    // titulek nad pásy nad grafem
    let mv = def.meritko / def.prevyseni.max(0.1);
    ent.push(text(
        trf!("Podélný profil – {} · povrch {} · 1:{}/{}", trasa.def.jmeno, povrch.jmeno, cislo(def.meritko, 0, true), cislo(mv, 0, true)),
        [0.0, v + vyska_nad + 1.5 * h],
        1.4 * h,
        H_POPIS,
        TextHorizontalAlignment::Left,
        TextVerticalAlignment::Bottom,
    ));
    Ok(Sestaveny { objekty: ent, trojuhelniku: pocet, useku: useky.len(), osy })
}

fn zajisti_hladiny(v: &mut dyn Vykres, doc: &CadDocument) {
    for (nazev, barva) in [(H_POHLED, 7), (H_RAM, 7), (H_MRIZKA, 8), (H_POPIS, 7), (H_TEREN, 32), (H_PASY, 7), (H_NIVELETA, 1), (H_NIV_POLYGON, 8)] {
        if !doc.layers.iter().any(|l| l.name.eq_ignore_ascii_case(nazev)) {
            v.hladina(LayerConfig { name: nazev.to_string(), color: Some(Color::from_index(barva)), ..Default::default() });
        }
    }
}

/// Nakreslí pohled (nový nebo místo starého) s levým dolním rohem ve `vlozeni`.
pub fn uloz(
    v: &mut dyn Vykres,
    doc: &CadDocument,
    stary: Option<&ObjektPohledu>,
    def: &DefinicePohledu,
    vlozeni: [f64; 2],
) -> Result<(Handle, Sestaveny), String> {
    uloz_s(v, doc, stary, def, vlozeni, None)
}

pub fn uloz_s(
    v: &mut dyn Vykres,
    doc: &CadDocument,
    stary: Option<&ObjektPohledu>,
    def: &DefinicePohledu,
    vlozeni: [f64; 2],
    nova: Option<&objekt::ObjektTrasy>,
) -> Result<(Handle, Sestaveny), String> {
    let s = sestav(doc, def, nova)?;
    zajisti_hladiny(v, doc);
    let blok = format!("TRA_POHLED_{}", def.id);
    let docasny = format!("{blok}_NOVY");
    if doc.block_records.iter().any(|b| b.name.eq_ignore_ascii_case(&docasny)) {
        let _ = v.tabulka(TableOperation::BlockDelete { name: docasny.clone() });
    }
    let handly = v.pridej(s.objekty.clone())?;
    v.tabulka(TableOperation::BlockCreate {
        name: docasny.clone(),
        entities: handly,
        base_point: [0.0, 0.0, 0.0],
        erase_originals: true,
        description: Some(trf!("Pohled podélného profilu {} (plugin Trasy)", def.id)),
    })
    .map_err(|e| trf!("blok pohledu nejde vytvořit: {}", e))?;
    if let Some(st) = stary {
        v.odeber(st.insert);
        if !st.blok.eq_ignore_ascii_case(&docasny) && doc.block_records.iter().any(|b| b.name.eq_ignore_ascii_case(&st.blok)) {
            let _ = v.tabulka(TableOperation::BlockDelete { name: st.blok.clone() });
        }
    }
    let jmeno = match v.tabulka(TableOperation::BlockRename { from: docasny.clone(), to: blok.clone() }) {
        Ok(_) => blok,
        Err(_) => docasny,
    };
    let mut ins = Insert::new(jmeno, v3(vlozeni));
    ins.common.layer = H_POHLED.into();
    let insert = v.pridej_jednu(EntityType::Insert(ins))?;
    let mut def = def.clone();
    def.otisk = otisk_s(doc, &def, nova);
    def.osy = Some([s.osy.od, s.osy.do_, s.osy.srovnavaci, s.osy.horni, s.osy.prevyseni]);
    MA_POHLEDY.store(true, std::sync::atomic::Ordering::SeqCst);
    let mut r = ExtendedDataRecord::new(APP);
    for x in def.zapis() {
        r.add_value(XDataValue::String(x));
    }
    if !v.zapis_xdata(insert, r) {
        return Err(tr("nepodařilo se uložit definici pohledu").into());
    }
    v.zapis_xdata(insert, crate::povrchy::zaznam_tridy("POHLED"));
    v.hotovo();
    Ok((insert, s))
}

/// Svislý text hodnoty v pásu (vlevo od svislé čáry místa).
fn svisly_text(t: String, x: f64, y_dole: f64, vyska: f64) -> EntityType {
    // u začátku pohledu napravo od čáry (vlevo je záhlaví pásu)
    let dx = if x < 1.5 * vyska { 1.25 * vyska } else { -0.25 * vyska };
    let p = [x + dx, y_dole + 0.4 * vyska];
    let mut e = Text::with_value(t, v3(p)).with_height(vyska).with_rotation(std::f64::consts::FRAC_PI_2);
    e.horizontal_alignment = TextHorizontalAlignment::Left;
    e.vertical_alignment = TextVerticalAlignment::Bottom;
    e.alignment_point = Some(v3(p));
    na(EntityType::Text(e), H_PASY)
}

/// Co potřebují pásy z pohledu.
struct KontextPasu<'a> {
    def: &'a DefinicePohledu,
    osy: &'a Osy,
    g: &'a Geometrie,
    /// Terénní profil povrchu pohledu.
    useky: &'a [Vec<[f64; 2]>],
    /// Profily dalších povrchů (id → úseky).
    dalsi: &'a [(String, Vec<Vec<[f64; 2]>>)],
    niveleta: Option<&'a trasy::niveleta::Niveleta>,
    h: f64,
    sirka_zahlavi: f64,
    /// Jen svislice (bez rámečků a vodorovných čar u hodnotových pásů).
    svislice: bool,
    fmt: FormatStanice,
    procenta: bool,
    /// Lomové body = vrcholy nivelety (šachty, profily toku).
    lomy_z_nivelety: bool,
}

/// Lomové body terénního profilu: místní maxima a minima a konce úseků.
fn lomy_profilu(useky: &[Vec<[f64; 2]>]) -> Vec<f64> {
    let mut out = Vec::new();
    for u in useky {
        if let (Some(a), Some(b)) = (u.first(), u.last()) {
            out.push(a[0]);
            out.push(b[0]);
        }
        for w in u.windows(3) {
            let (d1, d2) = (w[1][1] - w[0][1], w[2][1] - w[1][1]);
            if d1 * d2 < 0.0 && (d1.abs() + d2.abs()) > 0.02 {
                out.push(w[1][0]);
            }
        }
    }
    out
}

/// Pásy pod rámem a nad rámem pohledu, záhlaví vlevo. Vrací objekty a výšku
/// bloku pásů nad grafem (kvůli titulku).
fn pasy(k: &KontextPasu) -> (Vec<EntityType>, f64) {
    let mut ent = Vec::new();
    let (osy, h) = (k.osy, k.h);
    let w = osy.sirka();
    let m = k.def.meritko / 1000.0;
    let body: Vec<f64> = k.g.hlavni_body.iter().filter(|b| b.druh != DruhBodu::V).map(|b| b.staniceni).collect();
    let lomy_niv: Vec<f64> = k.niveleta.map_or(Vec::new(), |n| n.vrcholy.iter().map(|v| v.s).collect());
    let lomy_spolecne: Vec<f64> = if k.lomy_z_nivelety {
        lomy_niv.clone()
    } else {
        lomy_profilu(k.useky).into_iter().chain(lomy_niv.iter().copied()).collect()
    };
    let profil = |z: &Zdroj| -> Option<&[Vec<[f64; 2]>]> {
        match z {
            Zdroj::Teren => Some(k.useky),
            Zdroj::Povrch(id) if *id == k.def.povrch => Some(k.useky),
            Zdroj::Povrch(id) => k.dalsi.iter().find(|x| x.0 == *id).map(|x| &x.1[..]),
            Zdroj::Niveleta => None,
        }
    };
    let vyska = |z: &Zdroj, s: f64| -> Option<f64> {
        match z {
            Zdroj::Niveleta => k.niveleta.and_then(|n| n.vyska(s)),
            _ => profil(z).and_then(|u| vyska_v(u, s)),
        }
    };
    let x_od = |s: f64| (s.max(osy.od).min(osy.do_)) - osy.od;
    // rámeček textového řádku: vodorovné čáry, okraje a hranice úseků (sdílené
    // hrany sousedních řádků splynou – žádné zdvojení)
    let ramecek = |ent: &mut Vec<EntityType>, y0: f64, y1: f64, hranice: &[f64]| {
        ent.push(cara([0.0, y0], [w, y0], H_PASY));
        ent.push(cara([0.0, y1], [w, y1], H_PASY));
        for x in [0.0, w].iter().chain(hranice.iter()) {
            if *x >= -1e-9 && *x <= w + 1e-9 {
                ent.push(cara([*x, y0], [*x, y1], H_PASY));
            }
        }
    };
    let sklon_txt = |i: f64, des: usize| {
        if k.procenta { format!("{} %", cislo(i * 100.0, des + 1, true)) } else { format!("{} ‰", cislo(i * 1000.0, des, true)) }
    };

    let kresli = |ent: &mut Vec<EntityType>, pas: &Pas, y0: f64, y1: f64| {
        let ym = (y0 + y1) / 2.0;
        // záhlaví a rámeček řádku (jen v režimu s rámečky)
        if !k.svislice {
            for (a, b) in [
                ([-k.sirka_zahlavi, y0], [w, y0]),
                ([-k.sirka_zahlavi, y1], [w, y1]),
                ([-k.sirka_zahlavi, y0], [-k.sirka_zahlavi, y1]),
                ([0.0, y0], [0.0, y1]),
                ([w, y0], [w, y1]),
            ] {
                ent.push(cara(a, b, H_PASY));
            }
        }
        let zahlavi = if pas.nazev.trim().is_empty() { tr(pas.druh.nazev()).to_string() } else { pas.nazev.clone() };
        ent.push(text(zahlavi, [-k.sirka_zahlavi + 0.8 * h, ym], 0.9 * h, H_PASY, TextHorizontalAlignment::Left, TextVerticalAlignment::Middle));
        let text_useku = |ent: &mut Vec<EntityType>, a: f64, b: f64, t: String, stred: bool| {
            let (x, z) = if stred { ((a + b) / 2.0, TextHorizontalAlignment::Center) } else { (a + 0.6 * h, TextHorizontalAlignment::Left) };
            ent.push(text(t, [x, ym], 0.8 * h, H_PASY, z, TextVerticalAlignment::Middle));
        };
        match pas.druh {
            DruhPasu::Smer => {
                let d = 0.28 * pas.vyska * m;
                let mut x_pred = 0.0;
                for (s0, p) in &k.g.prvky {
                    let Prvek::Oblouk { r, sweep, .. } = *p else { continue };
                    let (a, b) = ((s0.max(osy.od) - osy.od), ((s0 + p.delka()).min(osy.do_) - osy.od));
                    if b <= a {
                        continue;
                    }
                    let dy = if sweep > 0.0 { d } else { -d };
                    ent.push(cara([x_pred, ym], [a, ym], H_PASY));
                    ent.push(cara([a, ym], [a, ym + dy], H_PASY));
                    ent.push(cara([a, ym + dy], [b, ym + dy], H_PASY));
                    ent.push(cara([b, ym + dy], [b, ym], H_PASY));
                    let (ty, zarovnani) = if dy > 0.0 { (ym - 0.3 * h, TextVerticalAlignment::Top) } else { (ym + 0.3 * h, TextVerticalAlignment::Bottom) };
                    ent.push(text(format!("R = {}", cislo(r, 2, true)), [(a + b) / 2.0, ty], 0.8 * h, H_PASY, TextHorizontalAlignment::Center, zarovnani));
                    x_pred = b;
                }
                ent.push(cara([x_pred, ym], [w, ym], H_PASY));
            }
            DruhPasu::Sklony => {
                let Some(n) = k.niveleta else { return };
                let useky: Vec<_> = n.useky().into_iter().filter(|u| u.do_ > osy.od && u.od < osy.do_).collect();
                // nízký řádek: jen text „sklon – délka“ v rámečku
                if pas.vyska < 8.0 {
                    let hranice: Vec<f64> = useky.iter().flat_map(|u| [x_od(u.od), x_od(u.do_)]).collect();
                    ramecek(ent, y0, y1, &hranice);
                    for u in &useky {
                        text_useku(ent, x_od(u.od), x_od(u.do_), format!("{} – {}", sklon_txt(u.sklon, pas.desetin), cislo(u.do_ - u.od, 2, true)), pas.na_stred);
                    }
                    return;
                }
                if k.svislice {
                    ramecek(ent, y0, y1, &[]);
                }
                for u in &useky {
                    let (a, b) = (x_od(u.od), x_od(u.do_));
                    if b <= a {
                        continue;
                    }
                    ent.push(cara([a, y0], [a, y1], H_PASY));
                    ent.push(cara([b, y0], [b, y1], H_PASY));
                    let (ya, yb) = if u.sklon >= 0.0 { (y1, y0) } else { (y0, y1) };
                    ent.push(cara([a, ya], [b, yb], H_PASY));
                    ent.push(text(sklon_txt(u.sklon, pas.desetin), [(a + b) / 2.0, ym + 0.15 * h], 0.8 * h, H_PASY, TextHorizontalAlignment::Center, TextVerticalAlignment::Bottom));
                    ent.push(text(format!("{} m", cislo(u.do_ - u.od, 2, true)), [(a + b) / 2.0, ym - 0.15 * h], 0.8 * h, H_PASY, TextHorizontalAlignment::Center, TextVerticalAlignment::Top));
                }
            }
            DruhPasu::Vzdalenosti => {
                let mista = mista_bez_kolizi(pas, osy.od, osy.do_, &lomy_spolecne, &body, 1.1 * h);
                let hranice: Vec<f64> = mista.iter().map(|s| s - osy.od).collect();
                ramecek(ent, y0, y1, &hranice);
                for w2 in mista.windows(2) {
                    text_useku(ent, w2[0] - osy.od, w2[1] - osy.od, cislo(w2[1] - w2[0], 2, true), true);
                }
            }
            DruhPasu::Text => {
                ramecek(ent, y0, y1, &[]);
                text_useku(ent, 0.0, w, pas.text.clone(), pas.na_stred);
            }
            DruhPasu::Useky => {
                let useky: Vec<_> = pas.useky.iter().filter(|u| u.do_ > osy.od && u.od < osy.do_).collect();
                let hranice: Vec<f64> = useky.iter().flat_map(|u| [x_od(u.od), x_od(u.do_)]).collect();
                ramecek(ent, y0, y1, &hranice);
                for u in useky {
                    text_useku(ent, x_od(u.od), x_od(u.do_), u.text.clone(), pas.na_stred);
                }
            }
            druh => {
                let lomy: Vec<f64> = match (&pas.druh, &pas.zdroj) {
                    (DruhPasu::Kota, z @ Zdroj::Povrch(_)) if !k.lomy_z_nivelety => {
                        profil(z).map(lomy_profilu).unwrap_or_default().into_iter().chain(lomy_spolecne.iter().copied()).collect()
                    }
                    _ => lomy_spolecne.clone(),
                };
                // rozestup ≈ výška svislého textu, ať se hodnoty nepřekrývají
                for s in mista_bez_kolizi(pas, osy.od, osy.do_, &lomy, &body, 1.1 * h) {
                    let x = s - osy.od;
                    let hodnota = match druh {
                        DruhPasu::Staniceni => {
                            let cele = pas.krok > 0.0 && ((s / pas.krok).round() * pas.krok - s).abs() < 1e-6;
                            Some(staniceni(s, k.fmt, if cele { 0 } else { pas.desetin }, true))
                        }
                        DruhPasu::Teren => vyska(&Zdroj::Teren, s).map(|z| cislo(z, pas.desetin, true)),
                        DruhPasu::Niveleta => vyska(&Zdroj::Niveleta, s).map(|z| cislo(z, pas.desetin, true)),
                        DruhPasu::Kota => vyska(&pas.zdroj, s).map(|z| cislo(z, pas.desetin, true)),
                        DruhPasu::Rozdil => match (vyska(&pas.zdroj, s), vyska(&pas.zdroj2, s)) {
                            (Some(a), Some(b)) => {
                                let d = a - b;
                                // „+“ jen u výkopu/násypu (niveleta − terén), ne u hloubek
                                let plus = d > 0.0 && pas.zdroj == Zdroj::Niveleta;
                                Some(format!("{}{}", if plus { "+" } else { "" }, cislo(d, pas.desetin, true)))
                            }
                            _ => None,
                        },
                        _ => None,
                    };
                    let Some(t) = hodnota else { continue };
                    ent.push(cara([x, y0], [x, y1], H_PASY));
                    ent.push(svisly_text(t, x, y1, 0.85 * h));
                }
            }
        }
    };

    let nad: Vec<&Pas> = k.def.pasy.iter().filter(|p| p.poloha == Poloha::Nad).collect();
    let pod: Vec<&Pas> = k.def.pasy.iter().filter(|p| p.poloha != Poloha::Nad).collect();
    let vyska_nad = if nad.is_empty() { 0.0 } else { nad.iter().map(|p| p.vyska * m).sum::<f64>() + 2.0 * h };
    let mut y0 = osy.vyska() + vyska_nad;
    for p in nad {
        let y1 = y0 - p.vyska * m;
        kresli(&mut ent, p, y0, y1);
        y0 = y1;
    }
    let mut y0 = 0.0;
    for p in pod {
        let y1 = y0 - p.vyska * m;
        kresli(&mut ent, p, y0, y1);
        y0 = y1;
    }
    (ent, vyska_nad)
}
