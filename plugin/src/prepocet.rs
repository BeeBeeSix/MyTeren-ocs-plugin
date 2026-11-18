//! Režim přepočtu povrchů a pohledů profilu po změně výkresu.
//!
//! - **Automaticky:** přepočítá se hned po každé změně (dosavadní chování).
//! - **Po dokončení úprav:** během série úprav se povrchy a pohledy jen
//!   označí jako zastaralé; přepočet proběhne jednou, až je chvíli klid.
//! - **Pozastaveno:** nepřepočítává se nic, dokud uživatel nezvolí
//!   „Přepočítat teď“ nebo nepřepne zpět.
//!
//! Místa, která by přepočítávala, se ptají [`smi`]; když nesmí, zavolají
//! [`odloz`]. Pracovní vlákno správce pak podle [`je_cas`] přepočet provede
//! (uvnitř [`Povoleni`]).

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rezim {
    Automaticky,
    PoDokonceni,
    Pozastaveno,
}

impl Rezim {
    pub fn klic(self) -> &'static str {
        match self {
            Rezim::Automaticky => "auto",
            Rezim::PoDokonceni => "po_dokonceni",
            Rezim::Pozastaveno => "pozastaveno",
        }
    }
    pub fn z_klice(k: &str) -> Option<Rezim> {
        match k {
            "auto" => Some(Rezim::Automaticky),
            "po_dokonceni" => Some(Rezim::PoDokonceni),
            "pozastaveno" => Some(Rezim::Pozastaveno),
            _ => None,
        }
    }
    fn z_u8(v: u8) -> Rezim {
        match v {
            1 => Rezim::PoDokonceni,
            2 => Rezim::Pozastaveno,
            _ => Rezim::Automaticky,
        }
    }
    fn u8(self) -> u8 {
        match self {
            Rezim::Automaticky => 0,
            Rezim::PoDokonceni => 1,
            Rezim::Pozastaveno => 2,
        }
    }
}

/// Jak dlouho musí být klid, než se v režimu „Po dokončení úprav“ přepočítá.
pub const KLID: Duration = Duration::from_millis(2500);

static REZIM: AtomicU8 = AtomicU8::new(0);
thread_local! {
    /// V tomto vlákně právě probíhá vyžádaný přepočet – kontroly ho nesmí
    /// odložit. Jen pro vlákno, které přepočítává: jiné vlákno (hlídání
    /// úchopů) by jinak ve stejnou chvíli překreslovalo totéž.
    static POVOLENO: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
/// Uživatel chce přepočítat hned („Přepočítat teď“, přepnutí na Automaticky).
static TED: AtomicBool = AtomicBool::new(false);
/// Od kdy něco čeká na přepočet (čas poslední odložené změny).
static CEKA: Mutex<Option<Instant>> = Mutex::new(None);

fn ceka() -> std::sync::MutexGuard<'static, Option<Instant>> {
    CEKA.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn rezim() -> Rezim {
    Rezim::z_u8(REZIM.load(Ordering::SeqCst))
}

/// Nastaví režim. Přepnutí na Automaticky přepočítá, co čeká.
pub fn nastav(r: Rezim) {
    REZIM.store(r.u8(), Ordering::SeqCst);
    if r == Rezim::Automaticky && ceka().is_some() {
        TED.store(true, Ordering::SeqCst);
    }
}

/// Smí se teď přepočítat (povrch, pohled)?
pub fn smi() -> bool {
    rezim() == Rezim::Automaticky || POVOLENO.with(|p| p.get())
}

/// Přepočet se odložil – zapamatovat (a posunout čekání na klid).
pub fn odloz() {
    *ceka() = Some(Instant::now());
}

/// Čeká něco na přepočet?
pub fn ceka_neco() -> bool {
    ceka().is_some()
}

/// Přepočítat hned („Přepočítat teď“) – i když nic neodložila tato relace
/// (např. zastaralý pohled po otevření výkresu).
pub fn pozadej() {
    TED.store(true, Ordering::SeqCst);
    let mut c = ceka();
    if c.is_none() {
        *c = Some(Instant::now());
    }
}

/// Je čas provést odložený přepočet?
pub fn je_cas() -> bool {
    let Some(od) = *ceka() else { return false };
    if TED.load(Ordering::SeqCst) {
        return true;
    }
    match rezim() {
        Rezim::Automaticky => true,
        Rezim::PoDokonceni => od.elapsed() >= KLID,
        Rezim::Pozastaveno => false,
    }
}

/// Po dobu života povolí přepočet; na začátku zruší čekání.
pub struct Povoleni;

impl Povoleni {
    pub fn zacni() -> Self {
        TED.store(false, Ordering::SeqCst);
        *ceka() = None;
        POVOLENO.with(|p| p.set(true));
        Povoleni
    }
}

impl Drop for Povoleni {
    fn drop(&mut self) {
        POVOLENO.with(|p| p.set(false));
    }
}

/// Po dobu života povolí přepočet bez zrušení čekání – pro úpravy, jejichž
/// výsledek je vidět jen v pohledu (niveleta), takže odložit nejdou.
pub struct Vynuceni(bool);

impl Vynuceni {
    pub fn zacni() -> Self {
        Vynuceni(POVOLENO.with(|p| p.replace(true)))
    }
}

impl Drop for Vynuceni {
    fn drop(&mut self) {
        POVOLENO.with(|p| p.set(self.0));
    }
}

#[cfg(test)]
mod testy {
    use super::*;

    // Jeden test – stav je globální a testy běží souběžně.
    #[test]
    fn rezimy_a_cekani() {
        assert_eq!(Rezim::z_klice(Rezim::PoDokonceni.klic()), Some(Rezim::PoDokonceni));
        assert_eq!(Rezim::z_klice("x"), None);

        nastav(Rezim::Automaticky);
        assert!(smi());

        nastav(Rezim::PoDokonceni);
        assert!(!smi());
        assert!(!je_cas());
        odloz();
        assert!(ceka_neco());
        assert!(!je_cas(), "hned po změně ještě ne – čeká se na klid");
        *ceka() = Some(Instant::now() - KLID);
        assert!(je_cas());
        {
            let _p = Povoleni::zacni();
            assert!(smi());
            assert!(!ceka_neco());
        }
        assert!(!smi());

        nastav(Rezim::Pozastaveno);
        odloz();
        *ceka() = Some(Instant::now() - KLID * 10);
        assert!(!je_cas(), "pozastaveno – jen na povel");
        pozadej();
        assert!(je_cas());
        drop(Povoleni::zacni());

        // přepnutí na Automaticky přepočítá, co čeká
        nastav(Rezim::Pozastaveno);
        odloz();
        nastav(Rezim::Automaticky);
        assert!(je_cas());
        drop(Povoleni::zacni());
        assert!(!je_cas());
    }
}
