//! Interaktivní příkazy: výběr objektu kliknutím, kreslení trasy vrcholy.
//!
//! Interaktivní příkaz nemá přístup k výkresu – sbírá kliknutí a na konci
//! předá výsledek příkazem (`CommandStep::Dispatch`), který už výkres má.

use std::collections::HashMap;
use std::sync::Mutex;

use ocs_plugin_api::dialog::{DialogDef, DialogResponse, DialogValue};

use ocs_plugin_api::host::{CommandStep, Handle, InteractiveCommand, PreviewWire};
use trasy::geometrie::{geometrie, Prvek, Vrchol};
use trasy::preklad::tr;
use trasy::trf;

/// Vybere jeden objekt kliknutím a spustí `prikaz <handle>`.
pub struct VyberObjektu {
    pub vyzva: &'static str,
    pub prikaz: &'static str,
}

impl InteractiveCommand for VyberObjektu {
    fn prompt(&self) -> String {
        tr(self.vyzva).to_string()
    }
    fn needs_object_pick(&self) -> bool {
        true
    }
    fn on_object_pick(&mut self, h: Handle, pt: [f64; 3]) -> CommandStep {
        CommandStep::Dispatch(format!("{} {:X} {:.4} {:.4}", self.prikaz, h.value(), pt[0], pt[1]))
    }
    fn on_point(&mut self, _pt: [f64; 3]) -> CommandStep {
        CommandStep::NeedPoint
    }
    fn on_enter(&mut self) -> CommandStep {
        CommandStep::Cancel
    }
}

/// Klik do výkresu → spustí `prikaz x y` (nástroje vrcholů editoru).
pub struct VyberBodu {
    pub vyzva: &'static str,
    pub prikaz: &'static str,
}

impl InteractiveCommand for VyberBodu {
    fn prompt(&self) -> String {
        tr(self.vyzva).to_string()
    }
    fn on_point(&mut self, pt: [f64; 3]) -> CommandStep {
        CommandStep::Dispatch(format!("{} {:.4} {:.4}", self.prikaz, pt[0], pt[1]))
    }
    fn on_enter(&mut self) -> CommandStep {
        CommandStep::Cancel
    }
}

/// Výsledek kreslení: vrcholy a id prodlužované trasy (převezme je
/// `TRA_KRESLIT_HOTOVO`).
pub static NAKRESLENO: Mutex<Option<(Vec<Vrchol>, Option<String>)>> = Mutex::new(None);

/// Kreslení trasy klikáním vrcholů tečnového polygonu. Každý vrchol dostane
/// poloměr, který je v panelu editoru v okamžiku kliknutí.
pub struct KresleniTrasy {
    /// Vrcholy (x, y, R).
    pub body: Vec<[f64; 3]>,
    /// Prodlužovaná trasa (id) a kolik vrcholů měla – ty se nevracejí.
    pub trasa: Option<(String, usize)>,
}

impl KresleniTrasy {
    pub fn nova() -> Self {
        Self { body: Vec::new(), trasa: None }
    }
    /// Pokračování z konce existující trasy; poslední vrchol se stane
    /// vnitřním a dostane poloměr z panelu.
    pub fn prodlouzeni(id: String, v: &[Vrchol]) -> Self {
        let mut body: Vec<[f64; 3]> = v.iter().map(|x| [x.x, x.y, x.r]).collect();
        if let Some(b) = body.last_mut() {
            b[2] = crate::tra::editor::polomer();
        }
        let n = body.len();
        Self { body, trasa: Some((id, n)) }
    }
    fn minimum(&self) -> usize {
        self.trasa.as_ref().map_or(0, |t| t.1)
    }
    fn vrcholy(&self, dalsi: Option<[f64; 2]>) -> Vec<Vrchol> {
        let mut b = self.body.clone();
        b.extend(dalsi.map(|p| [p[0], p[1], 0.0]));
        let n = b.len();
        b.iter().enumerate().map(|(i, p)| Vrchol::new(p[0], p[1], if i == 0 || i + 1 == n { 0.0 } else { p[2] })).collect()
    }
    fn vratit_pokud_chce(&mut self) {
        if crate::tra::editor::vezmi_vratit() && self.body.len() > self.minimum() {
            self.body.pop();
        }
    }
}

impl InteractiveCommand for KresleniTrasy {
    fn prompt(&self) -> String {
        let r = crate::tra::editor::polomer();
        if self.body.is_empty() {
            tr("Trasa: klikněte začátek trasy").to_string()
        } else {
            trf!("Trasa: klikněte vrchol č. {} (R = {} m z panelu; Enter = hotovo, Esc = zrušit)", self.body.len() + 1, r)
        }
    }
    fn on_point(&mut self, pt: [f64; 3]) -> CommandStep {
        self.vratit_pokud_chce();
        let p = [pt[0], pt[1]];
        if self.body.last().is_some_and(|q| (q[0] - p[0]).hypot(q[1] - p[1]) < 1e-6) {
            return CommandStep::NeedPoint;
        }
        self.body.push([p[0], p[1], crate::tra::editor::polomer()]);
        CommandStep::NeedPoint
    }
    fn on_enter(&mut self) -> CommandStep {
        self.vratit_pokud_chce();
        if self.body.len() < 2 || self.body.len() <= self.minimum() {
            return CommandStep::Cancel;
        }
        let id = self.trasa.as_ref().map(|t| t.0.clone());
        *NAKRESLENO.lock().unwrap_or_else(|e| e.into_inner()) = Some((self.vrcholy(None), id));
        CommandStep::Dispatch("TRA_KRESLIT_HOTOVO".into())
    }
    fn on_cursor_move(&mut self, pt: [f64; 3]) -> Vec<PreviewWire> {
        self.vratit_pokud_chce();
        if self.body.is_empty() {
            return Vec::new();
        }
        // poslední kliknutý vrchol se stane vnitřním s poloměrem z panelu
        if self.body.len() > self.minimum() {
            if let Some(b) = self.body.last_mut() {
                b[2] = crate::tra::editor::polomer();
            }
        }
        let v = self.vrcholy(Some([pt[0], pt[1]]));
        let polygon = PreviewWire::with_color(v.iter().map(|x| [x.x, x.y, 0.0]).collect(), [0.6, 0.6, 0.6, 1.0]);
        let mut out = vec![polygon];
        match geometrie(&v, 0.0) {
            Ok(g) => {
                for (_, p) in &g.prvky {
                    match *p {
                        Prvek::Primka { a, b } => out.push(PreviewWire::line([a[0], a[1], 0.0], [b[0], b[1], 0.0], None)),
                        Prvek::Oblouk { stred, r, uhel_a, sweep, .. } => {
                            out.push(crate::tra::uchopy::oblouk_nahledu(stred, r, uhel_a, sweep, None))
                        }
                    }
                }
            }
            Err(_) => out[0] = PreviewWire::with_color(v.iter().map(|x| [x.x, x.y, 0.0]).collect(), [1.0, 0.2, 0.2, 1.0]),
        }
        out
    }
}

/// Odpověď posledního dialogu (převezme ji příkaz z `DialogPrikaz::prikaz`).
pub static ODPOVED: Mutex<Option<HashMap<String, DialogValue>>> = Mutex::new(None);

pub fn vezmi_odpoved() -> Option<HashMap<String, DialogValue>> {
    ODPOVED.lock().unwrap_or_else(|e| e.into_inner()).take()
}

/// Dialog Open CAD Studia. Odpověď dialogu dostane jen běžící interaktivní
/// příkaz – tenhle ji uloží do [`ODPOVED`] a spustí `prikaz`, který ji použije.
pub struct DialogPrikaz {
    pub def: DialogDef,
    pub prikaz: String,
    pub hotovo: bool,
    /// Po potvrzení dialogu ještě klik do výkresu (výzva); příkaz pak dostane
    /// souřadnice `x y`. Dialog musí být první – OCS ho po kliknutí neotevře.
    pub klik: Option<&'static str>,
}

impl DialogPrikaz {
    pub fn new(def: DialogDef, prikaz: String) -> Self {
        Self { def, prikaz, hotovo: false, klik: None }
    }
    pub fn s_klikem(def: DialogDef, prikaz: String, vyzva: &'static str) -> Self {
        Self { def, prikaz, hotovo: false, klik: Some(vyzva) }
    }
}

impl InteractiveCommand for DialogPrikaz {
    fn prompt(&self) -> String {
        match (self.hotovo, self.klik) {
            (true, Some(v)) => tr(v).to_string(),
            _ => self.def.title.clone(),
        }
    }
    fn on_point(&mut self, pt: [f64; 3]) -> CommandStep {
        if self.hotovo && self.klik.is_some() {
            return CommandStep::Dispatch(format!("{} {:.4} {:.4}", self.prikaz, pt[0], pt[1]));
        }
        CommandStep::NeedPoint
    }
    fn on_enter(&mut self) -> CommandStep {
        CommandStep::Cancel
    }
    fn modal_dialog(&self) -> Option<DialogDef> {
        (!self.hotovo).then(|| self.def.clone())
    }
    fn on_dialog_response(&mut self, r: DialogResponse) -> CommandStep {
        self.hotovo = true;
        match r {
            DialogResponse::Submitted(m) => {
                *ODPOVED.lock().unwrap_or_else(|e| e.into_inner()) = Some(m);
                if self.klik.is_some() {
                    CommandStep::NeedPoint
                } else {
                    CommandStep::Dispatch(self.prikaz.clone())
                }
            }
            _ => CommandStep::Cancel,
        }
    }
}

/// Oblouk ve vrcholu bez dialogu: klik na vrchol, pak poloměr – napsaný do
/// příkazového řádku (OCS ho předá jako bod v té vzdálenosti od kliknutí),
/// nebo kliknutý (poloměr = vzdálenost od vrcholu). Enter = poloměr z panelu.
pub struct ObloukVeVrcholu {
    /// Trasy výkresu: vložení, vrcholy (světové), počáteční staničení.
    pub trasy: Vec<(Handle, Vec<Vrchol>, f64)>,
    /// Vybraný vrchol: (index trasy, index vrcholu, bod kliknutí).
    pub vybrany: Option<(usize, usize, [f64; 2])>,
}

impl ObloukVeVrcholu {
    fn prikaz(&self, r: f64) -> CommandStep {
        match self.vybrany {
            Some((t, i, _)) => CommandStep::Dispatch(format!("TRA_OBLOUK_NASTAV {:X} {} {:.4}", self.trasy[t].0.value(), i, r.max(0.0))),
            None => CommandStep::Cancel,
        }
    }
}

impl InteractiveCommand for ObloukVeVrcholu {
    fn prompt(&self) -> String {
        match self.vybrany {
            None => tr("Oblouk: klikněte na vrchol trasy").to_string(),
            Some((_, i, _)) => trf!(
                "Oblouk ve vrcholu {}: napište poloměr R a Enter (nebo klikněte – R = vzdálenost od vrcholu; jen Enter = R {} m z panelu)",
                i + 1,
                crate::tra::editor::polomer()
            ),
        }
    }
    fn on_point(&mut self, pt: [f64; 3]) -> CommandStep {
        let p = [pt[0], pt[1]];
        if let Some((_, _, p1)) = self.vybrany {
            let r = (p[0] - p1[0]).hypot(p[1] - p1[1]);
            return self.prikaz(r);
        }
        let mut nej: Option<(f64, usize, usize)> = None;
        for (t, (_, v, _)) in self.trasy.iter().enumerate() {
            for i in 1..v.len().saturating_sub(1) {
                let d = (v[i].x - p[0]).hypot(v[i].y - p[1]);
                if nej.is_none_or(|n| d < n.0) {
                    nej = Some((d, t, i));
                }
            }
        }
        if let Some((_, t, i)) = nej {
            self.vybrany = Some((t, i, p));
        }
        CommandStep::NeedPoint
    }
    fn on_enter(&mut self) -> CommandStep {
        self.prikaz(crate::tra::editor::polomer())
    }
    fn on_cursor_move(&mut self, pt: [f64; 3]) -> Vec<PreviewWire> {
        let Some((t, i, p1)) = self.vybrany else { return Vec::new() };
        let r = (pt[0] - p1[0]).hypot(pt[1] - p1[1]);
        let (_, v, pocatek) = &self.trasy[t];
        let mut v = v.clone();
        v[i].r = r;
        let mut out = vec![PreviewWire::line([p1[0], p1[1], 0.0], [pt[0], pt[1], 0.0], Some([0.6, 0.6, 0.6, 1.0]))];
        match geometrie(&v, *pocatek) {
            Ok(g) => {
                for (_, p) in &g.prvky {
                    match *p {
                        Prvek::Primka { a, b } => {
                            out.push(PreviewWire::line([a[0], a[1], 0.0], [b[0], b[1], 0.0], Some([1.0, 0.8, 0.2, 1.0])))
                        }
                        Prvek::Oblouk { stred, r, uhel_a, sweep, .. } => {
                            out.push(crate::tra::uchopy::oblouk_nahledu(stred, r, uhel_a, sweep, Some([1.0, 0.8, 0.2, 1.0])))
                        }
                    }
                }
            }
            Err(_) => out.push(PreviewWire::with_color(v.iter().map(|x| [x.x, x.y, 0.0]).collect(), [1.0, 0.2, 0.2, 1.0])),
        }
        out
    }
}

/// Pohled pro kreslení nivelety: id, vložení (levý dolní roh), osy.
pub struct PohledKresleni {
    pub id: String,
    pub vlozeni: [f64; 2],
    /// od, do, srovnávací rovina, horní okraj, převýšení
    pub osy: [f64; 5],
}

impl PohledKresleni {
    fn obsahuje(&self, p: [f64; 2]) -> bool {
        let [od, do_, sr, horni, prev] = self.osy;
        let (x, y) = (p[0] - self.vlozeni[0], p[1] - self.vlozeni[1]);
        x >= -1e-6 && x <= do_ - od + 1e-6 && y >= -1e-6 && y <= (horni - sr) * prev + 1e-6
    }
    /// Bod výkresu → (staničení, výška).
    fn na_profil(&self, p: [f64; 2]) -> (f64, f64) {
        let [od, do_, sr, _, prev] = self.osy;
        ((od + p[0] - self.vlozeni[0]).clamp(od, do_), sr + (p[1] - self.vlozeni[1]) / prev)
    }
    /// (staničení, výška) → bod výkresu.
    fn na_vykres(&self, s: f64, z: f64) -> [f64; 3] {
        let [od, _, sr, _, prev] = self.osy;
        [self.vlozeni[0] + s - od, self.vlozeni[1] + (z - sr) * prev, 0.0]
    }
}

/// Výsledek kreslení nivelety: id pohledu a vrcholy.
pub static NAKRESLENA_NIVELETA: Mutex<Option<(String, Vec<trasy::niveleta::VrcholNivelety>)>> = Mutex::new(None);

/// Výchozí poloměr zakružovacích oblouků nivelety [m].
pub const R_NIVELETY: f64 = 2000.0;

/// Kreslení nivelety klikáním vrcholů přímo v pohledu podélného profilu.
pub struct KresleniNivelety {
    pub pohledy: Vec<PohledKresleni>,
    pub vybrany: Option<usize>,
    pub body: Vec<(f64, f64)>,
}

impl KresleniNivelety {
    fn vrcholy(&self, dalsi: Option<(f64, f64)>) -> Vec<trasy::niveleta::VrcholNivelety> {
        let mut b = self.body.clone();
        b.extend(dalsi);
        let n = b.len();
        b.iter()
            .enumerate()
            .map(|(i, &(s, z))| trasy::niveleta::VrcholNivelety::new(s, z, if i == 0 || i + 1 == n { 0.0 } else { R_NIVELETY }))
            .collect()
    }
}

impl InteractiveCommand for KresleniNivelety {
    fn prompt(&self) -> String {
        if self.body.is_empty() {
            tr("Niveleta: klikněte v pohledu profilu začátek nivelety").to_string()
        } else {
            trf!("Niveleta: klikněte vrchol č. {} (oblouky R = {} m; Enter = hotovo, Esc = zrušit)", self.body.len() + 1, R_NIVELETY)
        }
    }
    fn on_point(&mut self, pt: [f64; 3]) -> CommandStep {
        let p = [pt[0], pt[1]];
        if self.vybrany.is_none() {
            self.vybrany = self.pohledy.iter().position(|v| v.obsahuje(p));
            if self.vybrany.is_none() {
                return CommandStep::NeedPoint;
            }
        }
        let Some(v) = self.vybrany.and_then(|i| self.pohledy.get(i)) else { return CommandStep::NeedPoint };
        let (s, z) = v.na_profil(p);
        if self.body.last().is_some_and(|q| s <= q.0 + 1e-3) {
            return CommandStep::NeedPoint; // vrcholy musí jít dopředu ve staničení
        }
        self.body.push((s, z));
        CommandStep::NeedPoint
    }
    fn on_enter(&mut self) -> CommandStep {
        let Some(v) = self.vybrany.and_then(|i| self.pohledy.get(i)) else { return CommandStep::Cancel };
        if self.body.len() < 2 {
            return CommandStep::Cancel;
        }
        *NAKRESLENA_NIVELETA.lock().unwrap_or_else(|e| e.into_inner()) = Some((v.id.clone(), self.vrcholy(None)));
        CommandStep::Dispatch("TRA_NIVELETA_HOTOVO".into())
    }
    fn on_cursor_move(&mut self, pt: [f64; 3]) -> Vec<PreviewWire> {
        let Some(v) = self.vybrany.and_then(|i| self.pohledy.get(i)) else { return Vec::new() };
        if self.body.is_empty() {
            return Vec::new();
        }
        let (s, z) = v.na_profil([pt[0], pt[1]]);
        let dalsi = (s > self.body.last().map_or(f64::MIN, |q| q.0) + 1e-3).then_some((s, z));
        let n = trasy::niveleta::Niveleta::nova("N", "", self.vrcholy(dalsi));
        let polygon = PreviewWire::with_color(n.vrcholy.iter().map(|x| v.na_vykres(x.s, x.z)).collect(), [0.6, 0.6, 0.6, 1.0]);
        let mut out = vec![polygon];
        if n.kontrola().is_ok() && n.vrcholy.len() >= 2 {
            out.push(PreviewWire::with_color(n.body().into_iter().map(|b| v.na_vykres(b[0], b[1])).collect(), [1.0, 0.3, 0.3, 1.0]));
        } else {
            out[0] = PreviewWire::with_color(n.vrcholy.iter().map(|x| v.na_vykres(x.s, x.z)).collect(), [1.0, 0.2, 0.2, 1.0]);
        }
        out
    }
}
