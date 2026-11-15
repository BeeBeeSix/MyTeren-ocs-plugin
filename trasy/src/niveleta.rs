//! Niveleta (návrhový profil): vrcholy (PVI) se zakružovacími oblouky.
//!
//! Zakružovací oblouk je kvadratická parabola zadaná vrcholovým poloměrem R
//! (česká praxe): tečna T = R·|i₂ − i₁| / 2, vzepětí y = T² / (2R).

/// Vrchol nivelety.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VrcholNivelety {
    /// Staničení [m].
    pub s: f64,
    /// Výška [m n. m.].
    pub z: f64,
    /// Poloměr zakružovacího oblouku [m]; 0 = bez oblouku. U krajních vrcholů se nepoužije.
    pub r: f64,
}

impl VrcholNivelety {
    pub fn new(s: f64, z: f64, r: f64) -> Self {
        Self { s, z, r }
    }
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
            if v.r <= 0.0 || di < 1e-9 {
                continue;
            }
            let t = v.r * di / 2.0;
            out.push(Oblouk { vrchol: k, tecna: t, od: v.s - t, do_: v.s + t, vzepeti: t * t / (2.0 * v.r) });
        }
        out
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
                return Some(p.z - o.tecna * i1 + i1 * x + znamenko * x * x / (2.0 * p.r));
            }
        }
        let k = v.windows(2).position(|w| s <= w[1].s + 1e-9).unwrap_or(v.len() - 2);
        Some(v[k].z + i[k] * (s - v[k].s))
    }

    /// Body lomené čáry nivelety (oblouky po kouscích – odchylka do 1 mm).
    pub fn body(&self) -> Vec<[f64; 2]> {
        let mut st: Vec<f64> = self.vrcholy.iter().map(|v| v.s).collect();
        for o in self.oblouky() {
            let r = self.vrcholy[o.vrchol].r;
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

    /// Řádky pro uložení k trase: `niv=id;jméno`, `nv=id;s;z;R`.
    pub fn zapis(&self) -> Vec<String> {
        let mut r = vec![format!("niv={};{}", self.id, self.jmeno.chars().take(150).collect::<String>())];
        for v in &self.vrcholy {
            r.push(format!("nv={};{};{};{}", self.id, f(v.s), f(v.z), f(v.r)));
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
                        n.vrcholy.push(VrcholNivelety::new(s, z, cislo(3).unwrap_or(0.0)));
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
}
