//! Niveleta (návrhový profil): vrcholy (PVI) se zakružovacími oblouky.
//!
//! Zakružovací oblouk je kvadratická parabola zadaná vrcholovým poloměrem R
//! (česká praxe): tečna T = R·|i₂ − i₁| / 2, vzepětí y = T² / (2R).
//! Oblouk jde zadat i délkou L = R·|i₂ − i₁| (= 2T) nebo hodnotou
//! K = L / A [m na 1 % lomu] = R / 100 (praxe 12d, AASHTO, Austroads).
//! Zadaná délka L zůstává při změně sklonů stejná, poloměr se dopočte.

/// Čím je zakružovací oblouk zadaný (co zůstane při změně sklonů).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Zadani {
    /// Vrcholovým poloměrem R (pole `r`).
    #[default]
    R,
    /// Hodnotou K = R / 100 (drží se R, jen se tak zadává a vypisuje).
    K,
    /// Délkou oblouku L [m] – při změně sklonů se drží L.
    L(f64),
}

impl Zadani {
    /// Písmeno zadání (R, L, K).
    pub fn pismeno(self) -> &'static str {
        match self {
            Zadani::R => "R",
            Zadani::K => "K",
            Zadani::L(_) => "L",
        }
    }
}

/// Typ zakružovacího oblouku.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum TypOblouku {
    /// Kvadratická parabola souměrná podle vrcholu (výchozí): T = R·Δi/2, y = x²/2R.
    #[default]
    Parabola,
    /// Nesouměrná parabola: délka `l1` před vrcholem a `l2` za ním (vodorovně),
    /// výška po částech; R a K nemají smysl.
    Nesoumerny { l1: f64, l2: f64 },
    /// Přesná kružnice o poloměru R (železnice, koleje).
    Kruh,
}

impl TypOblouku {
    /// Index v nabídce typů (parabola, nesouměrný, kružnice).
    pub fn index(self) -> usize {
        match self {
            TypOblouku::Parabola => 0,
            TypOblouku::Nesoumerny { .. } => 1,
            TypOblouku::Kruh => 2,
        }
    }
}

/// Vrchol nivelety.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VrcholNivelety {
    /// Staničení [m].
    pub s: f64,
    /// Výška [m n. m.].
    pub z: f64,
    /// Poloměr zakružovacího oblouku [m]; 0 = bez oblouku. U krajních vrcholů se nepoužije.
    /// U zadání délkou je to poslední dopočtený poloměr (platí `Niveleta::polomer`).
    pub r: f64,
    pub zadani: Zadani,
    pub typ: TypOblouku,
}

impl VrcholNivelety {
    pub fn new(s: f64, z: f64, r: f64) -> Self {
        Self { s, z, r, zadani: Zadani::R, typ: TypOblouku::Parabola }
    }

    /// Vrchol s obloukem zadaným písmenem a hodnotou (R, L nebo K; hodnota ≤ 0 = bez oblouku).
    pub fn zadany(s: f64, z: f64, zadani: &str, hodnota: f64) -> Self {
        let h = hodnota.max(0.0);
        match zadani {
            "L" | "l" if h > 0.0 => Self { r: 0.0, zadani: Zadani::L(h), ..Self::new(s, z, 0.0) },
            "K" | "k" => Self { zadani: Zadani::K, ..Self::new(s, z, h * 100.0) },
            _ => Self::new(s, z, h),
        }
    }

    /// Stejný vrchol s jiným zadáním oblouku (typ oblouku zůstane; u nesouměrného
    /// se délka L rozdělí v dosavadním poměru L1 : L2).
    pub fn se_zadanim(self, zadani: &str, hodnota: f64) -> Self {
        match self.typ {
            TypOblouku::Nesoumerny { l1, l2 } => {
                let l = hodnota.max(0.0);
                let podil = if l1 + l2 > 0.0 { l1 / (l1 + l2) } else { 0.5 };
                Self { typ: TypOblouku::Nesoumerny { l1: l * podil, l2: l - l * podil }, ..self }
            }
            typ => Self { typ, ..Self::zadany(self.s, self.z, zadani, hodnota) },
        }
    }
}

/// Sklony před a za vnitřním vrcholem `k`.
fn sklony_u(v: &[VrcholNivelety], k: usize) -> Option<(f64, f64)> {
    if k == 0 || k + 1 >= v.len() {
        return None;
    }
    let i1 = (v[k].z - v[k - 1].z) / (v[k].s - v[k - 1].s);
    let i2 = (v[k + 1].z - v[k].z) / (v[k + 1].s - v[k].s);
    (i1.is_finite() && i2.is_finite()).then_some((i1, i2))
}

/// Vodorovná délka oblouku o poloměru `r` mezi sklony `i1` a `i2`.
fn delka_z_r(kruh: bool, r: f64, i1: f64, i2: f64) -> f64 {
    if kruh {
        let (a1, a2) = (i1.atan(), i2.atan());
        r * ((a2 - a1).abs() / 2.0).tan() * (a1.cos() + a2.cos())
    } else {
        r * (i2 - i1).abs()
    }
}

/// Poloměr oblouku o vodorovné délce `l` mezi sklony `i1` a `i2`.
fn r_z_delky(kruh: bool, l: f64, i1: f64, i2: f64) -> f64 {
    let jedna = delka_z_r(kruh, 1.0, i1, i2);
    if jedna > 1e-12 { l / jedna } else { 0.0 }
}

/// Nastaví vrcholu `i` poloměr tak, aby zadání zůstalo (u délky L se přepočte
/// délka, u nesouměrného oblouku se L1 a L2 změní ve stejném poměru).
pub fn nastav_polomer(v: &mut [VrcholNivelety], i: usize, r: f64) {
    let r = r.max(0.0);
    let Some((i1, i2)) = sklony_u(v, i) else {
        v[i].r = r;
        return;
    };
    let kruh = v[i].typ == TypOblouku::Kruh;
    if let TypOblouku::Nesoumerny { l1, l2 } = v[i].typ {
        let l = delka_z_r(false, r, i1, i2);
        if l1 + l2 > 1e-9 && l > 1e-9 {
            let q = l / (l1 + l2);
            v[i].typ = TypOblouku::Nesoumerny { l1: l1 * q, l2: l2 * q };
        }
        return;
    }
    v[i].r = r;
    if let Zadani::L(_) = v[i].zadani {
        let l = delka_z_r(kruh, r, i1, i2);
        v[i].zadani = if r > 0.0 && l > 1e-9 { Zadani::L(l) } else { Zadani::R };
    }
}

/// Mez „malého“ poloměru zakružovacího oblouku pro kontrolu [m] (K = R / 100).
pub const R_MALY: f64 = 500.0;
/// Mez „velkého“ sklonu úseku pro kontrolu (poměr; 0,08 = 80 ‰).
pub const SKLON_VELKY: f64 = 0.08;

/// Upozornění kontroly nivelety (nejsou to chyby – niveleta se uloží).
#[derive(Clone, Debug, PartialEq)]
pub enum Upozorneni {
    /// Oblouky ve vrcholech `a` a `b` na sebe navazují bez mezipřímky (v pořádku).
    Navazuji { a: usize, b: usize },
    /// Malý poloměr ve vrcholu.
    MalyPolomer { vrchol: usize, r: f64 },
    /// Velký sklon úseku mezi vrcholy `od` a `od + 1`.
    VelkySklon { od: usize, sklon: f64 },
}

/// Sklon úseku za vrcholem `k` (poměr; za posledním vrcholem `None`).
pub fn sklon_za(v: &[VrcholNivelety], k: usize) -> Option<f64> {
    (k + 1 < v.len()).then(|| (v[k + 1].z - v[k].z) / (v[k + 1].s - v[k].s))
}

/// Sklon úseku před vrcholem `k` (poměr; před prvním vrcholem `None`).
pub fn sklon_pred(v: &[VrcholNivelety], k: usize) -> Option<f64> {
    (k > 0).then(|| (v[k].z - v[k - 1].z) / (v[k].s - v[k - 1].s))
}

/// Změní sklon úseku před vrcholem `k`: předchozí vrchol drží, vrchol `k` se posune výškově.
pub fn nastav_sklon_pred(v: &mut [VrcholNivelety], k: usize, sklon: f64) {
    if k > 0 && k < v.len() {
        v[k].z = v[k - 1].z + sklon * (v[k].s - v[k - 1].s);
    }
}

/// Změní sklon úseku za vrcholem `k`: vrchol `k` drží, následující se posune výškově.
pub fn nastav_sklon_za(v: &mut [VrcholNivelety], k: usize, sklon: f64) {
    if k + 1 < v.len() {
        v[k + 1].z = v[k].z + sklon * (v[k + 1].s - v[k].s);
    }
}

/// Vloží vrchol ve staničení `s` se sklonem `sklon` od vrcholu `od` (výška = z_od + sklon·Δs).
/// Za posledním vrcholem niveletu prodlouží, před prvním ji prodlouží dopředu.
/// Vrací index nového vrcholu; `None`, když ve staničení `s` už vrchol je.
pub fn vloz_sklonem(v: &mut Vec<VrcholNivelety>, od: usize, s: f64, sklon: f64, r: f64) -> Option<usize> {
    if od >= v.len() || v.iter().any(|x| (x.s - s).abs() < 1e-3) {
        return None;
    }
    let z = v[od].z + sklon * (s - v[od].s);
    vloz_vrchol(v, s, z, r)
}

/// Vloží vrchol (s, z) na místo podle staničení. Nový vnitřní vrchol dostane
/// oblouk `r`, nový krajní je bez oblouku a bývalý krajní oblouk dostane.
/// `None`, když ve staničení `s` už vrchol je.
pub fn vloz_vrchol(v: &mut Vec<VrcholNivelety>, s: f64, z: f64, r: f64) -> Option<usize> {
    if v.iter().any(|x| (x.s - s).abs() < 1e-3) {
        return None;
    }
    let i = v.iter().position(|x| x.s > s).unwrap_or(v.len());
    v.insert(i, VrcholNivelety::new(s, z, 0.0));
    // nový vnitřní vrchol dostane oblouk, nový krajní je bez oblouku a bývalý krajní oblouk dostane
    let n = v.len();
    if i > 0 && i + 1 < n {
        v[i].r = r.max(0.0);
    } else if i == 0 && n > 2 {
        v[1].r = v[1].r.max(r.max(0.0));
    } else if i + 1 == n && n > 2 {
        v[n - 2].r = v[n - 2].r.max(r.max(0.0));
    }
    Some(i)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Niveleta {
    pub id: String,
    pub jmeno: String,
    pub vrcholy: Vec<VrcholNivelety>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChybaNivelety {
    MaloVrcholu,
    /// Vrcholy nejdou po sobě ve staničení (index druhého).
    Poradi(usize),
    /// Oblouky ve vrcholech `a` a `b` se překrývají o `o` metrů (dotyk chyba není).
    Prekryv { a: usize, b: usize, o: f64 },
}

impl ChybaNivelety {
    pub fn popis(&self) -> (&'static str, Vec<String>) {
        match self {
            ChybaNivelety::MaloVrcholu => ("niveleta potřebuje aspoň dva vrcholy", vec![]),
            ChybaNivelety::Poradi(i) => ("vrchol nivelety {} neleží za předchozím ve staničení", vec![(i + 1).to_string()]),
            ChybaNivelety::Prekryv { a, b, o } => (
                "zakružovací oblouky ve vrcholech nivelety {} a {} se překrývají o {} m – zmenšete oblouk",
                vec![(a + 1).to_string(), (b + 1).to_string(), crate::stanice::cislo(*o, 2, true)],
            ),
        }
    }
}

impl Upozorneni {
    pub fn popis(&self) -> (&'static str, Vec<String>) {
        use crate::stanice::cislo;
        match self {
            Upozorneni::Navazuji { a, b } => (
                "oblouky ve vrcholech {} a {} na sebe navazují bez mezipřímky (v pořádku)",
                vec![(a + 1).to_string(), (b + 1).to_string()],
            ),
            Upozorneni::MalyPolomer { vrchol, r } => (
                "vrchol {}: R = {} m (K = {}) je malý",
                vec![(vrchol + 1).to_string(), cislo(*r, 0, true), cislo(r / 100.0, 1, true)],
            ),
            Upozorneni::VelkySklon { od, sklon } => (
                "úsek {}–{}: sklon {} ‰ je velký",
                vec![(od + 1).to_string(), (od + 2).to_string(), cislo(sklon * 1000.0, 1, true)],
            ),
        }
    }
    /// Jen informace (zelená), ne varování.
    pub fn v_poradku(&self) -> bool {
        matches!(self, Upozorneni::Navazuji { .. })
    }
}

/// Úsek nivelety mezi vrcholy (pro pás sklonů).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UsekNivelety {
    pub od: f64,
    pub do_: f64,
    /// Sklon (poměr, 0,01 = 10 ‰).
    pub sklon: f64,
}

/// Zakružovací oblouk ve vrcholu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oblouk {
    pub vrchol: usize,
    /// Tečna T (u nesouměrného delší z T1, T2).
    pub tecna: f64,
    /// Vodorovná délka oblouku před vrcholem (T1) a za ním (T2).
    pub t1: f64,
    pub t2: f64,
    /// Začátek a konec oblouku ve staničení.
    pub od: f64,
    pub do_: f64,
    pub vzepeti: f64,
    /// Vrcholový poloměr R [m] (u nesouměrného ekvivalentní parabola stejné délky).
    pub r: f64,
    /// Vodorovná délka oblouku L = T1 + T2 [m].
    pub delka: f64,
    /// K = L / A [m na 1 %] = R / 100.
    pub k: f64,
    /// Lom sklonu A [%] (kladný).
    pub a: f64,
    /// Vypuklý (vrcholový) oblouk – jinak údolnicový.
    pub vypukly: bool,
    pub typ: TypOblouku,
    /// Sklony před a za vrcholem.
    pub i1: f64,
    pub i2: f64,
}

impl Oblouk {
    /// Výška na oblouku ve staničení `s` (vrchol oblouku `sv`, `zv`).
    fn vyska(&self, sv: f64, zv: f64, s: f64) -> f64 {
        let (i1, i2) = (self.i1, self.i2);
        match self.typ {
            TypOblouku::Parabola => {
                let x = s - self.od;
                let znamenko = if i2 > i1 { 1.0 } else { -1.0 };
                zv - self.t1 * i1 + i1 * x + znamenko * x * x / (2.0 * self.r)
            }
            TypOblouku::Nesoumerny { .. } => {
                let (t1, t2) = (self.t1, self.t2);
                let e = (i2 - i1) * t1 * t2 / (2.0 * (t1 + t2));
                if s <= sv {
                    let x = s - (sv - t1);
                    zv - i1 * t1 + i1 * x + e / (t1 * t1) * x * x
                } else {
                    let d = sv + t2 - s;
                    zv + i2 * t2 - i2 * d + e / (t2 * t2) * d * d
                }
            }
            TypOblouku::Kruh => {
                let sg = if i2 > i1 { 1.0 } else { -1.0 };
                let a1 = i1.atan();
                let (ps, pz) = (sv - self.t1, zv - self.t1 * i1);
                let (cs, cz) = (ps - sg * self.r * a1.sin(), pz + sg * self.r * a1.cos());
                cz - sg * (self.r * self.r - (s - cs) * (s - cs)).max(0.0).sqrt()
            }
        }
    }

    /// Největší zakřivení (koeficient a z y = a·x²) – pro hustotu bodů kresby.
    fn krivost(&self) -> f64 {
        match self.typ {
            TypOblouku::Nesoumerny { .. } => {
                let e = ((self.i2 - self.i1) * self.t1 * self.t2 / (2.0 * (self.t1 + self.t2))).abs();
                (e / (self.t1 * self.t1)).max(e / (self.t2 * self.t2))
            }
            _ => 1.0 / (2.0 * self.r),
        }
    }
}

fn f(v: f64) -> String {
    let t = format!("{v:.6}");
    let t = t.trim_end_matches('0').trim_end_matches('.');
    if t.is_empty() || t == "-" { "0".into() } else { t.into() }
}

impl Niveleta {
    pub fn nova(id: &str, jmeno: &str, vrcholy: Vec<VrcholNivelety>) -> Self {
        Self { id: id.into(), jmeno: jmeno.into(), vrcholy }
    }

    /// Sklony úseků mezi vrcholy.
    pub fn sklony(&self) -> Vec<f64> {
        self.vrcholy.windows(2).map(|w| (w[1].z - w[0].z) / (w[1].s - w[0].s)).collect()
    }

    pub fn useky(&self) -> Vec<UsekNivelety> {
        self.vrcholy.windows(2).zip(self.sklony()).map(|(w, i)| UsekNivelety { od: w[0].s, do_: w[1].s, sklon: i }).collect()
    }

    /// Zakružovací oblouky (jen ve vnitřních vrcholech s R > 0 a lomem sklonu).
    pub fn oblouky(&self) -> Vec<Oblouk> {
        let i = self.sklony();
        let mut out = Vec::new();
        for k in 1..self.vrcholy.len().saturating_sub(1) {
            let v = self.vrcholy[k];
            let (i1, i2) = (i[k - 1], i[k]);
            let di = (i2 - i1).abs();
            let r = self.polomer(k);
            if r <= 0.0 || di < 1e-9 {
                continue;
            }
            let (t1, t2) = match v.typ {
                TypOblouku::Parabola => (r * di / 2.0, r * di / 2.0),
                TypOblouku::Nesoumerny { l1, l2 } => {
                    if l1 <= 0.0 || l2 <= 0.0 {
                        continue;
                    }
                    (l1, l2)
                }
                TypOblouku::Kruh => {
                    let (a1, a2) = (i1.atan(), i2.atan());
                    let t = r * ((a2 - a1).abs() / 2.0).tan();
                    (t * a1.cos(), t * a2.cos())
                }
            };
            let mut o = Oblouk {
                vrchol: k,
                tecna: t1.max(t2),
                t1,
                t2,
                od: v.s - t1,
                do_: v.s + t2,
                vzepeti: 0.0,
                r,
                delka: t1 + t2,
                k: r / 100.0,
                a: di * 100.0,
                vypukly: i2 < i1,
                typ: v.typ,
                i1,
                i2,
            };
            o.vzepeti = (o.vyska(v.s, v.z, v.s) - v.z).abs();
            out.push(o);
        }
        out
    }

    /// Platný poloměr oblouku ve vrcholu `k` (u zadání délkou dopočtený ze sklonů;
    /// u nesouměrného ekvivalentní parabola stejné délky).
    pub fn polomer(&self, k: usize) -> f64 {
        let v = self.vrcholy[k];
        let Some((i1, i2)) = sklony_u(&self.vrcholy, k) else {
            return if let Zadani::L(_) = v.zadani { 0.0 } else { v.r };
        };
        match (v.typ, v.zadani) {
            (TypOblouku::Nesoumerny { l1, l2 }, _) => r_z_delky(false, l1 + l2, i1, i2),
            (typ, Zadani::L(l)) => r_z_delky(typ == TypOblouku::Kruh, l, i1, i2),
            _ => v.r,
        }
    }

    /// Upozornění kontroly: navazující oblouky, malé R, velké sklony.
    pub fn upozorneni(&self) -> Vec<Upozorneni> {
        let mut out = Vec::new();
        let ob = self.oblouky();
        let najdi = |k: usize| ob.iter().find(|o| o.vrchol == k);
        for k in 0..self.vrcholy.len().saturating_sub(1) {
            if let (Some(a), Some(b)) = (najdi(k), najdi(k + 1)) {
                let mezera = self.vrcholy[k + 1].s - self.vrcholy[k].s - a.t2 - b.t1;
                if mezera.abs() < 0.01 {
                    out.push(Upozorneni::Navazuji { a: k, b: k + 1 });
                }
            }
        }
        for o in &ob {
            if !matches!(o.typ, TypOblouku::Nesoumerny { .. }) && o.r < R_MALY {
                out.push(Upozorneni::MalyPolomer { vrchol: o.vrchol, r: o.r });
            }
        }
        for (k, i) in self.sklony().into_iter().enumerate() {
            if i.abs() > SKLON_VELKY {
                out.push(Upozorneni::VelkySklon { od: k, sklon: i });
            }
        }
        out
    }

    /// Dopočte poloměry oblouků zadaných délkou (pole `r` pak odpovídá sklonům).
    pub fn dopocti(&mut self) {
        for k in 0..self.vrcholy.len() {
            if let Zadani::L(_) = self.vrcholy[k].zadani {
                self.vrcholy[k].r = self.polomer(k);
            }
        }
    }

    pub fn kontrola(&self) -> Result<(), ChybaNivelety> {
        if self.vrcholy.len() < 2 {
            return Err(ChybaNivelety::MaloVrcholu);
        }
        for k in 1..self.vrcholy.len() {
            if self.vrcholy[k].s <= self.vrcholy[k - 1].s + 1e-6 {
                return Err(ChybaNivelety::Poradi(k));
            }
        }
        let ob = self.oblouky();
        let za = |k: usize| ob.iter().find(|o| o.vrchol == k).map_or(0.0, |o| o.t2);
        let pred = |k: usize| ob.iter().find(|o| o.vrchol == k).map_or(0.0, |o| o.t1);
        for k in 0..self.vrcholy.len() - 1 {
            // dotyk (oblouky bez mezipřímky) je v pořádku, chyba je jen skutečný překryv
            let o = za(k) + pred(k + 1) - (self.vrcholy[k + 1].s - self.vrcholy[k].s);
            if o > 1e-3 {
                return Err(ChybaNivelety::Prekryv { a: k, b: k + 1, o });
            }
        }
        Ok(())
    }

    /// Výška nivelety ve staničení (mimo rozsah `None`).
    pub fn vyska(&self, s: f64) -> Option<f64> {
        let v = &self.vrcholy;
        if v.len() < 2 || s < v[0].s - 1e-9 || s > v[v.len() - 1].s + 1e-9 {
            return None;
        }
        let i = self.sklony();
        for o in self.oblouky() {
            if s >= o.od && s <= o.do_ {
                let p = v[o.vrchol];
                return Some(o.vyska(p.s, p.z, s));
            }
        }
        let k = v.windows(2).position(|w| s <= w[1].s + 1e-9).unwrap_or(v.len() - 2);
        Some(v[k].z + i[k] * (s - v[k].s))
    }

    /// Body lomené čáry nivelety (oblouky po kouscích – odchylka do 1 mm).
    pub fn body(&self) -> Vec<[f64; 2]> {
        let mut st: Vec<f64> = self.vrcholy.iter().map(|v| v.s).collect();
        for o in self.oblouky() {
            // odchylka tětivy paraboly y = a·x²: a·krok² / 4 ≤ 0,001
            let krok = (0.004 / o.krivost().max(1e-12)).sqrt().max(0.2);
            let n = ((o.do_ - o.od) / krok).ceil().max(2.0) as usize;
            st.extend((0..=n).map(|k| o.od + (o.do_ - o.od) * k as f64 / n as f64));
        }
        st.sort_by(|a, b| a.total_cmp(b));
        st.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        // vrchol uvnitř oblouku na niveletě neleží – vynechat
        let ob = self.oblouky();
        st.retain(|s| !ob.iter().any(|o| self.vrcholy[o.vrchol].s == *s));
        st.into_iter().filter_map(|s| self.vyska(s).map(|z| [s, z])).collect()
    }

    /// Řádky pro uložení k trase: `niv=id;jméno`,
    /// `nv=id;s;z;R[;zadání[;typ]]` – zadání `K` / `L<délka>` / prázdné (R),
    /// typ `C` (kružnice) / `N<L1>/<L2>` (nesouměrný) / bez sloupce (parabola).
    /// R je vždy platný poloměr (u nesouměrného ekvivalentní parabola) – starší
    /// verze další sloupce nečtou a kreslí parabolu.
    pub fn zapis(&self) -> Vec<String> {
        let mut r = vec![format!("niv={};{}", self.id, self.jmeno.chars().take(150).collect::<String>())];
        for (k, v) in self.vrcholy.iter().enumerate() {
            let zad = match v.zadani {
                Zadani::R => String::new(),
                Zadani::K => "K".into(),
                Zadani::L(l) => format!("L{}", f(l)),
            };
            let typ = match v.typ {
                TypOblouku::Parabola => String::new(),
                TypOblouku::Kruh => "C".into(),
                TypOblouku::Nesoumerny { l1, l2 } => format!("N{}/{}", f(l1), f(l2)),
            };
            let navic = match (zad.is_empty(), typ.is_empty()) {
                (true, true) => String::new(),
                (_, true) => format!(";{zad}"),
                _ => format!(";{zad};{typ}"),
            };
            r.push(format!("nv={};{};{};{}{}", self.id, f(v.s), f(v.z), f(self.polomer(k)), navic));
        }
        r
    }

    /// Nivelety z řádků (v pořadí, jak jsou uložené).
    pub fn cti_vse(radky: &[&str]) -> Vec<Niveleta> {
        let mut out: Vec<Niveleta> = Vec::new();
        for r in radky {
            if let Some(h) = r.strip_prefix("niv=") {
                let (id, jmeno) = h.split_once(';').unwrap_or((h, h));
                out.push(Niveleta::nova(id, jmeno, Vec::new()));
            } else if let Some(h) = r.strip_prefix("nv=") {
                let c: Vec<&str> = h.split(';').collect();
                let cislo = |i: usize| c.get(i).and_then(|x| x.trim().parse::<f64>().ok());
                if let (Some(id), Some(s), Some(z)) = (c.first(), cislo(1), cislo(2)) {
                    if let Some(n) = out.iter_mut().find(|n| n.id == *id) {
                        let mut v = VrcholNivelety::new(s, z, cislo(3).unwrap_or(0.0));
                        match c.get(4).map(|x| x.trim()) {
                            Some("K") => v.zadani = Zadani::K,
                            Some(x) if x.starts_with('L') => {
                                if let Some(l) = x[1..].parse::<f64>().ok().filter(|l| *l > 0.0) {
                                    v.zadani = Zadani::L(l);
                                }
                            }
                            _ => {}
                        }
                        match c.get(5).map(|x| x.trim()) {
                            Some("C") => v.typ = TypOblouku::Kruh,
                            Some(x) if x.starts_with('N') => {
                                if let Some((a, b)) = x[1..].split_once('/') {
                                    if let (Ok(l1), Ok(l2)) = (a.parse::<f64>(), b.parse::<f64>()) {
                                        if l1 > 0.0 && l2 > 0.0 {
                                            v.typ = TypOblouku::Nesoumerny { l1, l2 };
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                        n.vrcholy.push(v);
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod testy {
    use super::*;

    fn n() -> Niveleta {
        // stoupání 20 ‰, pak klesání −10 ‰, vypuklý oblouk R = 2000
        Niveleta::nova("N1", "Niveleta", vec![
            VrcholNivelety::new(0.0, 100.0, 0.0),
            VrcholNivelety::new(100.0, 102.0, 2000.0),
            VrcholNivelety::new(200.0, 101.0, 0.0),
        ])
    }

    fn blizko(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn sklony_a_oblouk() {
        let n = n();
        let i = n.sklony();
        assert!(blizko(i[0], 0.02) && blizko(i[1], -0.01));
        let o = n.oblouky()[0];
        assert!(blizko(o.tecna, 30.0)); // 2000 · 0,03 / 2
        assert!(blizko(o.vzepeti, 0.225)); // 30² / 4000
        assert!(n.kontrola().is_ok());
    }

    #[test]
    fn vysky() {
        let n = n();
        assert!(blizko(n.vyska(50.0).unwrap(), 101.0));
        // ve vrcholu je niveleta o vzepětí níž (vypuklý oblouk)
        assert!(blizko(n.vyska(100.0).unwrap(), 102.0 - 0.225));
        // začátek a konec oblouku navazují na tečny
        assert!(blizko(n.vyska(70.0).unwrap(), 101.4));
        assert!(blizko(n.vyska(130.0).unwrap(), 101.7));
        assert!(blizko(n.vyska(200.0).unwrap(), 101.0));
        assert!(n.vyska(-1.0).is_none());
    }

    #[test]
    fn udolnicovy_oblouk() {
        let n = Niveleta::nova("N", "", vec![
            VrcholNivelety::new(0.0, 100.0, 0.0),
            VrcholNivelety::new(100.0, 98.0, 1000.0),
            VrcholNivelety::new(200.0, 100.0, 0.0),
        ]);
        let o = n.oblouky()[0];
        assert!(blizko(n.vyska(100.0).unwrap(), 98.0 + o.vzepeti));
    }

    #[test]
    fn prekryv_a_poradi() {
        let mut n = n();
        n.vrcholy[1].r = 10_000.0; // T = 150 > 100
        match n.kontrola() {
            Err(ChybaNivelety::Prekryv { a: 0, b: 1, o }) => assert!((o - 50.0).abs() < 1e-6, "překryv o 50 m"),
            x => panic!("{x:?}"),
        }
        let n2 = Niveleta::nova("N", "", vec![VrcholNivelety::new(10.0, 1.0, 0.0), VrcholNivelety::new(5.0, 1.0, 0.0)]);
        assert_eq!(n2.kontrola(), Err(ChybaNivelety::Poradi(1)));
    }

    #[test]
    fn body_a_zapis() {
        let n = n();
        let b = n.body();
        assert!(b.len() > 5);
        assert!(blizko(b[0][0], 0.0) && blizko(b.last().unwrap()[0], 200.0));
        assert!(!b.iter().any(|p| blizko(p[0], 100.0) && blizko(p[1], 102.0)), "vrchol uvnitř oblouku není na niveletě");
        let r = n.zapis();
        let rr: Vec<&str> = r.iter().map(String::as_str).collect();
        assert_eq!(Niveleta::cti_vse(&rr), vec![n]);
    }

    #[test]
    fn oblouk_l_a_k() {
        let o = n().oblouky()[0];
        // R 2000, lom 3 % → L = 60, K = 20
        assert!(blizko(o.delka, 60.0) && blizko(o.k, 20.0) && blizko(o.a, 3.0) && o.vypukly);
        let mut l = n();
        l.vrcholy[1] = VrcholNivelety::zadany(100.0, 102.0, "L", 60.0);
        assert!(blizko(l.oblouky()[0].r, 2000.0));
        assert!(blizko(l.vyska(100.0).unwrap(), n().vyska(100.0).unwrap()));
        let mut k = n();
        k.vrcholy[1] = VrcholNivelety::zadany(100.0, 102.0, "K", 20.0);
        assert_eq!(k.oblouky(), n().oblouky());
        assert_eq!(k.vrcholy[1].zadani, Zadani::K);
    }

    #[test]
    fn delka_zustava_pri_zmene_sklonu() {
        let mut n = n();
        n.vrcholy[1] = VrcholNivelety::zadany(100.0, 102.0, "L", 60.0);
        n.vrcholy[2].z = 100.0; // lom 2 % + 2 % = 4 %
        let o = n.oblouky()[0];
        assert!(blizko(o.delka, 60.0) && blizko(o.r, 1500.0));
        n.dopocti();
        assert!(blizko(n.vrcholy[1].r, 1500.0));
        // zápis: R je platný poloměr, pátý sloupec drží zadání
        let r = n.zapis();
        assert_eq!(r[2], "nv=N1;100;102;1500;L60");
        let rr: Vec<&str> = r.iter().map(String::as_str).collect();
        assert_eq!(Niveleta::cti_vse(&rr), vec![n.clone()]);
        // starší zápis bez pátého sloupce = zadání poloměrem
        let stary = Niveleta::cti_vse(&["niv=N1;x", "nv=N1;0;100;0", "nv=N1;100;102;1500", "nv=N1;200;100;0"]);
        assert_eq!(stary[0].vrcholy[1].zadani, Zadani::R);
        // úchop změní poloměr – délka se přepočte, zadání zůstane délkou
        nastav_polomer(&mut n.vrcholy, 1, 1000.0);
        assert_eq!(n.vrcholy[1].zadani, Zadani::L(40.0));
    }

    #[test]
    fn sklony_a_vrchol_sklonem() {
        let mut v = n().vrcholy;
        assert!(blizko(sklon_za(&v, 0).unwrap(), 0.02) && blizko(sklon_pred(&v, 2).unwrap(), -0.01));
        assert!(sklon_pred(&v, 0).is_none() && sklon_za(&v, 2).is_none());
        nastav_sklon_za(&mut v, 1, -0.03); // vrchol 2 drží, vrchol 3 klesne
        assert!(blizko(v[2].z, 99.0) && blizko(v[1].z, 102.0));
        nastav_sklon_pred(&mut v, 1, 0.01); // vrchol 1 drží, vrchol 2 klesne
        assert!(blizko(v[1].z, 101.0) && blizko(v[0].z, 100.0));
        // vložení uvnitř: výška ze sklonu od vrcholu 1, dostane oblouk
        let i = vloz_sklonem(&mut v, 0, 50.0, 0.04, 800.0).unwrap();
        assert_eq!(i, 1);
        assert!(blizko(v[1].z, 102.0) && blizko(v[1].r, 800.0));
        // prodloužení za konec: nový konec bez oblouku, bývalý konec oblouk dostane
        let i = vloz_sklonem(&mut v, 3, 260.0, 0.005, 800.0).unwrap();
        assert_eq!(i, 4);
        assert!(blizko(v[4].z, v[3].z + 0.3) && blizko(v[4].r, 0.0) && blizko(v[3].r, 800.0));
        // ve stejném staničení vrchol nevznikne
        assert!(vloz_sklonem(&mut v, 0, 50.0, 0.0, 0.0).is_none());
    }

    #[test]
    fn nesoumerny_oblouk() {
        let mut n = n();
        n.vrcholy[1].typ = TypOblouku::Nesoumerny { l1: 20.0, l2: 40.0 };
        let o = n.oblouky()[0];
        assert!(blizko(o.od, 80.0) && blizko(o.do_, 140.0) && blizko(o.delka, 60.0));
        // e = Δi·L1·L2 / (2(L1+L2)) = −0,03·800/120 = −0,2
        assert!(blizko(o.vzepeti, 0.2));
        assert!(blizko(n.vyska(100.0).unwrap(), 101.8));
        // navazuje na tečny a je hladký v začátku i konci
        assert!(blizko(n.vyska(80.0).unwrap(), 101.6));
        assert!(blizko(n.vyska(140.0).unwrap(), 101.6));
        let d = 1e-4;
        assert!(((n.vyska(80.0 + d).unwrap() - n.vyska(80.0).unwrap()) / d - 0.02).abs() < 1e-3);
        assert!(((n.vyska(140.0).unwrap() - n.vyska(140.0 - d).unwrap()) / d + 0.01).abs() < 1e-3);
        // spojitost ve vrcholu
        assert!((n.vyska(100.0 - d).unwrap() - n.vyska(100.0 + d).unwrap()).abs() < 1e-5);
        // zápis a čtení
        let r = n.zapis();
        assert_eq!(r[2], "nv=N1;100;102;2000;;N20/40");
        let rr: Vec<&str> = r.iter().map(String::as_str).collect();
        assert_eq!(Niveleta::cti_vse(&rr), vec![n.clone()]);
        // zadání délkou rozdělí L v dosavadním poměru
        let v = n.vrcholy[1].se_zadanim("L", 90.0);
        assert_eq!(v.typ, TypOblouku::Nesoumerny { l1: 30.0, l2: 60.0 });
    }

    #[test]
    fn kruhovy_oblouk() {
        let mut n = n();
        n.vrcholy[1].typ = TypOblouku::Kruh;
        let o = n.oblouky()[0];
        // u malých sklonů téměř shodný s parabolou (T ≈ 30, y ≈ 0,225)
        assert!((o.t1 - 30.0).abs() < 0.05 && (o.t2 - 30.0).abs() < 0.05);
        assert!((o.vzepeti - 0.225).abs() < 0.002);
        // body oblouku leží na kružnici R = 2000 a navazují na tečny
        let (sv, zv) = (100.0, 102.0);
        assert!((n.vyska(o.od).unwrap() - (zv - o.t1 * 0.02)).abs() < 1e-6);
        assert!((n.vyska(o.do_).unwrap() - (zv - o.t2 * 0.01)).abs() < 1e-6);
        let _ = sv;
        let r = n.zapis();
        let rr: Vec<&str> = r.iter().map(String::as_str).collect();
        assert_eq!(Niveleta::cti_vse(&rr)[0].vrcholy[1].typ, TypOblouku::Kruh);
        // zadání délkou u kružnice vrátí stejný poloměr
        let l = o.delka;
        n.vrcholy[1] = n.vrcholy[1].se_zadanim("L", l);
        assert!((n.polomer(1) - 2000.0).abs() < 1e-6);
    }

    #[test]
    fn dotyk_oblouku_neni_chyba() {
        // dva oblouky, které se přesně dotknou (T2 + T1 = vzdálenost vrcholů)
        let mut n = Niveleta::nova("N", "", vec![
            VrcholNivelety::new(0.0, 100.0, 0.0),
            VrcholNivelety::new(100.0, 102.0, 0.0),
            VrcholNivelety::new(200.0, 101.0, 0.0),
            VrcholNivelety::new(300.0, 103.0, 0.0),
        ]);
        // lomy 3 % a 3 %: T = R·0,03/2 → R = 100/0,03 dá T = 50 + 50 = 100
        n.vrcholy[1].r = 100.0 / 0.03;
        n.vrcholy[2].r = 100.0 / 0.03;
        assert!(n.kontrola().is_ok());
        assert!(n.upozorneni().contains(&Upozorneni::Navazuji { a: 1, b: 2 }));
        n.vrcholy[2].r = 110.0 / 0.03;
        assert!(matches!(n.kontrola(), Err(ChybaNivelety::Prekryv { a: 1, b: 2, .. })));
    }

    #[test]
    fn upozorneni_maly_polomer_a_sklon() {
        let mut n = n();
        n.vrcholy[1].r = 200.0;
        n.vrcholy[2].z = 80.0; // −220 ‰
        let u = n.upozorneni();
        assert!(u.contains(&Upozorneni::MalyPolomer { vrchol: 1, r: 200.0 }));
        assert!(u.iter().any(|x| matches!(x, Upozorneni::VelkySklon { od: 1, .. })));
    }
}
