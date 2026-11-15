//! Terénní profil: výšky povrchu (TIN) podél trasy s lomy v průsečících
//! s hranami trojúhelníků. Mimo povrch má profil mezeru.

use crate::geometrie::{Geometrie, Prvek};

/// Trojúhelník povrchu (x, y, z) ve světových souřadnicích.
pub type Troj = [[f64; 3]; 3];

/// Mřížkový index trojúhelníků pro rychlé hledání podle polohy.
pub struct Povrch {
    troj: Vec<Troj>,
    x0: f64,
    y0: f64,
    bunka: f64,
    nx: usize,
    ny: usize,
    mrizka: Vec<Vec<u32>>,
}

const TOL: f64 = 1e-9;

impl Povrch {
    pub fn new(troj: Vec<Troj>) -> Self {
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for t in &troj {
            for p in t {
                x0 = x0.min(p[0]);
                y0 = y0.min(p[1]);
                x1 = x1.max(p[0]);
                y1 = y1.max(p[1]);
            }
        }
        if troj.is_empty() {
            return Self { troj, x0: 0.0, y0: 0.0, bunka: 1.0, nx: 0, ny: 0, mrizka: Vec::new() };
        }
        // zhruba dva trojúhelníky na buňku
        let plocha = ((x1 - x0) * (y1 - y0)).max(1e-6);
        let bunka = (plocha / troj.len() as f64 * 2.0).sqrt().max(0.01);
        let nx = (((x1 - x0) / bunka) as usize + 1).min(4096);
        let ny = (((y1 - y0) / bunka) as usize + 1).min(4096);
        let bunka = ((x1 - x0) / nx as f64).max((y1 - y0) / ny as f64).max(0.01) * 1.000001;
        let mut mrizka = vec![Vec::new(); nx * ny];
        for (k, t) in troj.iter().enumerate() {
            let (ax, ay, bx, by) = obdelnik(t);
            let (i0, j0) = ((((ax - x0) / bunka) as usize).min(nx - 1), (((ay - y0) / bunka) as usize).min(ny - 1));
            let (i1, j1) = ((((bx - x0) / bunka) as usize).min(nx - 1), (((by - y0) / bunka) as usize).min(ny - 1));
            for j in j0..=j1 {
                for i in i0..=i1 {
                    mrizka[j * nx + i].push(k as u32);
                }
            }
        }
        Self { troj, x0, y0, bunka, nx, ny, mrizka }
    }

    pub fn je_prazdny(&self) -> bool {
        self.troj.is_empty()
    }

    pub fn pocet(&self) -> usize {
        self.troj.len()
    }

    /// Indexy trojúhelníků, které mohou zasahovat do obdélníku.
    fn kandidati(&self, ax: f64, ay: f64, bx: f64, by: f64) -> Vec<u32> {
        if self.nx == 0 {
            return Vec::new();
        }
        let bunka = |x: f64, x0: f64, n: usize| (((x - x0) / self.bunka).floor().max(0.0) as usize).min(n - 1);
        let (i0, i1) = (bunka(ax.min(bx), self.x0, self.nx), bunka(ax.max(bx), self.x0, self.nx));
        let (j0, j1) = (bunka(ay.min(by), self.y0, self.ny), bunka(ay.max(by), self.y0, self.ny));
        let mut out = Vec::new();
        for j in j0..=j1 {
            for i in i0..=i1 {
                out.extend_from_slice(&self.mrizka[j * self.nx + i]);
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Výška povrchu v bodě (mimo povrch `None`).
    pub fn vyska(&self, x: f64, y: f64) -> Option<f64> {
        for k in self.kandidati(x, y, x, y) {
            if let Some(z) = vyska_v_troj(&self.troj[k as usize], x, y) {
                return Some(z);
            }
        }
        None
    }
}

fn obdelnik(t: &Troj) -> (f64, f64, f64, f64) {
    let xs = [t[0][0], t[1][0], t[2][0]];
    let ys = [t[0][1], t[1][1], t[2][1]];
    (
        xs.iter().cloned().fold(f64::MAX, f64::min),
        ys.iter().cloned().fold(f64::MAX, f64::min),
        xs.iter().cloned().fold(f64::MIN, f64::max),
        ys.iter().cloned().fold(f64::MIN, f64::max),
    )
}

/// Výška v trojúhelníku (barycentrické souřadnice), mimo něj `None`.
fn vyska_v_troj(t: &Troj, x: f64, y: f64) -> Option<f64> {
    let [a, b, c] = t;
    let det = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
    if det.abs() < 1e-12 {
        return None;
    }
    let l1 = ((b[1] - c[1]) * (x - c[0]) + (c[0] - b[0]) * (y - c[1])) / det;
    let l2 = ((c[1] - a[1]) * (x - c[0]) + (a[0] - c[0]) * (y - c[1])) / det;
    let l3 = 1.0 - l1 - l2;
    let tol = -1e-9;
    (l1 >= tol && l2 >= tol && l3 >= tol).then(|| l1 * a[2] + l2 * b[2] + l3 * c[2])
}

/// Parametr průsečíku úsečky AB s úsečkou CD na AB (0..1), jinak `None`.
fn prusecik(a: [f64; 2], b: [f64; 2], c: [f64; 3], d: [f64; 3]) -> Option<f64> {
    let r = [b[0] - a[0], b[1] - a[1]];
    let s = [d[0] - c[0], d[1] - c[1]];
    let den = r[0] * s[1] - r[1] * s[0];
    if den.abs() < 1e-12 {
        return None;
    }
    let qp = [c[0] - a[0], c[1] - a[1]];
    let t = (qp[0] * s[1] - qp[1] * s[0]) / den;
    let u = (qp[0] * r[1] - qp[1] * r[0]) / den;
    (t > TOL && t < 1.0 - TOL && (-TOL..=1.0 + TOL).contains(&u)).then_some(t)
}

/// Lomená čára osy mezi staničeními `od`..`do_`: body (staničení, x, y).
/// Oblouky se dělí tak, aby se tětiva od oblouku odchýlila nejvýš o 1 cm.
pub fn body_osy(g: &Geometrie, od: f64, do_: f64) -> Vec<(f64, [f64; 2])> {
    let mut out: Vec<(f64, [f64; 2])> = Vec::new();
    let mut pridej = |s: f64, p: [f64; 2]| {
        if out.last().is_none_or(|(s0, _)| s - s0 > 1e-9) {
            out.push((s, p));
        }
    };
    for (s0, p) in &g.prvky {
        let l = p.delka();
        let (a, b) = (s0.max(od), (s0 + l).min(do_));
        if b <= a {
            continue;
        }
        let krok = match *p {
            Prvek::Primka { .. } => b - a,
            Prvek::Oblouk { r, .. } => (8.0 * r * 0.01).sqrt().max(0.05),
        };
        let n = ((b - a) / krok).ceil().max(1.0) as usize;
        for k in 0..=n {
            let s = a + (b - a) * k as f64 / n as f64;
            pridej(s, p.bod(s - s0).0);
        }
    }
    out
}

/// Terénní profil podél trasy: souvislé úseky bodů (staničení, výška).
/// Lomy jsou v průsečících osy s hranami trojúhelníků povrchu.
pub fn profil_terenu(g: &Geometrie, povrch: &Povrch, od: f64, do_: f64) -> Vec<Vec<[f64; 2]>> {
    let osa = body_osy(g, od.max(g.pocatek), do_.min(g.konec()));
    if osa.len() < 2 || povrch.je_prazdny() {
        return Vec::new();
    }
    // staničení k vyhodnocení: body osy + průsečíky s hranami
    let mut st: Vec<(f64, [f64; 2])> = Vec::new();
    for w in osa.windows(2) {
        let ((sa, a), (sb, b)) = (w[0], w[1]);
        st.push((sa, a));
        for k in povrch.kandidati(a[0], a[1], b[0], b[1]) {
            let t = &povrch.troj[k as usize];
            for (c, d) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                if let Some(u) = prusecik(a, b, c, d) {
                    st.push((sa + u * (sb - sa), [a[0] + u * (b[0] - a[0]), a[1] + u * (b[1] - a[1])]));
                }
            }
        }
    }
    st.push(*osa.last().unwrap());
    st.sort_by(|x, y| x.0.total_cmp(&y.0));
    st.dedup_by(|x, y| (x.0 - y.0).abs() < 1e-6);

    let mut useky: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut akt: Vec<[f64; 2]> = Vec::new();
    for (s, p) in st {
        match povrch.vyska(p[0], p[1]) {
            Some(z) => akt.push([s, z]),
            None => {
                if akt.len() >= 2 {
                    useky.push(std::mem::take(&mut akt));
                } else {
                    akt.clear();
                }
            }
        }
    }
    if akt.len() >= 2 {
        useky.push(akt);
    }
    // body na přímce mezi sousedy jsou zbytečné (oblouk dělený po kouscích)
    for u in &mut useky {
        let mut v: Vec<[f64; 2]> = Vec::with_capacity(u.len());
        for &p in u.iter() {
            if v.len() >= 2 {
                let (a, b) = (v[v.len() - 2], v[v.len() - 1]);
                // lineární předpověď výšky v b z a a p
                let z_lin = a[1] + (p[1] - a[1]) * (b[0] - a[0]) / (p[0] - a[0]);
                if (z_lin - b[1]).abs() < 1e-6 {
                    v.pop();
                }
            }
            v.push(p);
        }
        *u = v;
    }
    useky
}

/// Rozsah výšek profilu (min, max).
pub fn rozsah_vysek(useky: &[Vec<[f64; 2]>]) -> Option<(f64, f64)> {
    let mut it = useky.iter().flatten().map(|p| p[1]);
    let first = it.next()?;
    Some(it.fold((first, first), |(a, b), z| (a.min(z), b.max(z))))
}

#[cfg(test)]
mod testy {
    use super::*;
    use crate::geometrie::{geometrie, Vrchol};

    /// Čtverec 0..100 × 0..100 ze dvou trojúhelníků, rovina z = 0,1·x + 5.
    fn rovina() -> Povrch {
        let z = |x: f64| 0.1 * x + 5.0;
        let a = [0.0, 0.0, z(0.0)];
        let b = [100.0, 0.0, z(100.0)];
        let c = [100.0, 100.0, z(100.0)];
        let d = [0.0, 100.0, z(0.0)];
        Povrch::new(vec![[a, b, c], [a, c, d]])
    }

    #[test]
    fn vyska_v_bode() {
        let p = rovina();
        assert!((p.vyska(50.0, 20.0).unwrap() - 10.0).abs() < 1e-9);
        assert!(p.vyska(150.0, 20.0).is_none());
    }

    #[test]
    fn primka_pres_rovinu() {
        let g = geometrie(&[Vrchol::new(10.0, 50.0, 0.0), Vrchol::new(90.0, 50.0, 0.0)], 0.0).unwrap();
        let u = profil_terenu(&g, &rovina(), 0.0, 1e9);
        assert_eq!(u.len(), 1);
        // rovina = přímka v profilu: zůstanou jen krajní body
        assert_eq!(u[0].len(), 2);
        assert!((u[0][0][1] - 6.0).abs() < 1e-9);
        assert!((u[0][1][1] - 14.0).abs() < 1e-9);
    }

    #[test]
    fn lom_na_hrane() {
        // střecha: hřeben na x = 50 (z = 10), okraje z = 0
        let t = vec![
            [[0.0, 0.0, 0.0], [50.0, 0.0, 10.0], [50.0, 100.0, 10.0]],
            [[0.0, 0.0, 0.0], [50.0, 100.0, 10.0], [0.0, 100.0, 0.0]],
            [[50.0, 0.0, 10.0], [100.0, 0.0, 0.0], [100.0, 100.0, 0.0]],
            [[50.0, 0.0, 10.0], [100.0, 100.0, 0.0], [50.0, 100.0, 10.0]],
        ];
        let p = Povrch::new(t);
        let g = geometrie(&[Vrchol::new(10.0, 30.0, 0.0), Vrchol::new(90.0, 30.0, 0.0)], 100.0).unwrap();
        let u = profil_terenu(&g, &p, 0.0, 1e9);
        assert_eq!(u.len(), 1);
        let hreben = u[0].iter().find(|b| (b[0] - 140.0).abs() < 1e-6).expect("lom na hřebeni");
        assert!((hreben[1] - 10.0).abs() < 1e-9);
        assert_eq!(u[0].len(), 3);
    }

    #[test]
    fn mezera_mimo_povrch() {
        let g = geometrie(&[Vrchol::new(-50.0, 50.0, 0.0), Vrchol::new(150.0, 50.0, 0.0)], 0.0).unwrap();
        let u = profil_terenu(&g, &rovina(), 0.0, 1e9);
        assert_eq!(u.len(), 1);
        assert!((u[0][0][0] - 50.0).abs() < 1e-6, "začátek na okraji povrchu");
        assert!((u[0].last().unwrap()[0] - 150.0).abs() < 1e-6);
        assert_eq!(rozsah_vysek(&u).map(|r| (r.0.round(), r.1.round())), Some((5.0, 15.0)));
    }

    #[test]
    fn oblouk_po_kouscich() {
        let g = geometrie(&[Vrchol::new(10.0, 10.0, 0.0), Vrchol::new(60.0, 10.0, 20.0), Vrchol::new(60.0, 90.0, 0.0)], 0.0).unwrap();
        let osa = body_osy(&g, g.pocatek, g.konec());
        assert!(osa.len() > 5);
        assert!((osa.last().unwrap().0 - g.konec()).abs() < 1e-9);
        let u = profil_terenu(&g, &rovina(), 0.0, 1e9);
        assert_eq!(u.len(), 1);
    }
}
