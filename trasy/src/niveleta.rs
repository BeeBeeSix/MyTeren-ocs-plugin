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
}

impl VrcholNivelety {
    pub fn new(s: f64, z: f64, r: f64) -> Self {
        Self { s, z, r, zadani: Zadani::R }
    }

    /// Vrchol s obloukem zadaným písmenem a hodnotou (R, L nebo K; hodnota ≤ 0 = bez oblouku).
    pub fn zadany(s: f64, z: f64, zadani: &str, hodnota: f64) -> Self {
        let h = hodnota.max(0.0);
        match zadani {
            "L" | "l" if h > 0.0 => Self { s, z, r: 0.0, zadani: Zadani::L(h) },
            "K" | "k" => Self { s, z, r: h * 100.0, zadani: Zadani::K },
            _ => Self::new(s, z, h),
        }
    }
}

/// Rozdíl sklonů ve vnitřním vrcholu `k` (poměr, kladný).
fn lom(v: &[VrcholNivelety], k: usize) -> f64 {
    if k == 0 || k + 1 >= v.len() {
        return 0.0;
    }
    let i1 = (v[k].z - v[k - 1].z) / (v[k].s - v[k - 1].s);
    let i2 = (v[k + 1].z - v[k].z) / (v[k + 1].s - v[k].s);
    let d = (i2 - i1).abs();
    if d.is_finite() { d } else { 0.0 }
}

/// Nastaví vrcholu `i` poloměr tak, aby zadání zůstalo (u délky L se přepočte L = R·lom).
pub fn nastav_polomer(v: &mut [VrcholNivelety], i: usize, r: f64) {
    let r = r.max(0.0);
    let d = lom(v, i);
    v[i].r = r;
    if let Zadani::L(_) = v[i].zadani {
        v[i].zadani = if r > 0.0 && d > 1e-9 { Zadani::L(r * d) } else { Zadani::R };
    }
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
    /// Oblouky ve vrcholech `a` a `b` se překrývají.
    Prekryv { a: usize, b: usize },
}

impl ChybaNivelety {
    pub fn popis(&self) -> (&'static str, Vec<String>) {
        match self {
            ChybaNivelety::MaloVrcholu => ("niveleta potřebuje aspoň dva vrcholy", vec![]),
            ChybaNivelety::Poradi(i) => ("vrchol nivelety {} neleží za předchozím ve staničení", vec![(i + 1).to_string()]),
            ChybaNivelety::Prekryv { a, b } => (
                "zakružovací oblouky ve vrcholech nivelety {} a {} se překrývají – zmenšete poloměr",
                vec![(a + 1).to_string(), (b + 1).to_string()],
            ),
        }
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
    pub tecna: f64,
    /// Začátek a konec oblouku ve staničení.
    pub od: f64,
    pub do_: f64,
    pub vzepeti: f64,
    /// Vrcholový poloměr R [m].
    pub r: f64,
    /// Délka oblouku L = 2T [m].
    pub delka: f64,
    /// K = L / A [m na 1 %] = R / 100.
    pub k: f64,
    /// Lom sklonu A [%] (kladný).
    pub a: f64,
    /// Vypuklý (vrcholový) oblouk – jinak údolnicový.
    pub vypukly: bool,
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
            let di = (i[k] - i[k - 1]).abs();
            let r = self.polomer(k);
            if r <= 0.0 || di < 1e-9 {
                continue;
            }
            let t = r * di / 2.0;
            out.push(Oblouk {
                vrchol: k,
                tecna: t,
                od: v.s - t,
                do_: v.s + t,
                vzepeti: t * t / (2.0 * r),
                r,
                delka: 2.0 * t,
                k: r / 100.0,
                a: di * 100.0,
                vypukly: i[k] < i[k - 1],
            });
        }
        out
    }

    /// Platný poloměr oblouku ve vrcholu `k` (u zadání délkou dopočtený ze sklonů).
    pub fn polomer(&self, k: usize) -> f64 {
        let v = self.vrcholy[k];
        match v.zadani {
            Zadani::L(l) => {
                let d = lom(&self.vrcholy, k);
                if d > 1e-9 { l / d } else { 0.0 }
            }
            _ => v.r,
        }
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
        let tecna = |k: usize| ob.iter().find(|o| o.vrchol == k).map_or(0.0, |o| o.tecna);
        for k in 0..self.vrcholy.len() - 1 {
            if tecna(k) + tecna(k + 1) > self.vrcholy[k + 1].s - self.vrcholy[k].s + 1e-6 {
                return Err(ChybaNivelety::Prekryv { a: k, b: k + 1 });
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
                let (i1, i2) = (i[o.vrchol - 1], i[o.vrchol]);
                let x = s - o.od;
                let znamenko = if i2 > i1 { 1.0 } else { -1.0 };
                return Some(p.z - o.tecna * i1 + i1 * x + znamenko * x * x / (2.0 * o.r));
            }
        }
        let k = v.windows(2).position(|w| s <= w[1].s + 1e-9).unwrap_or(v.len() - 2);
        Some(v[k].z + i[k] * (s - v[k].s))
    }

    /// Body lomené čáry nivelety (oblouky po kouscích – odchylka do 1 mm).
    pub fn body(&self) -> Vec<[f64; 2]> {
        let mut st: Vec<f64> = self.vrcholy.iter().map(|v| v.s).collect();
        for o in self.oblouky() {
            let r = o.r;
            // odchylka tětivy paraboly: krok² / (8R) ≤ 0,001
            let krok = (0.008 * r).sqrt().max(0.2);
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

    /// Řádky pro uložení k trase: `niv=id;jméno`, `nv=id;s;z;R[;K | ;L<délka>]`
    /// (R je vždy platný poloměr – starší verze pátý sloupec nečtou).
    pub fn zapis(&self) -> Vec<String> {
        let mut r = vec![format!("niv={};{}", self.id, self.jmeno.chars().take(150).collect::<String>())];
        for (k, v) in self.vrcholy.iter().enumerate() {
            let zad = match v.zadani {
                Zadani::R => String::new(),
                Zadani::K => ";K".into(),
                Zadani::L(l) => format!(";L{}", f(l)),
            };
            r.push(format!("nv={};{};{};{}{}", self.id, f(v.s), f(v.z), f(self.polomer(k)), zad));
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
        assert_eq!(n.kontrola(), Err(ChybaNivelety::Prekryv { a: 0, b: 1 }));
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
}
