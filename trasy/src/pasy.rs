//! Pásy pod (a nad) podélným profilem: definice, místa zápisu, hodnoty.

/// Druh pásu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DruhPasu {
    /// Staničení.
    Staniceni,
    /// Výška terénu (povrch pohledu).
    Teren,
    /// Výška nivelety (návrhového profilu).
    Niveleta,
    /// Rozdíl dvou výšek (výchozí niveleta − terén: hloubka / výška násypu).
    Rozdil,
    /// Směrové poměry trasy (přímá, oblouk s poloměrem).
    Smer,
    /// Sklony nivelety s délkami úseků.
    Sklony,
    /// Výška z libovolného zdroje (jiný povrch, niveleta…).
    Kota,
    /// Vzdálenosti mezi místy zápisu.
    Vzdalenosti,
    /// Jeden text přes celou délku.
    Text,
    /// Úseky s textem (katastrální území, parcely, druh povrchu, uložení…).
    Useky,
}

impl DruhPasu {
    pub const VSE: [DruhPasu; 10] = [
        DruhPasu::Staniceni,
        DruhPasu::Teren,
        DruhPasu::Niveleta,
        DruhPasu::Rozdil,
        DruhPasu::Smer,
        DruhPasu::Sklony,
        DruhPasu::Kota,
        DruhPasu::Vzdalenosti,
        DruhPasu::Text,
        DruhPasu::Useky,
    ];

    pub fn kod(self) -> &'static str {
        match self {
            DruhPasu::Staniceni => "staniceni",
            DruhPasu::Teren => "teren",
            DruhPasu::Niveleta => "niveleta",
            DruhPasu::Rozdil => "rozdil",
            DruhPasu::Smer => "smer",
            DruhPasu::Sklony => "sklony",
            DruhPasu::Kota => "kota",
            DruhPasu::Vzdalenosti => "vzdalenosti",
            DruhPasu::Text => "text",
            DruhPasu::Useky => "useky",
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
            DruhPasu::Kota => "Kóta",
            DruhPasu::Vzdalenosti => "Vzdálenosti",
            DruhPasu::Text => "Text",
            DruhPasu::Useky => "Úseky s textem",
        }
    }
    /// Zapisuje hodnoty do míst (svislé texty)?
    pub fn hodnotovy(self) -> bool {
        matches!(self, DruhPasu::Staniceni | DruhPasu::Teren | DruhPasu::Niveleta | DruhPasu::Rozdil | DruhPasu::Kota)
    }
    /// Řádek s textem v rámečku (úseky, sklony, vzdálenosti…)?
    pub fn textovy(self) -> bool {
        matches!(self, DruhPasu::Sklony | DruhPasu::Vzdalenosti | DruhPasu::Text | DruhPasu::Useky)
    }
}

/// Odkud pás čte výšku.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Zdroj {
    /// Povrch pohledu (terénní profil).
    Teren,
    /// Jiný povrch Terénu podle id.
    Povrch(String),
    /// Niveleta trasy.
    Niveleta,
}

impl Zdroj {
    pub fn kod(&self) -> String {
        match self {
            Zdroj::Teren => "teren".into(),
            Zdroj::Niveleta => "niveleta".into(),
            Zdroj::Povrch(id) => format!("povrch:{id}"),
        }
    }
    pub fn z_kodu(k: &str) -> Option<Self> {
        match k.trim() {
            "teren" => Some(Zdroj::Teren),
            "niveleta" => Some(Zdroj::Niveleta),
            k => k.strip_prefix("povrch:").filter(|id| !id.is_empty()).map(|id| Zdroj::Povrch(id.into())),
        }
    }
}

/// Umístění pásu vůči grafu.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Poloha {
    #[default]
    Pod,
    Nad,
}

/// Úsek textového pásu: staničení od–do a text.
#[derive(Clone, Debug, PartialEq)]
pub struct Usek {
    pub od: f64,
    pub do_: f64,
    pub text: String,
}

/// Jeden pás.
#[derive(Clone, Debug, PartialEq)]
pub struct Pas {
    pub druh: DruhPasu,
    /// Výška řádku [mm na papíře].
    pub vyska: f64,
    /// Zápis po kroku [m]; 0 = ne.
    pub krok: f64,
    /// Zápis v lomech (terénu, nivelety).
    pub lomy: bool,
    /// Zápis v hlavních bodech trasy (ZÚ, TK, KT, KÚ).
    pub body_trasy: bool,
    /// Ruční staničení.
    pub rucni: Vec<f64>,
    /// Desetinná místa hodnot.
    pub desetin: usize,
    /// Vlastní záhlaví; prázdné = podle druhu.
    pub nazev: String,
    pub poloha: Poloha,
    /// Výška pro kótu, u rozdílu „A“.
    pub zdroj: Zdroj,
    /// U rozdílu „B“ (A − B).
    pub zdroj2: Zdroj,
    /// Text pásu Text.
    pub text: String,
    /// Úseky pásu Úseky.
    pub useky: Vec<Usek>,
    /// Text úseků na střed (jinak vlevo).
    pub na_stred: bool,
}

/// Zakóduje text do zápisu pásu (oddělovače `;`, `|`, `=`).
fn kod_textu(t: &str) -> String {
    t.replace('%', "%25").replace(';', "%3B").replace('|', "%7C").replace('=', "%3D")
}
fn dekod_textu(t: &str) -> String {
    t.replace("%3D", "=").replace("%7C", "|").replace("%3B", ";").replace("%25", "%")
}

impl Pas {
    pub fn novy(druh: DruhPasu) -> Self {
        let (vyska, desetin) = match druh {
            DruhPasu::Smer | DruhPasu::Sklony => (10.0, 1),
            DruhPasu::Vzdalenosti | DruhPasu::Text | DruhPasu::Useky => (6.0, 2),
            _ => (15.0, 2),
        };
        let (zdroj, zdroj2) = match druh {
            DruhPasu::Niveleta | DruhPasu::Rozdil => (Zdroj::Niveleta, Zdroj::Teren),
            _ => (Zdroj::Teren, Zdroj::Niveleta),
        };
        Self {
            druh,
            vyska,
            krok: 20.0,
            lomy: false,
            body_trasy: true,
            rucni: Vec::new(),
            desetin,
            nazev: String::new(),
            poloha: Poloha::Pod,
            zdroj,
            zdroj2,
            text: String::new(),
            useky: Vec::new(),
            na_stred: false,
        }
    }

    /// Záhlaví pásu (vlastní, jinak podle druhu – český text, plugin překládá).
    pub fn zahlavi(&self) -> &str {
        if self.nazev.trim().is_empty() { self.druh.nazev() } else { &self.nazev }
    }

    /// Řádek pro uložení: `kód;výška;krok;lomy;body;desetin;r1|r2|…` a za ním
    /// volitelné `klíč=hodnota` (starší verze je přeskočí).
    pub fn zapis(&self) -> String {
        let r: Vec<String> = self.rucni.iter().map(|s| format!("{s}")).collect();
        let mut t = format!(
            "{};{};{};{};{};{};{}",
            self.druh.kod(),
            self.vyska,
            self.krok,
            u8::from(self.lomy),
            u8::from(self.body_trasy),
            self.desetin,
            r.join("|")
        );
        let vychozi = Pas::novy(self.druh);
        if !self.nazev.is_empty() {
            t += &format!(";n={}", kod_textu(&self.nazev));
        }
        if self.poloha == Poloha::Nad {
            t += ";p=nad";
        }
        if self.zdroj != vychozi.zdroj {
            t += &format!(";z={}", kod_textu(&self.zdroj.kod()));
        }
        if self.zdroj2 != vychozi.zdroj2 {
            t += &format!(";z2={}", kod_textu(&self.zdroj2.kod()));
        }
        if !self.text.is_empty() {
            t += &format!(";t={}", kod_textu(&self.text));
        }
        if !self.useky.is_empty() {
            let u: Vec<String> = self.useky.iter().map(|u| format!("{}:{}:{}", u.od, u.do_, kod_textu(&u.text))).collect();
            t += &format!(";u={}", u.join("|"));
        }
        if self.na_stred {
            t += ";s=1";
        }
        t
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
        for kv in c.iter().skip(7) {
            let Some((k, v)) = kv.split_once('=') else { continue };
            match k {
                "n" => p.nazev = dekod_textu(v),
                "p" => p.poloha = if v == "nad" { Poloha::Nad } else { Poloha::Pod },
                "z" => p.zdroj = Zdroj::z_kodu(&dekod_textu(v)).unwrap_or(p.zdroj),
                "z2" => p.zdroj2 = Zdroj::z_kodu(&dekod_textu(v)).unwrap_or(p.zdroj2),
                "t" => p.text = dekod_textu(v),
                "u" => {
                    p.useky = v
                        .split('|')
                        .filter_map(|x| {
                            let mut d = x.splitn(3, ':');
                            let od = d.next()?.trim().parse().ok()?;
                            let do_ = d.next()?.trim().parse().ok()?;
                            Some(Usek { od, do_, text: dekod_textu(d.next().unwrap_or("")) })
                        })
                        .collect()
                }
                "s" => p.na_stred = v == "1",
                _ => {}
            }
        }
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

/// Úsek přes celou trasu (pro šablony; staničení se ořízne na rozsah pohledu).
fn cely(text: &str) -> Vec<Usek> {
    vec![Usek { od: 0.0, do_: 1e9, text: text.into() }]
}

fn pojmenovany(druh: DruhPasu, nazev: &str, vyska: f64) -> Pas {
    let mut p = Pas::novy(druh);
    p.nazev = nazev.into();
    p.vyska = vyska;
    p
}

/// Hodnoty jen v lomových bodech (bez kroku).
fn v_lomech(mut p: Pas) -> Pas {
    p.krok = 0.0;
    p.lomy = true;
    p.body_trasy = false;
    p
}

fn nad(mut p: Pas) -> Pas {
    p.poloha = Poloha::Nad;
    p
}

/// Šablony pohledu podle oboru (`obecna`, `kanalizace`, `tok`); neznámá = výchozí sada.
/// Pásy, pro které Trasy zatím nemají data (parcely, katastr…), jsou textové
/// úseky k vyplnění.
pub fn sada_sablony(sablona: &str) -> Vec<Pas> {
    use DruhPasu as D;
    match sablona {
        "obecna" => {
            let mut rozdil = pojmenovany(D::Rozdil, "Rozdíl (výkop − / násyp +)", 11.0);
            rozdil = v_lomech(rozdil);
            vec![
                nad(Pas { useky: cely("…"), ..pojmenovany(D::Useky, "Katastrální území", 5.0) }),
                nad(Pas { useky: cely("…"), ..pojmenovany(D::Useky, "Parcelní čísla", 5.0) }),
                nad(Pas { useky: cely("…"), ..pojmenovany(D::Useky, "Druh povrchu", 5.0) }),
                v_lomech(pojmenovany(D::Niveleta, "Kóta upraveného terénu", 13.0)),
                v_lomech(pojmenovany(D::Teren, "Kóta stávajícího terénu", 13.0)),
                rozdil,
                v_lomech(pojmenovany(D::Staniceni, "Staničení [km]", 15.0)),
                pojmenovany(D::Sklony, "Sklon [‰] – délka [m]", 5.0),
                Pas { useky: cely("…"), ..pojmenovany(D::Useky, "Druh úpravy", 5.0) },
            ]
        }
        "kanalizace" => vec![
            nad(Pas { useky: cely("…"), ..pojmenovany(D::Useky, "Katastrální území", 5.0) }),
            nad(Pas { useky: cely("…"), ..pojmenovany(D::Useky, "Parcelní čísla", 5.0) }),
            nad(Pas { useky: cely("…"), ..pojmenovany(D::Useky, "Druh povrchu", 5.0) }),
            nad(v_lomech(pojmenovany(D::Vzdalenosti, "Vzdálenosti šachet", 5.0))),
            v_lomech(pojmenovany(D::Teren, "Kóta terénu", 13.0)),
            v_lomech(Pas { zdroj: Zdroj::Teren, zdroj2: Zdroj::Niveleta, ..pojmenovany(D::Rozdil, "Hloubka dna potrubí", 11.0) }),
            v_lomech(pojmenovany(D::Niveleta, "Kóta dna potrubí", 13.0)),
            v_lomech(pojmenovany(D::Staniceni, "Staničení [km]", 15.0)),
            Pas { useky: cely("…"), ..pojmenovany(D::Useky, "Profil – materiál – délka", 5.0) },
            pojmenovany(D::Sklony, "Sklon [‰] – délka [m]", 5.0),
            Pas { useky: cely("pískové lože tl. 100 mm"), ..pojmenovany(D::Useky, "Uložení", 5.0) },
        ],
        "tok" => vec![
            nad(Pas { useky: cely("…"), ..pojmenovany(D::Useky, "Katastrální území", 5.0) }),
            nad(Pas { useky: cely("…"), ..pojmenovany(D::Useky, "Parcelní čísla", 5.0) }),
            nad(v_lomech(pojmenovany(D::Vzdalenosti, "Vzdálenosti profilů", 5.0))),
            v_lomech(pojmenovany(D::Teren, "Kóta dna stávající", 13.0)),
            v_lomech(pojmenovany(D::Niveleta, "Kóta dna navržená", 13.0)),
            v_lomech(pojmenovany(D::Rozdil, "Rozdíl dna (výkop − / násyp +)", 11.0)),
            v_lomech(pojmenovany(D::Staniceni, "Staničení [km]", 15.0)),
            pojmenovany(D::Sklony, "Sklon dna [‰] – délka [m]", 5.0),
            Pas { useky: cely("…"), ..pojmenovany(D::Useky, "Opevnění koryta", 5.0) },
        ],
        _ => vychozi_sada(),
    }
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
    fn zapis_novych_poli() {
        let mut p = Pas::novy(DruhPasu::Useky);
        p.nazev = "Druh; povrchu = test|x".into();
        p.poloha = Poloha::Nad;
        p.useky = vec![Usek { od: 0.0, do_: 120.5, text: "svah 1:2; 50 %".into() }, Usek { od: 120.5, do_: 330.0, text: "asfalt".into() }];
        p.na_stred = true;
        assert_eq!(Pas::cti(&p.zapis()), Some(p));
        let mut k = Pas::novy(DruhPasu::Kota);
        k.zdroj = Zdroj::Povrch("Zamereni2019".into());
        assert_eq!(Pas::cti(&k.zapis()), Some(k));
        let mut r = Pas::novy(DruhPasu::Rozdil);
        r.zdroj = Zdroj::Teren;
        r.zdroj2 = Zdroj::Povrch("P2".into());
        assert_eq!(Pas::cti(&r.zapis()), Some(r));
        // starý zápis bez nových polí
        let s = Pas::cti("teren;15;20;1;1;2;").unwrap();
        assert_eq!((s.poloha, s.nazev.as_str(), s.zdroj.clone()), (Poloha::Pod, "", Zdroj::Teren));
    }

    #[test]
    fn sablony() {
        for sab in ["obecna", "kanalizace", "tok"] {
            let s = sada_sablony(sab);
            assert!(s.iter().any(|p| p.poloha == Poloha::Nad), "{sab}");
            assert!(s.iter().any(|p| p.druh == DruhPasu::Staniceni), "{sab}");
            for p in &s {
                assert_eq!(Pas::cti(&p.zapis()).as_ref(), Some(p), "{sab}: {}", p.zahlavi());
            }
        }
        assert_eq!(sada_sablony("neznama"), vychozi_sada());
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
