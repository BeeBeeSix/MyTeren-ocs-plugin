//! Povrchy pluginu Terén ve výkrese (jen čtení).
//!
//! Povrch Terénu je blok `TER_POVRCH_<ID>` vložený jednou; na vložení je
//! XDATA `TEREN_POVRCH` s definicí (`id=…`, `jmeno=…`). Trojúhelníky jsou
//! v bloku jako 3D plochy – ale jen když má povrch styl se zobrazeným TIN.

use ocs_plugin_api::host::{Document as CadDocument, EntityType, Handle};
use trasy::profil::Troj;

use crate::tra::objekt::retezce;

pub const APP_TEREN: &str = "TEREN_POVRCH";

#[derive(Clone, Debug)]
pub struct PovrchTerenu {
    pub id: String,
    pub jmeno: String,
    pub blok: String,
    /// Vložení bloku (pro hlídání změn povrchu – další krok).
    #[allow(dead_code)]
    pub insert: Handle,
    pub vlozeni: [f64; 3],
    pub otoceni: f64,
}

/// Povrchy Terénu ve výkrese.
pub fn povrchy(doc: &CadDocument) -> Vec<PovrchTerenu> {
    doc.model_space_entities()
        .filter_map(|e| {
            let EntityType::Insert(i) = e else { return None };
            let r = i.common.extended_data.get_record(APP_TEREN)?;
            let radky = retezce(r);
            let hodnota = |k: &str| radky.iter().find_map(|x| x.strip_prefix(k)).map(str::to_string);
            let id = hodnota("id=")?;
            let jmeno = hodnota("jmeno=").unwrap_or_else(|| id.clone());
            Some(PovrchTerenu {
                id,
                jmeno,
                blok: i.block_name.clone(),
                insert: i.common.handle,
                vlozeni: [i.insert_point.x, i.insert_point.y, i.insert_point.z],
                otoceni: i.rotation,
            })
        })
        .collect()
}

/// Trojúhelníky povrchu ve světových souřadnicích (z 3D ploch v bloku).
pub fn trojuhelniky(doc: &CadDocument, p: &PovrchTerenu) -> Vec<Troj> {
    let zaklad = doc
        .block_records
        .iter()
        .find(|b| b.name.eq_ignore_ascii_case(&p.blok))
        .map_or([0.0; 3], |b| [b.base_point.x, b.base_point.y, b.base_point.z]);
    let (s, c) = p.otoceni.sin_cos();
    let svet = |v: &ocs_plugin_api::host::codec::types::Vector3| {
        let (dx, dy) = (v.x - zaklad[0], v.y - zaklad[1]);
        [p.vlozeni[0] + dx * c - dy * s, p.vlozeni[1] + dx * s + dy * c, p.vlozeni[2] + v.z - zaklad[2]]
    };
    let mut out = Vec::new();
    for e in doc.entities_in_block(&p.blok) {
        if let EntityType::Face3D(f) = e {
            let (a, b, cc, d) = (svet(&f.first_corner), svet(&f.second_corner), svet(&f.third_corner), svet(&f.fourth_corner));
            out.push([a, b, cc]);
            // čtyřúhelník = dva trojúhelníky
            if (d[0] - cc[0]).hypot(d[1] - cc[1]) > 1e-9 {
                out.push([a, cc, d]);
            }
        }
    }
    out
}

#[cfg(test)]
mod testy {
    use super::*;
    use ocs_plugin_api::host::acadrust::DwgReader;
    use trasy::geometrie::{geometrie, z_polylinie};
    use trasy::profil::{profil_terenu, rozsah_vysek, Povrch};

    /// Profil červené polylinie nad povrchem z testovacího výkresu.
    #[test]
    fn profil_z_testovaciho_vykresu() {
        let cesta = concat!(env!("CARGO_MANIFEST_DIR"), "/../ukazka/trasy-zkouska.dwg");
        let Ok(mut r) = DwgReader::from_file(cesta) else { return };
        let doc = r.read().expect("čtení");
        let pov = povrchy(&doc);
        assert!(!pov.is_empty(), "v ukázce je povrch Terénu");
        for p in &pov {
            let t = trojuhelniky(&doc, p);
            let (mut a, mut b) = ([f64::MAX; 2], [f64::MIN; 2]);
            for x in t.iter().flatten() {
                a = [a[0].min(x[0]), a[1].min(x[1])];
                b = [b[0].max(x[0]), b[1].max(x[1])];
            }
            let idx = Povrch::new(t.clone());
            let ok = t.iter().filter(|x| { let c = [(x[0][0]+x[1][0]+x[2][0])/3.0, (x[0][1]+x[1][1]+x[2][1])/3.0]; idx.vyska(c[0], c[1]).is_some() }).count();
            eprintln!("  těžiště nalezena: {} z {}", ok, t.len());
            eprintln!("povrch {} ({}): {} trojúhelníků, rozsah {:?} – {:?}, vložení {:?}", p.jmeno, p.id, t.len(), a, b, p.vlozeni);
        }
        let mut nejlepsi = None;
        for e in doc.model_space_entities() {
            let EntityType::LwPolyline(pl) = e else { continue };
            let body: Vec<([f64; 2], f64)> = pl.vertices.iter().map(|v| ([v.location.x, v.location.y], v.bulge)).collect();
            let Ok(v) = z_polylinie(&body, pl.is_closed) else { continue };
            let g = geometrie(&v, 0.0).unwrap();
            let klik = [-719071.9999, -1087665.2836];
            eprintln!("  vzdálenost kliku {:.2} m", trasy::upravy::vzdalenost(&v, Some(&g), klik));
            eprintln!("  polylinie od {:?} do {:?}", v.first().map(|x| (x.x, x.y)), v.last().map(|x| (x.x, x.y)));
            for p in &pov {
                let idx = Povrch::new(trojuhelniky(&doc, p));
                let u = profil_terenu(&g, &idx, g.pocatek, g.konec());
                let bodu: usize = u.iter().map(Vec::len).sum();
                eprintln!("polylinie {:X} × {}: úseků {}, bodů {}, výšky {:?}", pl.common.handle.value(), p.jmeno, u.len(), bodu, rozsah_vysek(&u));
                if bodu > 0 {
                    nejlepsi = Some(bodu);
                }
            }
        }
        assert!(nejlepsi.is_some(), "aspoň jedna polylinie přechází přes povrch");
    }
}
