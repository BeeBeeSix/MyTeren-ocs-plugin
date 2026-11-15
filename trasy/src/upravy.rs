//! Úpravy tečnového polygonu trasy pro editor: vložení a smazání vrcholu,
//! oblouk ve vrcholu, nejbližší vrchol a úsek, tabulka prvků.

use crate::geometrie::{vzd, Bod, Geometrie, Prvek, Vrchol};

/// Vzdálenost bodu od úsečky a parametr průmětu (0..1).
fn k_usecce(p: Bod, a: Bod, b: Bod) -> (f64, f64) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    (vzd(p, [a[0] + t * dx, a[1] + t * dy]), t)
}

/// Nejbližší úsek tečnového polygonu: (index prvního vrcholu úseku, vzdálenost, parametr).
pub fn nejblizsi_usek(v: &[Vrchol], p: Bod) -> Option<(usize, f64, f64)> {
    (0..v.len().saturating_sub(1))
        .map(|i| {
            let (d, t) = k_usecce(p, v[i].bod(), v[i + 1].bod());
            (i, d, t)
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

/// Nejbližší vrchol: (index, vzdálenost).
pub fn nejblizsi_vrchol(v: &[Vrchol], p: Bod) -> Option<(usize, f64)> {
    v.iter().enumerate().map(|(i, x)| (i, vzd(p, x.bod()))).min_by(|a, b| a.1.total_cmp(&b.1))
}

/// Vzdálenost bodu od trasy (od polygonu i od osy, co je blíž).
pub fn vzdalenost(v: &[Vrchol], g: Option<&Geometrie>, p: Bod) -> f64 {
    let polygon = nejblizsi_usek(v, p).map_or(f64::MAX, |x| x.1);
    let osa = g.and_then(|g| g.prumet(p)).map_or(f64::MAX, |(_, o)| o.abs());
    polygon.min(osa)
}

/// Vloží vrchol do nejbližšího úseku polygonu v místě kolmého průmětu
/// kliknutí. Vrací nové vrcholy a index nového vrcholu.
pub fn vloz_vrchol(v: &[Vrchol], p: Bod, r: f64) -> Option<(Vec<Vrchol>, usize)> {
    let (i, _, t) = nejblizsi_usek(v, p)?;
    let (a, b) = (v[i].bod(), v[i + 1].bod());
    // ne těsně u stávajícího vrcholu
    let t = t.clamp(0.05, 0.95);
    let q = [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])];
    let mut n = v.to_vec();
    n.insert(i + 1, Vrchol::new(q[0], q[1], r.max(0.0)));
    Some((n, i + 1))
}

/// Smaže vrchol `i` (trasa musí mít aspoň dva vrcholy).
pub fn smaz_vrchol(v: &[Vrchol], i: usize) -> Option<Vec<Vrchol>> {
    if v.len() <= 2 || i >= v.len() {
        return None;
    }
    let mut n = v.to_vec();
    n.remove(i);
    if let Some(x) = n.first_mut() {
        x.r = 0.0;
    }
    if let Some(x) = n.last_mut() {
        x.r = 0.0;
    }
    Some(n)
}

/// Řádek tabulky prvků.
#[derive(Clone, Debug, PartialEq)]
pub struct RadekPrvku {
    pub oblouk: bool,
    pub od: f64,
    pub do_: f64,
    pub delka: f64,
    /// Poloměr se znaménkem (+ vlevo, − vpravo); 0 u přímé.
    pub polomer: f64,
    /// Vrchol polygonu, ke kterému oblouk patří.
    pub vrchol: Option<usize>,
}

pub fn tabulka_prvku(g: &Geometrie) -> Vec<RadekPrvku> {
    g.prvky
        .iter()
        .map(|(s, p)| match *p {
            Prvek::Primka { .. } => RadekPrvku { oblouk: false, od: *s, do_: s + p.delka(), delka: p.delka(), polomer: 0.0, vrchol: None },
            Prvek::Oblouk { r, sweep, vrchol, .. } => RadekPrvku {
                oblouk: true,
                od: *s,
                do_: s + p.delka(),
                delka: p.delka(),
                polomer: r * sweep.signum(),
                vrchol: Some(vrchol),
            },
        })
        .collect()
}

/// Číslo z buňky tabulky („100,00 P“, „1 250,5“, „37“).
pub fn cislo_z_bunky(t: &str) -> Option<f64> {
    let c: String = t.chars().filter(|c| c.is_ascii_digit() || matches!(c, ',' | '.' | '-')).collect();
    c.replace(',', ".").parse().ok()
}

/// Nové poloměry z upravené tabulky prvků: u oblouku se bere přepsané R,
/// jinak přepsaná délka (R = délka / úhel odbočení). Chyba = číslo řádku
/// (od 1), který nejde použít, nebo 0, když tabulka neodpovídá trase.
pub fn z_tabulky_prvku(v: &[Vrchol], g: &Geometrie, radky: &[Vec<String>]) -> Result<Vec<Vrchol>, usize> {
    let puvodni = tabulka_prvku(g);
    if puvodni.len() != radky.len() {
        return Err(0);
    }
    let mut n = v.to_vec();
    for (k, (p, r)) in puvodni.iter().zip(radky).enumerate() {
        let Some(i) = p.vrchol else { continue };
        let nove_r = r.get(4).and_then(|t| cislo_z_bunky(t));
        let nova_l = r.get(3).and_then(|t| cislo_z_bunky(t));
        if let Some(x) = nove_r.filter(|x| (x.abs() - p.polomer.abs()).abs() > 0.005) {
            if x < 0.0 {
                return Err(k + 1);
            }
            n[i].r = x;
        } else if let Some(l) = nova_l.filter(|l| (l - p.delka).abs() > 0.005) {
            let w = g.vrcholy[i].odbocka.abs();
            if l <= 0.0 || w <= 0.0 {
                return Err(k + 1);
            }
            n[i].r = l / w;
        }
    }
    Ok(n)
}

#[cfg(test)]
mod testy {
    use super::*;
    use crate::geometrie::geometrie;

    fn l() -> Vec<Vrchol> {
        vec![Vrchol::new(0.0, 0.0, 0.0), Vrchol::new(100.0, 0.0, 20.0), Vrchol::new(100.0, 100.0, 0.0)]
    }

    #[test]
    fn vlozeni() {
        let (n, i) = vloz_vrchol(&l(), [50.0, 3.0], 30.0).unwrap();
        assert_eq!(i, 1);
        assert_eq!(n.len(), 4);
        assert!((n[1].x - 50.0).abs() < 1e-9 && n[1].y.abs() < 1e-9 && n[1].r == 30.0);
        // vrchol na přímce nic nezmění na délce
        let g0 = geometrie(&l(), 0.0).unwrap();
        let g1 = geometrie(&n, 0.0).unwrap();
        assert!((g0.delka() - g1.delka()).abs() < 1e-6);
        let (_, i) = vloz_vrchol(&l(), [104.0, 60.0], 0.0).unwrap();
        assert_eq!(i, 2);
    }

    #[test]
    fn smazani() {
        let n = smaz_vrchol(&l(), 1).unwrap();
        assert_eq!(n.len(), 2);
        assert!(smaz_vrchol(&n, 0).is_none());
        let n = smaz_vrchol(&l(), 0).unwrap();
        assert_eq!(n[0].r, 0.0);
    }

    #[test]
    fn nejblizsi() {
        assert_eq!(nejblizsi_vrchol(&l(), [98.0, 4.0]).unwrap().0, 1);
        assert_eq!(nejblizsi_usek(&l(), [30.0, -5.0]).unwrap().0, 0);
    }

    #[test]
    fn uprava_z_tabulky() {
        let g = geometrie(&l(), 0.0).unwrap();
        let mut t: Vec<Vec<String>> = tabulka_prvku(&g)
            .iter()
            .map(|r| vec![String::new(), String::new(), String::new(), format!("{:.2}", r.delka), format!("{:.2} L", r.polomer)])
            .collect();
        assert_eq!(z_tabulky_prvku(&l(), &g, &t).unwrap(), l());
        t[1][4] = "30,00 L".into();
        assert_eq!(z_tabulky_prvku(&l(), &g, &t).unwrap()[1].r, 30.0);
        t[1][4] = format!("{:.2}", 20.0);
        t[1][3] = format!("{:.4}", 40.0 * std::f64::consts::FRAC_PI_2);
        assert!((z_tabulky_prvku(&l(), &g, &t).unwrap()[1].r - 40.0).abs() < 1e-3);
        assert_eq!(z_tabulky_prvku(&l(), &g, &t[..2]), Err(0));
        assert_eq!(cislo_z_bunky("1 250,5"), Some(1250.5));
    }

    #[test]
    fn vlozeny_vrchol_bez_oblouku() {
        // téměř přímý vrchol (zaokrouhlení) nesmí dát oblouk nulové délky
        let v = vec![Vrchol::new(0.0, 0.0, 0.0), Vrchol::new(50.0, 1e-8, 100.0), Vrchol::new(100.0, 0.0, 0.0)];
        let g = geometrie(&v, 0.0).unwrap();
        assert_eq!(tabulka_prvku(&g).iter().filter(|r| r.oblouk).count(), 0);
    }

    #[test]
    fn tabulka() {
        let g = geometrie(&l(), 0.0).unwrap();
        let t = tabulka_prvku(&g);
        assert_eq!(t.len(), 3);
        assert!(!t[0].oblouk && t[1].oblouk && !t[2].oblouk);
        assert!((t[1].polomer - 20.0).abs() < 1e-9);
        assert!((t[2].do_ - g.konec()).abs() < 1e-9);
    }
}
