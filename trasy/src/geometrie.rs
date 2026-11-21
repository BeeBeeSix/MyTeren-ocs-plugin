//! Směrové řešení trasy: tečnový polygon (vrcholy + poloměry + přechodnice)
//! → přímé, přechodnice (klotoidy) a kruhové oblouky, staničení, hlavní body.
//!
//! Trasa je uložená jen jako vrcholy tečnového polygonu s poloměrem v každém
//! vnitřním vrcholu (0 = bez oblouku, lom) a délkami přechodnic před a za
//! obloukem (0 = bez přechodnice). Oblouky jsou tak vždy tečné na sousední
//! přímé a změna vrcholu nebo poloměru přirozeně přepočte všechno.

use std::f64::consts::PI;

const EPS: f64 = 1e-9;
/// Úhel odbočení, pod kterým se vrchol bere jako ležící na přímce [rad].
pub const TOL_PRIMKY: f64 = 1e-6;

pub type Bod = [f64; 2];

/// Vrchol tečnového polygonu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vrchol {
    pub x: f64,
    pub y: f64,
    /// Poloměr oblouku ve vrcholu [m]; 0 = bez oblouku. U prvního a posledního
    /// vrcholu se nepoužije.
    pub r: f64,
    /// Délka přechodnice (klotoidy) před obloukem [m]; 0 = bez přechodnice.
    pub l1: f64,
    /// Délka přechodnice za obloukem [m].
    pub l2: f64,
}

impl Vrchol {
    pub fn new(x: f64, y: f64, r: f64) -> Self {
        Self { x, y, r, l1: 0.0, l2: 0.0 }
    }
    pub fn s_prechodnicemi(x: f64, y: f64, r: f64, l1: f64, l2: f64) -> Self {
        Self { x, y, r, l1, l2 }
    }
    /// Má vrchol přechodnice (a oblouk)?
    pub fn ma_prechodnice(&self) -> bool {
        self.r > 0.0 && (self.l1 > 0.0 || self.l2 > 0.0)
    }
    pub fn bod(&self) -> Bod {
        [self.x, self.y]
    }
}

/// Prvek osy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Prvek {
    Primka { a: Bod, b: Bod },
    /// Kruhový oblouk; `uhel_a` = směr ze středu na začátek, `sweep` = středový
    /// úhel se znaménkem (+ proti směru hodin = levotočivý).
    Oblouk { stred: Bod, r: f64, a: Bod, b: Bod, uhel_a: f64, sweep: f64, vrchol: usize },
    /// Přechodnice (klotoida): křivost se mění lineárně z `k0` na `k1`
    /// (se znaménkem, + vlevo) po délce `delka`; `smer` = azimut na začátku.
    Prechodnice { a: Bod, smer: f64, k0: f64, k1: f64, delka: f64, vrchol: usize },
}

/// Bod a azimut na klotoidě ve vzdálenosti `t` od začátku (Gaussova
/// kvadratura po úsecích nejvýš 2 m – přesnost pod tisícinu milimetru).
pub fn bod_klotoidy(a: Bod, smer: f64, k0: f64, k1: f64, delka: f64, t: f64) -> (Bod, f64) {
    const XI: [f64; 5] = [-0.906_179_845_938_664, -0.538_469_310_105_683, 0.0, 0.538_469_310_105_683, 0.906_179_845_938_664];
    const WI: [f64; 5] = [0.236_926_885_056_189, 0.478_628_670_499_366, 0.568_888_888_888_889, 0.478_628_670_499_366, 0.236_926_885_056_189];
    let dk = if delka > EPS { (k1 - k0) / (2.0 * delka) } else { 0.0 };
    let smer_u = |u: f64| smer + k0 * u + dk * u * u;
    let n = (t.abs() / 2.0).ceil().max(1.0) as usize;
    let h = t / n as f64;
    let (mut x, mut y) = (0.0, 0.0);
    for j in 0..n {
        let c = (j as f64 + 0.5) * h;
        for (xi, wi) in XI.iter().zip(WI) {
            let f = smer_u(c + xi * h / 2.0);
            x += wi * f.cos() * h / 2.0;
            y += wi * f.sin() * h / 2.0;
        }
    }
    ([a[0] + x, a[1] + y], smer_u(t))
}

/// Parametry přechodnice délky `l` k oblouku `r`: (τ úhel tečny, odsazení
/// oblouku p, posun k, parametr A). Bez přechodnice samé nuly.
pub fn parametry_prechodnice(l: f64, r: f64) -> (f64, f64, f64, f64) {
    if l <= 0.0 || r <= 0.0 {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let tau = l / (2.0 * r);
    let (q, _) = bod_klotoidy([0.0, 0.0], 0.0, 0.0, 1.0 / r, l, l);
    (tau, q[1] - r * (1.0 - tau.cos()), q[0] - r * tau.sin(), (r * l).sqrt())
}

impl Prvek {
    pub fn delka(&self) -> f64 {
        match *self {
            Prvek::Primka { a, b } => vzd(a, b),
            Prvek::Oblouk { r, sweep, .. } => r * sweep.abs(),
            Prvek::Prechodnice { delka, .. } => delka,
        }
    }
    /// Bod a směr (azimut, radiány proti směru hodin od +x) ve vzdálenosti `t`
    /// od začátku prvku.
    pub fn bod(&self, t: f64) -> (Bod, f64) {
        match *self {
            Prvek::Primka { a, b } => {
                let d = vzd(a, b);
                let u = if d > EPS { t / d } else { 0.0 };
                ([a[0] + u * (b[0] - a[0]), a[1] + u * (b[1] - a[1])], (b[1] - a[1]).atan2(b[0] - a[0]))
            }
            Prvek::Oblouk { stred, r, uhel_a, sweep, .. } => {
                let l = r * sweep.abs();
                let u = if l > EPS { t / l } else { 0.0 };
                let f = uhel_a + sweep * u;
                ([stred[0] + r * f.cos(), stred[1] + r * f.sin()], f + sweep.signum() * PI / 2.0)
            }
            Prvek::Prechodnice { a, smer, k0, k1, delka, .. } => bod_klotoidy(a, smer, k0, k1, delka, t.clamp(0.0, delka)),
        }
    }
    pub fn je_oblouk(&self) -> bool {
        matches!(self, Prvek::Oblouk { .. })
    }
    pub fn je_prechodnice(&self) -> bool {
        matches!(self, Prvek::Prechodnice { .. })
    }
    /// Body po krocích nejvýš `krok` (kreslení přechodnice lomenou čarou).
    pub fn body(&self, krok: f64) -> Vec<Bod> {
        let l = self.delka();
        let n = (l / krok.max(0.01)).ceil().max(1.0) as usize;
        (0..=n).map(|k| self.bod(l * k as f64 / n as f64).0).collect()
    }
}

/// Druh hlavního bodu trasy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DruhBodu {
    /// Začátek úseku (začátek trasy).
    Zu,
    /// Konec úseku (konec trasy).
    Ku,
    /// Tečna – kružnice (začátek oblouku).
    Tk,
    /// Kružnice – tečna (konec oblouku).
    Kt,
    /// Tečna – přechodnice (začátek přechodnice před obloukem).
    Tp,
    /// Přechodnice – kružnice.
    Pk,
    /// Kružnice – přechodnice.
    Kp,
    /// Přechodnice – tečna (konec přechodnice za obloukem).
    Pt,
    /// Vrchol tečnového polygonu.
    V,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HlavniBod {
    pub druh: DruhBodu,
    /// Staničení; u vrcholu (V) staničení jeho průmětu – středu oblouku.
    pub staniceni: f64,
    pub bod: Bod,
    /// Azimut osy v bodě.
    pub smer: f64,
    /// Index vrcholu tečnového polygonu, ke kterému bod patří.
    pub vrchol: usize,
}

/// Údaje oblouku ve vrcholu (pro tabulku vrcholů a úchopy).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UdajeVrcholu {
    /// Úhel odbočení se znaménkem (+ vlevo) [rad].
    pub odbocka: f64,
    /// Délka tečny T [m] (s přechodnicemi tečna před vrcholem T1).
    pub tecna: f64,
    /// Délka tečny za vrcholem T2 [m] (bez přechodnic stejná jako T1).
    pub tecna2: f64,
    /// Délka kruhového oblouku [m] (bez přechodnic).
    pub delka_oblouku: f64,
    /// Délka celé křivky: přechodnice + oblouk + přechodnice [m].
    pub delka_celkem: f64,
    /// Vnější vzdálenost (vrchol – střed oblouku) [m].
    pub vzepeti: f64,
    /// Jednotkový směr osy úhlu dovnitř oblouku.
    pub osa: Bod,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Chyba {
    MaloVrcholu,
    /// Dva po sobě jdoucí vrcholy splývají (index druhého).
    StejneVrcholy(usize),
    /// Trasa se ve vrcholu obrací o 180°.
    Obrat(usize),
    ZapornyPolomer(usize),
    /// Oblouky ve vrcholech `a` a `b` (nebo oblouk a konec trasy) se na
    /// přímé mezi nimi nevejdou.
    Prekryv { a: usize, b: usize },
    /// Přechodnice ve vrcholu jsou delší, než dovolí úhel odbočení.
    DlouhePrechodnice(usize),
}

impl Chyba {
    /// Text pro uživatele (česky; plugin ho překládá klíčem).
    pub fn popis(&self) -> (&'static str, Vec<String>) {
        match self {
            Chyba::MaloVrcholu => ("trasa potřebuje aspoň dva vrcholy", vec![]),
            Chyba::StejneVrcholy(i) => ("vrcholy {} a {} splývají", vec![i.to_string(), (i + 1).to_string()]),
            Chyba::Obrat(i) => ("ve vrcholu {} se trasa obrací zpět", vec![(i + 1).to_string()]),
            Chyba::ZapornyPolomer(i) => ("poloměr ve vrcholu {} je záporný", vec![(i + 1).to_string()]),
            Chyba::Prekryv { a, b } => (
                "oblouky ve vrcholech {} a {} se nevejdou na přímou mezi nimi – zmenšete poloměr nebo vrcholy oddalte",
                vec![(a + 1).to_string(), (b + 1).to_string()],
            ),
            Chyba::DlouhePrechodnice(i) => (
                "přechodnice ve vrcholu {} jsou delší, než dovolí úhel – zkraťte je nebo zvětšete poloměr",
                vec![(i + 1).to_string()],
            ),
        }
    }
}

/// Vypočtená geometrie trasy.
#[derive(Clone, Debug, PartialEq)]
pub struct Geometrie {
    /// Prvky se staničením začátku.
    pub prvky: Vec<(f64, Prvek)>,
    pub pocatek: f64,
    pub hlavni_body: Vec<HlavniBod>,
    /// Údaje oblouku pro každý vrchol (u krajních a bez oblouku nulové).
    pub vrcholy: Vec<UdajeVrcholu>,
}

pub fn vzd(a: Bod, b: Bod) -> f64 {
    (b[0] - a[0]).hypot(b[1] - a[1])
}

fn jednotkovy(a: Bod, b: Bod) -> Bod {
    let d = vzd(a, b);
    [(b[0] - a[0]) / d, (b[1] - a[1]) / d]
}

/// Úhel odbočení se znaménkem z `d1` na `d2` (jednotkové směry).
fn odbocka(d1: Bod, d2: Bod) -> f64 {
    let cross = d1[0] * d2[1] - d1[1] * d2[0];
    let dot = d1[0] * d2[0] + d1[1] * d2[1];
    cross.atan2(dot)
}

/// Délky tečen T1 (před vrcholem) a T2 (za ním) ve vrcholu `i` (0 u krajních
/// a bez oblouku); s přechodnicemi podle odsazení p a posunu k.
pub fn tecny(v: &[Vrchol], i: usize) -> (f64, f64) {
    if i == 0 || i + 1 >= v.len() || v[i].r <= 0.0 {
        return (0.0, 0.0);
    }
    let d1 = jednotkovy(v[i - 1].bod(), v[i].bod());
    let d2 = jednotkovy(v[i].bod(), v[i + 1].bod());
    let w = odbocka(d1, d2).abs();
    // vrchol skoro na přímce (vložený, zaokrouhlený) = bez oblouku
    if w < TOL_PRIMKY {
        return (0.0, 0.0);
    }
    let r = v[i].r;
    if v[i].l1 <= 0.0 && v[i].l2 <= 0.0 {
        let t = r * (w / 2.0).tan();
        return (t, t);
    }
    let (_, p1, k1, _) = parametry_prechodnice(v[i].l1, r);
    let (_, p2, k2, _) = parametry_prechodnice(v[i].l2, r);
    let (s, c) = w.sin_cos();
    ((r + p2 - (r + p1) * c) / s + k1, (r + p1 - (r + p2) * c) / s + k2)
}

/// Zmenší poloměry, které se mezi sousední vrcholy nevejdou, na největší
/// možné (zaokrouhlené dolů na 0,1 m). Vrací indexy upravených vrcholů.
pub fn prizpusob_polomery(v: &mut [Vrchol]) -> Vec<usize> {
    let mut zmenene = Vec::new();
    let n = v.len();
    if n < 3 {
        return zmenene;
    }
    for _ in 0..20 {
        let mut hotovo = true;
        for i in 0..n - 1 {
            let (ta, tb) = (tecny(v, i).1, tecny(v, i + 1).0);
            let d = vzd(v[i].bod(), v[i + 1].bod());
            if ta + tb <= d + 1e-9 || ta + tb <= 0.0 {
                continue;
            }
            hotovo = false;
            let k = d / (ta + tb) * 0.999;
            for j in [i, i + 1] {
                if j > 0 && j + 1 < n && v[j].r > 0.0 {
                    v[j].r = ((v[j].r * k) * 10.0).floor() / 10.0;
                    v[j].l1 = ((v[j].l1 * k) * 100.0).floor() / 100.0;
                    v[j].l2 = ((v[j].l2 * k) * 100.0).floor() / 100.0;
                    if !zmenene.contains(&j) {
                        zmenene.push(j);
                    }
                }
            }
        }
        if hotovo {
            break;
        }
    }
    zmenene.sort_unstable();
    zmenene
}

/// Spočte prvky, staničení a hlavní body trasy.
pub fn geometrie(v: &[Vrchol], pocatek: f64) -> Result<Geometrie, Chyba> {
    let n = v.len();
    if n < 2 {
        return Err(Chyba::MaloVrcholu);
    }
    for i in 1..n {
        if vzd(v[i - 1].bod(), v[i].bod()) < 1e-6 {
            return Err(Chyba::StejneVrcholy(i - 1));
        }
    }
    for (i, x) in v.iter().enumerate() {
        if x.r < 0.0 || x.l1 < 0.0 || x.l2 < 0.0 {
            return Err(Chyba::ZapornyPolomer(i));
        }
    }
    let nula = UdajeVrcholu { odbocka: 0.0, tecna: 0.0, tecna2: 0.0, delka_oblouku: 0.0, delka_celkem: 0.0, vzepeti: 0.0, osa: [0.0, 0.0] };
    let mut udaje = vec![nula; n];
    let mut t = vec![0.0; n];
    let mut t2 = vec![0.0; n];
    for i in 1..n - 1 {
        let d1 = jednotkovy(v[i - 1].bod(), v[i].bod());
        let d2 = jednotkovy(v[i].bod(), v[i + 1].bod());
        let w = odbocka(d1, d2);
        if w.abs() > PI - 1e-6 {
            return Err(Chyba::Obrat(i));
        }
        (t[i], t2[i]) = tecny(v, i);
        let osa = {
            let o = [d2[0] - d1[0], d2[1] - d1[1]];
            let l = o[0].hypot(o[1]);
            if l > EPS { [o[0] / l, o[1] / l] } else { [0.0, 0.0] }
        };
        let r = if t[i] > 0.0 { v[i].r } else { 0.0 };
        let (l1, l2) = if r > 0.0 { (v[i].l1, v[i].l2) } else { (0.0, 0.0) };
        let (tau1, _, _, _) = parametry_prechodnice(l1, r);
        let (tau2, _, _, _) = parametry_prechodnice(l2, r);
        let lk = r * (w.abs() - tau1 - tau2);
        if r > 0.0 && lk < -1e-9 {
            return Err(Chyba::DlouhePrechodnice(i));
        }
        udaje[i] = UdajeVrcholu {
            odbocka: w,
            tecna: t[i],
            tecna2: t2[i],
            delka_oblouku: lk.max(0.0),
            delka_celkem: lk.max(0.0) + l1 + l2,
            vzepeti: if r > 0.0 { r * (1.0 / (w.abs() / 2.0).cos() - 1.0) } else { 0.0 },
            osa,
        };
    }
    for i in 0..n - 1 {
        if t2[i] + t[i + 1] > vzd(v[i].bod(), v[i + 1].bod()) + 1e-6 {
            return Err(Chyba::Prekryv { a: i, b: i + 1 });
        }
    }

    let mut prvky: Vec<(f64, Prvek)> = Vec::new();
    let mut body = Vec::new();
    let mut s = pocatek;
    let mut cur = v[0].bod();
    body.push(HlavniBod {
        druh: DruhBodu::Zu,
        staniceni: s,
        bod: cur,
        smer: (v[1].y - v[0].y).atan2(v[1].x - v[0].x),
        vrchol: 0,
    });
    let pridej = |prvky: &mut Vec<(f64, Prvek)>, s: &mut f64, p: Prvek| {
        let l = p.delka();
        if l > 1e-9 {
            prvky.push((*s, p));
            *s += l;
        }
    };
    for i in 1..n - 1 {
        let d1 = jednotkovy(v[i - 1].bod(), v[i].bod());
        let d2 = jednotkovy(v[i].bod(), v[i + 1].bod());
        let pi = v[i].bod();
        if t[i] <= 0.0 {
            pridej(&mut prvky, &mut s, Prvek::Primka { a: cur, b: pi });
            cur = pi;
            body.push(HlavniBod { druh: DruhBodu::V, staniceni: s, bod: pi, smer: d2[1].atan2(d2[0]), vrchol: i });
            continue;
        }
        let w = udaje[i].odbocka;
        let tk = [pi[0] - t[i] * d1[0], pi[1] - t[i] * d1[1]];
        let kt = [pi[0] + t2[i] * d2[0], pi[1] + t2[i] * d2[1]];
        pridej(&mut prvky, &mut s, Prvek::Primka { a: cur, b: tk });
        let strana = w.signum();
        let r = v[i].r;
        let (l1, l2) = (v[i].l1, v[i].l2);
        let (tau1, _, _, _) = parametry_prechodnice(l1, r);
        let (tau2, _, _, _) = parametry_prechodnice(l2, r);
        let s_tk = s;
        let smer1 = d1[1].atan2(d1[0]);
        body.push(HlavniBod { druh: if l1 > 0.0 { DruhBodu::Tp } else { DruhBodu::Tk }, staniceni: s, bod: tk, smer: smer1, vrchol: i });
        // přechodnice před obloukem
        let mut a = tk;
        let mut smer = smer1;
        if l1 > 0.0 {
            let p = Prvek::Prechodnice { a, smer, k0: 0.0, k1: strana / r, delka: l1, vrchol: i };
            pridej(&mut prvky, &mut s, p);
            (a, smer) = p.bod(l1);
            body.push(HlavniBod { druh: DruhBodu::Pk, staniceni: s, bod: a, smer, vrchol: i });
        }
        // kruhový oblouk (bez přechodnic přesně od TK do KT)
        let sweep = strana * (w.abs() - tau1 - tau2).max(0.0);
        let stred = [a[0] - strana * r * smer.sin(), a[1] + strana * r * smer.cos()];
        let uhel_a = (a[1] - stred[1]).atan2(a[0] - stred[0]);
        let konec = if l2 > 0.0 { [stred[0] + r * (uhel_a + sweep).cos(), stred[1] + r * (uhel_a + sweep).sin()] } else { kt };
        pridej(&mut prvky, &mut s, Prvek::Oblouk { stred, r, a, b: konec, uhel_a, sweep, vrchol: i });
        // přechodnice za obloukem
        if l2 > 0.0 {
            smer += sweep;
            body.push(HlavniBod { druh: DruhBodu::Kp, staniceni: s, bod: konec, smer, vrchol: i });
            pridej(&mut prvky, &mut s, Prvek::Prechodnice { a: konec, smer, k0: strana / r, k1: 0.0, delka: l2, vrchol: i });
        }
        // vrchol: střed celé křivky
        let s_v = (s_tk + s) / 2.0;
        let (bod_v, smer_v) = prvky
            .iter()
            .rev()
            .find(|(s0, p)| s_v >= *s0 - 1e-9 && s_v <= s0 + p.delka() + 1e-9)
            .map_or((pi, smer), |(s0, p)| p.bod(s_v - s0));
        // vnější vzdálenost: vrchol – střed křivky (s přechodnicemi z polohy)
        if l1 > 0.0 || l2 > 0.0 {
            udaje[i].vzepeti = vzd(pi, bod_v);
        }
        body.push(HlavniBod { druh: DruhBodu::V, staniceni: s_v, bod: pi, smer: smer_v, vrchol: i });
        body.push(HlavniBod {
            druh: if l2 > 0.0 { DruhBodu::Pt } else { DruhBodu::Kt },
            staniceni: s,
            bod: kt,
            smer: d2[1].atan2(d2[0]),
            vrchol: i,
        });
        cur = kt;
    }
    let konec = v[n - 1].bod();
    pridej(&mut prvky, &mut s, Prvek::Primka { a: cur, b: konec });
    body.push(HlavniBod {
        druh: DruhBodu::Ku,
        staniceni: s,
        bod: konec,
        smer: (v[n - 1].y - v[n - 2].y).atan2(v[n - 1].x - v[n - 2].x),
        vrchol: n - 1,
    });
    Ok(Geometrie { prvky, pocatek, hlavni_body: body, vrcholy: udaje })
}

impl Geometrie {
    pub fn delka(&self) -> f64 {
        self.prvky.iter().map(|(_, p)| p.delka()).sum()
    }
    pub fn konec(&self) -> f64 {
        self.pocatek + self.delka()
    }
    /// Bod a azimut osy ve staničení `s` (mimo trasu `None`).
    pub fn bod(&self, s: f64) -> Option<(Bod, f64)> {
        if s < self.pocatek - 1e-9 || s > self.konec() + 1e-9 {
            return None;
        }
        let mut posledni = None;
        for (s0, p) in &self.prvky {
            let l = p.delka();
            if s <= s0 + l + 1e-9 {
                return Some(p.bod((s - s0).clamp(0.0, l)));
            }
            posledni = Some((s0, p));
        }
        posledni.map(|(s0, p)| p.bod((s - s0).clamp(0.0, p.delka())))
    }
    /// Staničení, ve kterých leží značky po kroku `krok` (násobky kroku).
    pub fn staniceni_po_kroku(&self, krok: f64) -> Vec<f64> {
        if krok <= 0.0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut k = (self.pocatek / krok - 1e-9).ceil();
        loop {
            let s = k * krok;
            if s > self.konec() + 1e-9 {
                break;
            }
            out.push(s);
            k += 1.0;
            if out.len() > 100_000 {
                break;
            }
        }
        out
    }
    /// Průmět bodu na osu: (staničení, kolmá vzdálenost; + vlevo).
    pub fn prumet(&self, p: Bod) -> Option<(f64, f64)> {
        let mut nej: Option<(f64, f64)> = None;
        for (s0, e) in &self.prvky {
            let (t, q) = match *e {
                Prvek::Primka { a, b } => {
                    let d = jednotkovy(a, b);
                    let t = ((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]).clamp(0.0, vzd(a, b));
                    (t, [a[0] + t * d[0], a[1] + t * d[1]])
                }
                Prvek::Oblouk { stred, r, uhel_a, sweep, .. } => {
                    let f = (p[1] - stred[1]).atan2(p[0] - stred[0]);
                    let mut u = (f - uhel_a) * sweep.signum();
                    u = u.rem_euclid(2.0 * PI);
                    let l = sweep.abs();
                    let u = if u > l { if u - l < 2.0 * PI - u { l } else { 0.0 } } else { u };
                    let t = r * u;
                    (t, e.bod(t).0)
                }
                Prvek::Prechodnice { delka, .. } => {
                    // hrubě po krocích, pak zjemnit dělením intervalu
                    let n = 40;
                    let d2 = |t: f64| {
                        let q = e.bod(t).0;
                        (q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2)
                    };
                    let tk = |k: usize| delka * k as f64 / n as f64;
                    let k = (0..=n).min_by(|&a, &b| d2(tk(a)).total_cmp(&d2(tk(b)))).unwrap_or(0);
                    let (mut lo, mut hi) = (tk(k.saturating_sub(1)), tk((k + 1).min(n)));
                    for _ in 0..40 {
                        let (m1, m2) = (lo + (hi - lo) / 3.0, hi - (hi - lo) / 3.0);
                        if d2(m1) < d2(m2) {
                            hi = m2
                        } else {
                            lo = m1
                        }
                    }
                    let t = (lo + hi) / 2.0;
                    (t, e.bod(t).0)
                }
            };
            let d = vzd(p, q);
            if nej.is_none_or(|(_, dn)| d < dn.abs()) {
                let (_, smer) = e.bod(t);
                let strana = (smer.cos() * (p[1] - q[1]) - smer.sin() * (p[0] - q[0])).signum();
                nej = Some((s0 + t, d * strana));
            }
        }
        nej
    }
}

// ── Převod z polylinie ──────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum ChybaPolylinie {
    MaloVrcholu,
    Uzavrena,
    /// Polylinie začíná nebo končí obloukem.
    KrajniOblouk,
    /// Oblouk se středovým úhlem 180° a víc.
    VelkyOblouk(usize),
    /// Ve vrcholu polylinie (index) na sebe oblouk a sousední úsek nenavazují
    /// tečně; úhel v stupních.
    Netecny { vrchol: usize, uhel: f64 },
}

impl ChybaPolylinie {
    pub fn popis(&self) -> (&'static str, Vec<String>) {
        match self {
            ChybaPolylinie::MaloVrcholu => ("polylinie má méně než dva vrcholy", vec![]),
            ChybaPolylinie::Uzavrena => ("uzavřená polylinie nemůže být trasou", vec![]),
            ChybaPolylinie::KrajniOblouk => ("polylinie musí začínat i končit přímým úsekem", vec![]),
            ChybaPolylinie::VelkyOblouk(i) => ("oblouk za vrcholem {} polylinie má 180° nebo víc", vec![(i + 1).to_string()]),
            ChybaPolylinie::Netecny { vrchol, uhel } => (
                "ve vrcholu {} polylinie oblouk nenavazuje tečně (odchylka {}°)",
                vec![(vrchol + 1).to_string(), format!("{uhel:.3}")],
            ),
        }
    }
}

/// Povolená odchylka tečnosti v napojení oblouku [rad] (≈ 0,06°).
pub const TOL_TECNOSTI: f64 = 1e-3;

fn otoc(d: Bod, u: f64) -> Bod {
    let (s, c) = u.sin_cos();
    [d[0] * c - d[1] * s, d[0] * s + d[1] * c]
}

/// Převede polylinii (vrcholy s „bulge“ úseku za vrcholem) na tečnový polygon.
pub fn z_polylinie(body: &[(Bod, f64)], uzavrena: bool) -> Result<Vec<Vrchol>, ChybaPolylinie> {
    if uzavrena {
        return Err(ChybaPolylinie::Uzavrena);
    }
    // odstranit opakované body
    let mut b: Vec<(Bod, f64)> = Vec::new();
    for &(p, bulge) in body {
        if let Some(last) = b.last_mut() {
            if vzd(last.0, p) < 1e-9 {
                last.1 = bulge;
                continue;
            }
        }
        b.push((p, bulge));
    }
    if b.len() < 2 {
        return Err(ChybaPolylinie::MaloVrcholu);
    }
    struct Usek {
        a: Bod,
        b: Bod,
        oblouk: Option<(f64, f64)>, // (středový úhel se znaménkem, poloměr)
        d_a: Bod,
        d_b: Bod,
    }
    let mut useky = Vec::new();
    for i in 0..b.len() - 1 {
        let (a, bulge) = b[i];
        let c = b[i + 1].0;
        let d = jednotkovy(a, c);
        if bulge.abs() < 1e-12 {
            useky.push(Usek { a, b: c, oblouk: None, d_a: d, d_b: d });
        } else {
            let th = 4.0 * bulge.atan();
            if th.abs() >= PI - 1e-9 {
                return Err(ChybaPolylinie::VelkyOblouk(i));
            }
            let r = vzd(a, c) / (2.0 * (th.abs() / 2.0).sin());
            useky.push(Usek { a, b: c, oblouk: Some((th, r)), d_a: otoc(d, -th / 2.0), d_b: otoc(d, th / 2.0) });
        }
    }
    if useky.first().is_some_and(|u| u.oblouk.is_some()) || useky.last().is_some_and(|u| u.oblouk.is_some()) {
        return Err(ChybaPolylinie::KrajniOblouk);
    }
    for i in 1..useky.len() {
        let (p, q) = (&useky[i - 1], &useky[i]);
        if p.oblouk.is_some() || q.oblouk.is_some() {
            let u = odbocka(p.d_b, q.d_a).abs();
            if u > TOL_TECNOSTI {
                return Err(ChybaPolylinie::Netecny { vrchol: i, uhel: u.to_degrees() });
            }
        }
    }
    let mut v = vec![Vrchol::new(useky[0].a[0], useky[0].a[1], 0.0)];
    for i in 0..useky.len() {
        let u = &useky[i];
        match u.oblouk {
            Some((th, r)) => {
                let t = r * (th.abs() / 2.0).tan();
                v.push(Vrchol::new(u.a[0] + t * u.d_a[0], u.a[1] + t * u.d_a[1], r));
            }
            None => {
                // lom mezi dvěma přímými = vrchol bez oblouku
                if i + 1 < useky.len() && useky[i + 1].oblouk.is_none() {
                    let w = odbocka(u.d_b, useky[i + 1].d_a).abs();
                    if w > 1e-9 {
                        v.push(Vrchol::new(u.b[0], u.b[1], 0.0));
                    }
                }
            }
        }
    }
    let l = useky.last().map(|u| u.b).unwrap_or([0.0, 0.0]);
    v.push(Vrchol::new(l[0], l[1], 0.0));
    Ok(v)
}

#[cfg(test)]
mod testy {
    use super::*;

    fn blizko(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    /// Pravý úhel s poloměrem 10: T = 10, oblouk = 5π.
    fn pravy_uhel() -> Vec<Vrchol> {
        vec![Vrchol::new(0.0, 0.0, 0.0), Vrchol::new(100.0, 0.0, 10.0), Vrchol::new(100.0, 100.0, 0.0)]
    }

    #[test]
    fn delka_a_hlavni_body() {
        let g = geometrie(&pravy_uhel(), 0.0).unwrap();
        let l = 90.0 + 5.0 * PI + 90.0;
        assert!(blizko(g.delka(), l));
        let tk = g.hlavni_body.iter().find(|b| b.druh == DruhBodu::Tk).unwrap();
        let kt = g.hlavni_body.iter().find(|b| b.druh == DruhBodu::Kt).unwrap();
        assert!(blizko(tk.staniceni, 90.0) && blizko(tk.bod[0], 90.0) && blizko(tk.bod[1], 0.0));
        assert!(blizko(kt.staniceni, 90.0 + 5.0 * PI));
        assert!(blizko(kt.bod[0], 100.0) && blizko(kt.bod[1], 10.0));
        assert_eq!(g.hlavni_body.first().unwrap().druh, DruhBodu::Zu);
        assert_eq!(g.hlavni_body.last().unwrap().druh, DruhBodu::Ku);
        let u = g.vrcholy[1];
        assert!(blizko(u.tecna, 10.0));
        assert!(blizko(u.vzepeti, 10.0 * (2f64.sqrt() - 1.0)));
    }

    #[test]
    fn bod_na_staniceni() {
        let g = geometrie(&pravy_uhel(), 1000.0).unwrap();
        let (p, smer) = g.bod(1050.0).unwrap();
        assert!(blizko(p[0], 50.0) && blizko(p[1], 0.0) && blizko(smer, 0.0));
        // střed oblouku: 45° na kružnici se středem (90, 10)
        let (p, smer) = g.bod(1090.0 + 2.5 * PI).unwrap();
        let k = 10.0 / 2f64.sqrt();
        assert!(blizko(p[0], 90.0 + k) && blizko(p[1], 10.0 - k));
        assert!(blizko(smer, PI / 4.0));
        let (p, _) = g.bod(g.konec()).unwrap();
        assert!(blizko(p[0], 100.0) && blizko(p[1], 100.0));
        assert!(g.bod(999.0).is_none());
    }

    #[test]
    fn pravotocivy_oblouk() {
        let v = vec![Vrchol::new(0.0, 0.0, 0.0), Vrchol::new(100.0, 0.0, 10.0), Vrchol::new(100.0, -100.0, 0.0)];
        let g = geometrie(&v, 0.0).unwrap();
        let (p, _) = g.bod(90.0 + 2.5 * PI).unwrap();
        let k = 10.0 / 2f64.sqrt();
        assert!(blizko(p[0], 90.0 + k) && blizko(p[1], -10.0 + k));
        assert!(g.vrcholy[1].odbocka < 0.0);
    }

    #[test]
    fn prekryv_oblouku() {
        let v = vec![
            Vrchol::new(0.0, 0.0, 0.0),
            Vrchol::new(100.0, 0.0, 60.0),
            Vrchol::new(100.0, 50.0, 60.0),
            Vrchol::new(200.0, 50.0, 0.0),
        ];
        assert_eq!(geometrie(&v, 0.0), Err(Chyba::Prekryv { a: 1, b: 2 }));
        // nevejdoucí se poloměry se zmenší na největší možné
        let mut w = v.clone();
        let zm = prizpusob_polomery(&mut w);
        assert!(!zm.is_empty());
        assert!(geometrie(&w, 0.0).is_ok());
        assert!(w.iter().zip(&v).all(|(a, b)| a.r <= b.r));
    }

    #[test]
    fn lom_bez_oblouku() {
        let v = vec![Vrchol::new(0.0, 0.0, 0.0), Vrchol::new(100.0, 0.0, 0.0), Vrchol::new(100.0, 100.0, 0.0)];
        let g = geometrie(&v, 0.0).unwrap();
        assert!(blizko(g.delka(), 200.0));
        assert_eq!(g.prvky.len(), 2);
    }

    #[test]
    fn krok_staniceni() {
        let g = geometrie(&pravy_uhel(), 15.0).unwrap();
        let s = g.staniceni_po_kroku(20.0);
        assert_eq!(s.first().copied(), Some(20.0));
        assert!(s.last().copied().unwrap() <= g.konec());
    }

    #[test]
    fn prumet_bodu() {
        let g = geometrie(&pravy_uhel(), 0.0).unwrap();
        let (s, o) = g.prumet([50.0, 3.0]).unwrap();
        assert!(blizko(s, 50.0) && blizko(o, 3.0));
        let (s, o) = g.prumet([50.0, -2.0]).unwrap();
        assert!(blizko(s, 50.0) && blizko(o, -2.0));
    }

    #[test]
    fn polylinie_s_tecnym_obloukem() {
        // přímá (0,0)-(90,0), oblouk R10 do (100,10), přímá do (100,100)
        let bulge = (PI / 2.0 / 4.0).tan();
        let pl = vec![([0.0, 0.0], 0.0), ([90.0, 0.0], bulge), ([100.0, 10.0], 0.0), ([100.0, 100.0], 0.0)];
        let v = z_polylinie(&pl, false).unwrap();
        assert_eq!(v.len(), 3);
        assert!(blizko(v[1].x, 100.0) && blizko(v[1].y, 0.0) && blizko(v[1].r, 10.0));
        let g = geometrie(&v, 0.0).unwrap();
        assert!(blizko(g.delka(), 180.0 + 5.0 * PI));
    }

    #[test]
    fn polylinie_jen_primky() {
        let pl = vec![([0.0, 0.0], 0.0), ([50.0, 0.0], 0.0), ([100.0, 0.0], 0.0), ([100.0, 80.0], 0.0)];
        let v = z_polylinie(&pl, false).unwrap();
        // přímý vrchol (50,0) vypadne
        assert_eq!(v.len(), 3);
        assert!(v.iter().all(|x| x.r == 0.0));
    }

    #[test]
    fn polylinie_netecna() {
        let bulge = (PI / 2.0 / 4.0).tan();
        // oblouk začíná na konci přímé, ale směr nesedí
        let pl = vec![([0.0, 0.0], 0.0), ([90.0, 5.0], bulge), ([100.0, 15.0], 0.0), ([100.0, 100.0], 0.0)];
        assert!(matches!(z_polylinie(&pl, false), Err(ChybaPolylinie::Netecny { vrchol: 1, .. })));
    }

    #[test]
    fn polylinie_obratovy_oblouk() {
        // dva protisměrné oblouky přímo za sebou (S-křivka) jsou v pořádku
        let b = (PI / 2.0 / 4.0).tan();
        let pl = vec![
            ([0.0, 0.0], 0.0),
            ([90.0, 0.0], b),
            ([100.0, 10.0], -b),
            ([110.0, 20.0], 0.0),
            ([200.0, 20.0], 0.0),
        ];
        let v = z_polylinie(&pl, false).unwrap();
        assert_eq!(v.len(), 4);
        let g = geometrie(&v, 0.0).unwrap();
        assert!(blizko(g.delka(), 90.0 + 10.0 * PI + 90.0));
    }

    /// Konec každé křivky leží přesně na tečně (KT/PT = vrchol + T2 · směr).
    fn konce_na_tecnach(v: &[Vrchol]) {
        let g = geometrie(v, 0.0).unwrap();
        for b in g.hlavni_body.iter().filter(|b| matches!(b.druh, DruhBodu::Kt | DruhBodu::Pt)) {
            let s0 = b.staniceni - 1e-7;
            let (p, smer) = g.bod(s0).unwrap();
            assert!(vzd(p, b.bod) < 1e-5, "konec křivky {:?} mimo tečnu o {}", b.druh, vzd(p, b.bod));
            assert!((smer - b.smer).sin().abs() < 1e-6, "směr na konci křivky nesedí");
        }
        // spojitost: konec každého prvku = začátek dalšího
        for w in g.prvky.windows(2) {
            let (s0, p) = &w[0];
            let (s1, q) = &w[1];
            assert!((s0 + p.delka() - s1).abs() < 1e-9);
            let (a, sa) = p.bod(p.delka());
            let (b, sb) = q.bod(0.0);
            assert!(vzd(a, b) < 1e-5, "mezera mezi prvky {}", vzd(a, b));
            assert!((sa - sb).sin().abs() < 1e-6, "zlom směru mezi prvky");
        }
    }

    #[test]
    fn souvisle_prechodnice() {
        let v = vec![
            Vrchol::new(0.0, 0.0, 0.0),
            Vrchol::s_prechodnicemi(95.0, 46.0, 60.0, 22.0, 22.0),
            Vrchol::new(175.0, 6.0, 45.0),
            Vrchol::s_prechodnicemi(262.0, 58.0, 80.0, 18.0, 30.0),
            Vrchol::new(345.0, 40.0, 0.0),
        ];
        konce_na_tecnach(&v);
        let g = geometrie(&v, 0.0).unwrap();
        let druhy: Vec<_> = g.hlavni_body.iter().filter(|b| b.vrchol == 1).map(|b| b.druh).collect();
        assert_eq!(druhy, vec![DruhBodu::Tp, DruhBodu::Pk, DruhBodu::Kp, DruhBodu::V, DruhBodu::Pt]);
        let u = g.vrcholy[1];
        assert!(blizko(u.delka_celkem, u.delka_oblouku + 44.0));
        // souměrné přechodnice: T1 = T2
        assert!(blizko(u.tecna, u.tecna2));
        assert!(g.vrcholy[3].tecna < g.vrcholy[3].tecna2);
        assert_eq!(g.prvky.iter().filter(|(_, p)| p.je_prechodnice()).count(), 4);
    }

    #[test]
    fn prechodnice_jen_na_jedne_strane() {
        let v = vec![Vrchol::new(0.0, 0.0, 0.0), Vrchol::s_prechodnicemi(100.0, 0.0, 50.0, 0.0, 30.0), Vrchol::new(100.0, -120.0, 0.0)];
        konce_na_tecnach(&v);
        let g = geometrie(&v, 0.0).unwrap();
        let druhy: Vec<_> = g.hlavni_body.iter().filter(|b| b.vrchol == 1).map(|b| b.druh).collect();
        assert_eq!(druhy, vec![DruhBodu::Tk, DruhBodu::Kp, DruhBodu::V, DruhBodu::Pt]);
    }

    #[test]
    fn parametry_znamych_hodnot() {
        // R = 100, L = 50: τ = 0,25 rad, p ≈ L²/24R, A = √5000
        let (tau, p, k, a) = parametry_prechodnice(50.0, 100.0);
        assert!(blizko(tau, 0.25));
        // p = L²/24R − L⁴/2688R³ (+ zanedbatelné členy)
        assert!((p - (50.0f64.powi(2) / 2400.0 - 50.0f64.powi(4) / 2_688_000_000.0)).abs() < 1e-4);
        // k = L/2 − L³/240R²
        assert!((k - (25.0 - 125_000.0 / 2_400_000.0)).abs() < 1e-4);
        assert!(blizko(a, 5000f64.sqrt()));
        // délka klotoidy = délka po křivce
        let p = Prvek::Prechodnice { a: [0.0, 0.0], smer: 0.0, k0: 0.0, k1: 0.01, delka: 50.0, vrchol: 1 };
        let b = p.body(0.1);
        let l: f64 = b.windows(2).map(|w| vzd(w[0], w[1])).sum();
        assert!((l - 50.0).abs() < 1e-3);
    }

    #[test]
    fn prilis_dlouhe_prechodnice() {
        let v = vec![Vrchol::new(0.0, 0.0, 0.0), Vrchol::s_prechodnicemi(100.0, 0.0, 50.0, 60.0, 60.0), Vrchol::new(150.0, 20.0, 0.0)];
        assert_eq!(geometrie(&v, 0.0), Err(Chyba::DlouhePrechodnice(1)));
    }

    #[test]
    fn prumet_na_prechodnici() {
        let v = vec![Vrchol::new(0.0, 0.0, 0.0), Vrchol::s_prechodnicemi(100.0, 0.0, 50.0, 30.0, 30.0), Vrchol::new(100.0, 120.0, 0.0)];
        let g = geometrie(&v, 0.0).unwrap();
        let (s0, p) = g.prvky.iter().find(|(_, p)| p.je_prechodnice()).unwrap();
        let (q, smer) = p.bod(12.0);
        let bod = [q[0] - 2.0 * smer.sin(), q[1] + 2.0 * smer.cos()];
        let (s, o) = g.prumet(bod).unwrap();
        assert!((s - (s0 + 12.0)).abs() < 1e-4 && (o - 2.0).abs() < 1e-4);
    }

    #[test]
    fn polylinie_zacina_obloukem() {
        let b = 0.2;
        let pl = vec![([0.0, 0.0], b), ([10.0, 0.0], 0.0), ([20.0, 0.0], 0.0)];
        assert_eq!(z_polylinie(&pl, false), Err(ChybaPolylinie::KrajniOblouk));
    }
}
