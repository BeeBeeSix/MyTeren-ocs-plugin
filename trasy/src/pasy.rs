//! Pásy pod podélným profilem (tabulka): definice, místa zápisu, hodnoty.

/// Druh pásu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DruhPasu {
    /// Staničení.
    Staniceni,
    /// Výška terénu.
    Teren,
    /// Výška nivelety (návrhového profilu).
    Niveleta,
    /// Rozdíl niveleta − terén (hloubka / výška násypu).
    Rozdil,
    /// Směrové poměry trasy (přímá, oblouk s poloměrem).
    Smer,
    /// Sklony nivelety s délkami úseků.
    Sklony,
}

impl DruhPasu {
    pub const VSE: [DruhPasu; 6] =
        [DruhPasu::Staniceni, DruhPasu::Teren, DruhPasu::Niveleta, DruhPasu::Rozdil, DruhPasu::Smer, DruhPasu::Sklony];

    pub fn kod(self) -> &'static str {
        match self {
            DruhPasu::Staniceni => "staniceni",
            DruhPasu::Teren => "teren",
            DruhPasu::Niveleta => "niveleta",
            DruhPasu::Rozdil => "rozdil",
            DruhPasu::Smer => "smer",
            DruhPasu::Sklony => "sklony",
        }
    }
    pub fn z_kodu(k: &str) -> Option<Self> {
        Self::VSE.into_iter().find(|d| d.kod() == k)
    }
    /// Český název (záhlaví pásu; plugin překládá).
    pub fn nazev(self) -> &'static str {
        match self {
            DruhPasu::Staniceni => "Staničení",
            DruhPasu::Teren => "Výška terénu",
            DruhPasu::Niveleta => "Výška nivelety",
            DruhPasu::Rozdil => "Rozdíl (hloubka)",
            DruhPasu::Smer => "Směrové poměry",
            DruhPasu::Sklony => "Sklony nivelety",
        }
    }
}

/// Jeden pás.
#[derive(Clone, Debug, PartialEq)]
pub struct Pas {
    pub druh: DruhPasu,
    /// Výška řádku [mm na papíře].
    pub vyska: f64,
    /// Zápis po kroku [m]; 0 = ne.
    pub krok: f64,
    /// Zápis v lomech terénu.
    pub lomy: bool,
    /// Zápis v hlavních bodech trasy (ZÚ, TK, KT, KÚ).
    pub body_trasy: bool,
    /// Ruční staničení.
    pub rucni: Vec<f64>,
    /// Desetinná místa hodnot.
    pub desetin: usize,
}

impl Pas {
    pub fn novy(druh: DruhPasu) -> Self {
        let (vyska, desetin) = match druh {
            DruhPasu::Smer | DruhPasu::Sklony => (10.0, 1),
            DruhPasu::Staniceni => (15.0, 2),
            _ => (15.0, 2),
        };
        Self { druh, vyska, krok: 20.0, lomy: false, body_trasy: true, rucni: Vec::new(), desetin }
    }

    /// Řádek pro uložení: `kód;výška;krok;lomy;body;desetin;r1|r2|…`.
    pub fn zapis(&self) -> String {
        let r: Vec<String> = self.rucni.iter().map(|s| format!("{s}")).collect();
        format!(
            "{};{};{};{};{};{};{}",
            self.druh.kod(),
            self.vyska,
            self.krok,
            u8::from(self.lomy),
            u8::from(self.body_trasy),
            self.desetin,
            r.join("|")
        )
    }

    pub fn cti(t: &str) -> Option<Self> {
        let c: Vec<&str> = t.split(';').collect();
        let druh = DruhPasu::z_kodu(c.first()?)?;
        let mut p = Pas::novy(druh);
        let cislo = |i: usize| c.get(i).and_then(|x| x.trim().parse::<f64>().ok());
        if let Some(x) = cislo(1) {
            p.vyska = x.max(3.0);
        }
        if let Some(x) = cislo(2) {
            p.krok = x.max(0.0);
        }
        p.lomy = c.get(3).is_some_and(|x| *x == "1");
        p.body_trasy = c.get(4).is_none_or(|x| *x == "1");
        if let Some(x) = cislo(5) {
            p.desetin = (x as usize).min(4);
        }
        p.rucni = c.get(6).map_or(Vec::new(), |x| x.split('|').filter_map(|v| v.trim().parse().ok()).collect());
        Some(p)
    }
}

/// Výchozí sada pásů nového pohledu.
pub fn vychozi_sada() -> Vec<Pas> {
    let mut niveleta = Pas::novy(DruhPasu::Niveleta);
    niveleta.lomy = true; // ve vrcholech nivelety
    vec![
        Pas::novy(DruhPasu::Sklony),
        niveleta,
        Pas::novy(DruhPasu::Teren),
        Pas::novy(DruhPasu::Rozdil),
        Pas::novy(DruhPasu::Staniceni),
        Pas::novy(DruhPasu::Smer),
    ]
}

/// Staničení, ve kterých pás zapisuje hodnoty (seřazená, bez opakování).
pub fn mista(p: &Pas, od: f64, do_: f64, lomy: &[f64], body: &[f64]) -> Vec<f64> {
    let mut s = Vec::new();
    if p.krok > 0.0 {
        let mut x = (od / p.krok).ceil() * p.krok;
        while x <= do_ + 1e-9 && s.len() < 10_000 {
            s.push(x);
            x += p.krok;
        }
    }
    if p.lomy {
        s.extend_from_slice(lomy);
    }
    if p.body_trasy {
        s.extend_from_slice(body);
    }
    s.extend_from_slice(&p.rucni);
    s.retain(|x| *x >= od - 1e-9 && *x <= do_ + 1e-9);
    s.sort_by(|a, b| a.total_cmp(b));
    s.dedup_by(|a, b| (*a - *b).abs() < 1e-3);
    s
}

/// Místa zápisu bez kolizí: když jsou dvě místa blíž než `rozestup`, zůstane
/// to důležitější (ruční > body trasy > lomy > krok); stejně důležitá zůstanou obě.
pub fn mista_bez_kolizi(p: &Pas, od: f64, do_: f64, lomy: &[f64], body: &[f64], rozestup: f64) -> Vec<f64> {
    let blizko = |a: f64, b: f64| (a - b).abs() < 1e-3;
    let priorita = |s: f64| {
        if p.rucni.iter().any(|&r| blizko(r, s)) {
            3
        } else if p.body_trasy && body.iter().any(|&b| blizko(b, s)) {
            2
        } else if p.lomy && lomy.iter().any(|&l| blizko(l, s)) {
            1
        } else {
            0
        }
    };
    let mut kandidati: Vec<(f64, i32)> = mista(p, od, do_, lomy, body).into_iter().map(|s| (s, priorita(s))).collect();
    // od nejdůležitějších; méně důležité místo vypadne, když koliduje s ponechaným důležitějším
    kandidati.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.total_cmp(&b.0)));
    let mut ponechana: Vec<(f64, i32)> = Vec::new();
    for (s, pr) in kandidati {
        if ponechana.iter().any(|&(k, kp)| kp > pr && (k - s).abs() < rozestup) {
            continue;
        }
        ponechana.push((s, pr));
    }
    let mut out: Vec<f64> = ponechana.into_iter().map(|x| x.0).collect();
    out.sort_by(|a, b| a.total_cmp(b));
    out
}

/// Výška lomené čáry profilu ve staničení (lineárně; mimo úseky `None`).
pub fn vyska_v(useky: &[Vec<[f64; 2]>], s: f64) -> Option<f64> {
    for u in useky {
        for w in u.windows(2) {
            let (a, b) = (w[0], w[1]);
            if s >= a[0] - 1e-9 && s <= b[0] + 1e-9 {
                let t = if (b[0] - a[0]).abs() > 1e-12 { (s - a[0]) / (b[0] - a[0]) } else { 0.0 };
                return Some(a[1] + t * (b[1] - a[1]));
            }
        }
    }
    None
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn zapis_cteni() {
        let mut p = Pas::novy(DruhPasu::Teren);
        p.lomy = true;
        p.rucni = vec![12.5, 40.0];
        assert_eq!(Pas::cti(&p.zapis()), Some(p));
        assert_eq!(Pas::cti("nesmysl;1"), None);
    }

    #[test]
    fn mista_zapisu() {
        let mut p = Pas::novy(DruhPasu::Teren);
        p.rucni = vec![33.0, 500.0];
        let m = mista(&p, 5.0, 61.0, &[7.0, 8.0], &[5.0, 30.0, 61.0]);
        assert_eq!(m, vec![5.0, 20.0, 30.0, 33.0, 40.0, 60.0, 61.0]);
        p.lomy = true;
        assert!(mista(&p, 5.0, 61.0, &[7.0, 8.0], &[]).contains(&7.0));
    }

    #[test]
    fn bez_kolizi() {
        let p = Pas::novy(DruhPasu::Staniceni); // krok 20, body trasy
        // bod trasy 61,5 je moc blízko kroku 60 → krok vypadne
        let m = mista_bez_kolizi(&p, 0.0, 100.0, &[], &[51.05, 61.5], 3.0);
        assert!(m.contains(&51.05) && m.contains(&61.5) && !m.contains(&60.0) && m.contains(&40.0));
        // dva body trasy blízko sebe zůstanou oba
        let m = mista_bez_kolizi(&p, 0.0, 100.0, &[], &[50.0, 51.0], 3.0);
        assert!(m.contains(&50.0) && m.contains(&51.0));
    }

    #[test]
    fn vyska() {
        let u = vec![vec![[0.0, 10.0], [10.0, 20.0]], vec![[20.0, 5.0], [30.0, 5.0]]];
        assert_eq!(vyska_v(&u, 5.0), Some(15.0));
        assert_eq!(vyska_v(&u, 15.0), None);
        assert_eq!(vyska_v(&u, 25.0), Some(5.0));
    }
}
