//! Trojúhelníky povrchu jako síť PolyfaceMesh.
//!
//! Formát DWG/DXF má u PolyfaceMesh indexy i počty jako 16bitová čísla,
//! proto se povrch dělí na díly po nejvýš [`MAX_NA_DIL`] vrcholech
//! a plochách. Jedna síť se vloží, smaže i načte o řád rychleji než
//! tisíce samostatných 3D ploch.

use std::collections::HashMap;

use ocs_plugin_api::host::codec::entities::polyface_mesh::{PolyfaceFace, PolyfaceMesh, PolyfaceVertex};
use ocs_plugin_api::host::EntityType;

use teren::povrch::{Definice, Povrch};

/// Nejvíc vrcholů i ploch v jednom dílu sítě (limit formátu je 32 767).
pub const MAX_NA_DIL: usize = 32_000;

fn klic(a: usize, b: usize) -> (usize, usize) {
    if a < b { (a, b) } else { (b, a) }
}

/// Trojúhelníky povrchu (globální souřadnice) rozdělené do sítí.
///
/// Hrany na povinných spojnicích a hranicích jsou skryté (záporný index
/// vrcholu): leží přesně na čarách v modelu, a protože se blok povrchu
/// kreslí až po nich, síť by je jinak překryla svou barvou.
pub fn site(p: &Povrch) -> Vec<PolyfaceMesh> {
    let g: Vec<[f64; 3]> = (0..p.vrcholy.len())
        .map(|i| {
            let (x, y, z) = p.globalni(i);
            [x, y, z]
        })
        .collect();
    // seřadit trojúhelníky po dlaždicích, aby díly byly souvislé plochy
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for v in &g {
        x0 = x0.min(v[0]);
        y0 = y0.min(v[1]);
        x1 = x1.max(v[0]);
        y1 = y1.max(v[1]);
    }
    let dilu = (p.trojuhelniky.len() as f64 / (MAX_NA_DIL as f64 * 0.8)).ceil().max(1.0);
    let k = dilu.sqrt().ceil().max(1.0);
    let (dx, dy) = (((x1 - x0) / k).max(1e-9), ((y1 - y0) / k).max(1e-9));
    let mut poradi: Vec<(u32, u32, usize)> = p
        .trojuhelniky
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let cx = (g[t[0]][0] + g[t[1]][0] + g[t[2]][0]) / 3.0;
            let cy = (g[t[0]][1] + g[t[1]][1] + g[t[2]][1]) / 3.0;
            (((cy - y0) / dy) as u32, ((cx - x0) / dx) as u32, i)
        })
        .collect();
    poradi.sort_unstable();

    let mut out = Vec::new();
    let mut m = PolyfaceMesh::new();
    let mut mapa: HashMap<usize, i16> = HashMap::new();
    for &(_, _, ti) in &poradi {
        let t = p.trojuhelniky[ti];
        let novych = t.iter().filter(|i| !mapa.contains_key(i)).count();
        if m.vertices.len() + novych > MAX_NA_DIL || m.faces.len() + 1 > MAX_NA_DIL {
            out.push(std::mem::replace(&mut m, PolyfaceMesh::new()));
            mapa.clear();
        }
        let idx = t.map(|i| {
            *mapa.entry(i).or_insert_with(|| {
                m.vertices.push(PolyfaceVertex::from_xyz(g[i][0], g[i][1], g[i][2]));
                m.vertices.len() as i16
            })
        });
        // hrana i vede z vrcholu i do vrcholu i+1
        let skryta = |a: usize, b: usize| p.povinne_hrany.contains(&klic(t[a], t[b]));
        let mut f = PolyfaceFace::triangle(idx[0], idx[1], idx[2]);
        if skryta(0, 1) {
            f.index1 = -f.index1;
        }
        if skryta(1, 2) {
            f.index2 = -f.index2;
        }
        if skryta(2, 0) {
            f.index3 = -f.index3;
        }
        m.faces.push(f);
    }
    if !m.faces.is_empty() {
        out.push(m);
    }
    out
}

/// Síť jako entita na hladině s barvou a tloušťkou; vrcholy a plochy sítě
/// nesou vlastní hladinu a barvu – dostanou stejné jako síť.
pub fn entita(mut m: PolyfaceMesh, hladina: &str, barva: ocs_plugin_api::host::codec::types::Color, tloustka: ocs_plugin_api::host::codec::types::LineWeight) -> EntityType {
    m.common.layer = hladina.to_string();
    m.common.color = barva;
    m.common.line_weight = tloustka;
    for v in &mut m.vertices {
        v.common.layer = hladina.to_string();
        v.common.color = barva;
        v.common.line_weight = tloustka;
    }
    for f in &mut m.faces {
        f.common.layer = hladina.to_string();
        f.common.color = barva;
        f.common.line_weight = tloustka;
    }
    EntityType::PolyfaceMesh(m)
}

/// Trojúhelníky sítě (indexy od 1, záporné = skrytá hrana) ve světových
/// souřadnicích podle `svet`.
pub fn trojuhelniky(m: &PolyfaceMesh, svet: impl Fn(&ocs_plugin_api::host::codec::types::Vector3) -> [f64; 3]) -> Vec<[[f64; 3]; 3]> {
    let vrchol = |i: i16| -> Option<[f64; 3]> {
        let i = i.unsigned_abs() as usize;
        (i >= 1).then(|| m.vertices.get(i - 1).map(|v| svet(&v.location))).flatten()
    };
    let mut out = Vec::with_capacity(m.faces.len());
    for f in &m.faces {
        let (Some(a), Some(b), Some(c)) = (vrchol(f.index1), vrchol(f.index2), vrchol(f.index3)) else { continue };
        out.push([a, b, c]);
        if let Some(d) = vrchol(f.index4) {
            out.push([a, c, d]);
        }
    }
    out
}

/// Odkud pochází povinná hrana povrchu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DruhHrany {
    Hranice,
    Spojnice,
    /// Hrana přidaná úpravou povrchu.
    Uprava,
}

/// Povinné hrany povrchu (skryté v síti) jako úsečky v globálních
/// souřadnicích s určením, zda leží na hranici, spojnici, nebo vznikly
/// úpravou. Kreslí se do bloku povrchu, aby trojúhelníky byly vidět celé
/// i bez čar spojnic a hranic v modelu.
pub fn povinne_cary(p: &Povrch, def: &Definice) -> Vec<(DruhHrany, [f64; 3], [f64; 3])> {
    // vzdálenost bodu od úsečky v půdorysu
    fn vzd(q: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l2 = dx * dx + dy * dy;
        let t = if l2 > 0.0 { (((q[0] - a[0]) * dx + (q[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
        (q[0] - a[0] - t * dx).hypot(q[1] - a[1] - t * dy)
    }
    // hrana leží na lomené čáře, když oba konce i střed leží na stejném úseku
    fn na_care(c: &[[f64; 2]], uzavrena: bool, a: [f64; 2], b: [f64; 2]) -> bool {
        let s = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
        let n = c.len();
        let useku = if uzavrena { n } else { n.saturating_sub(1) };
        (0..useku).any(|i| {
            let (u, w) = (c[i], c[(i + 1) % n]);
            [a, b, s].iter().all(|&q| vzd(q, u, w) < 1e-3)
        })
    }
    let spojnice: Vec<Vec<[f64; 2]>> = def.spojnice.iter().map(|c| c.iter().map(|q| [q[0], q[1]]).collect()).collect();
    let mut hrany: Vec<(usize, usize)> = p.povinne_hrany.iter().copied().collect();
    hrany.sort_unstable();
    hrany
        .into_iter()
        .map(|(i, j)| {
            let (a, b) = (p.globalni(i), p.globalni(j));
            let (a2, b2) = ([a.0, a.1], [b.0, b.1]);
            let druh = if def.hranice.iter().chain(&def.diry).any(|c| na_care(c, true, a2, b2)) {
                DruhHrany::Hranice
            } else if spojnice.iter().any(|c| na_care(c, false, a2, b2)) {
                DruhHrany::Spojnice
            } else {
                DruhHrany::Uprava
            };
            (druh, [a.0, a.1, a.2], [b.0, b.1, b.2])
        })
        .collect()
}

/// Hmotnost entity pro dávkování vkládání (limit 64 MB na zprávu pluginu).
pub fn vaha(e: &EntityType) -> usize {
    match e {
        EntityType::PolyfaceMesh(m) => m.faces.len() + m.vertices.len(),
        _ => 1,
    }
}

#[cfg(test)]
mod testy {
    use super::*;
    use teren::body::Bod;
    use teren::povrch::Definice;

    fn povrch(n: usize, spojnice: Vec<Vec<[f64; 3]>>) -> Povrch {
        let mut body = Vec::new();
        for i in 0..n {
            for j in 0..n {
                let (x, y) = (i as f64 * 2.0 + (j % 3) as f64 * 0.1, j as f64 * 2.0 + (i % 5) as f64 * 0.1);
                body.push(Bod { cislo: body.len().to_string(), x, y, z: 300.0 + x * 0.1, popis: String::new() });
            }
        }
        Povrch::z_definice(&Definice { body, spojnice, ..Default::default() }, 0.001).unwrap()
    }

    #[test]
    fn dily_do_limitu_a_vsechny_trojuhelniky() {
        let p = povrch(160, Vec::new());
        let s = site(&p);
        assert!(s.len() > 1, "velký povrch se rozdělí");
        assert!(s.iter().all(|m| m.vertices.len() <= MAX_NA_DIL && m.faces.len() <= MAX_NA_DIL));
        assert_eq!(s.iter().map(|m| m.faces.len()).sum::<usize>(), p.trojuhelniky.len());
        let zpet: usize = s.iter().map(|m| trojuhelniky(m, |v| [v.x, v.y, v.z]).len()).sum();
        assert_eq!(zpet, p.trojuhelniky.len());
    }

    #[test]
    fn hrany_spojnic_jsou_skryte() {
        let p = povrch(6, vec![vec![[0.0, 0.0, 300.0], [10.0, 10.0, 301.0]]]);
        assert!(!p.povinne_hrany.is_empty());
        let s = site(&p);
        let skrytych: usize = s.iter().flat_map(|m| &m.faces).map(|f| [f.index1, f.index2, f.index3].iter().filter(|i| **i < 0).count()).sum();
        // každá povinná hrana uvnitř má dva trojúhelníky, na kraji jeden
        assert!(skrytych >= p.povinne_hrany.len());
    }

    #[test]
    fn skryte_hrany_se_kresli_jako_cary_spojnic() {
        let sp = vec![vec![[0.0, 0.0, 300.0], [10.0, 10.0, 301.0]]];
        let p = povrch(6, sp.clone());
        let def = Definice { spojnice: sp, ..Default::default() };
        let cary = povinne_cary(&p, &def);
        assert_eq!(cary.len(), p.povinne_hrany.len());
        assert!(cary.iter().all(|c| c.0 == DruhHrany::Spojnice));
    }
}
