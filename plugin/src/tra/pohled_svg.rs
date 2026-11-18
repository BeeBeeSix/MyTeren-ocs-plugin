//! Vývojová pomůcka: výstup kreslení pohledu profilu do SVG (bez OCS).
//! Spuštění: `TEREN_SVG=složka cargo test --release --lib svg_stylu -- --ignored`
//! – pro každý vestavěný styl profilu zapíše `<id>.svg` (ukázková data editoru).

use ocs_plugin_api::host::codec::entities::text::{TextHorizontalAlignment as Hz, TextVerticalAlignment as Vt};
use ocs_plugin_api::host::codec::entities::BoundaryEdge;
use ocs_plugin_api::host::codec::types::{Color, LineWeight, Transparency};
use ocs_plugin_api::host::EntityType;
use std::fmt::Write;

fn barva(c: &Color, vrstva: &str) -> String {
    match c {
        Color::Rgb { r, g, b } => {
            // světlé barvy (bílá „podle hladiny“) na papíře černě
            if (*r as u32 * 3 + *g as u32 * 6 + *b as u32) / 10 > 200 {
                "#1e1e1e".into()
            } else {
                format!("#{r:02x}{g:02x}{b:02x}")
            }
        }
        _ => match vrstva {
            super::pohled::H_TEREN => "#cc6600".into(),
            super::pohled::H_NIVELETA => "#ff0000".into(),
            _ => "#1e1e1e".into(),
        },
    }
}

fn sila(e: &EntityType, mm: f64) -> f64 {
    match e.common().line_weight {
        LineWeight::Value(v) => (v as f64 / 100.0 * mm).max(0.15),
        _ => 0.25 * mm,
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// SVG z entit (souřadnice modelu, `m` = metrů na milimetr papíru).
pub fn svg(ent: &[EntityType], m: f64) -> String {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    let mut roz = |x: f64, y: f64| {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    };
    for e in ent {
        match e {
            EntityType::Line(l) => {
                roz(l.start.x, l.start.y);
                roz(l.end.x, l.end.y);
            }
            EntityType::LwPolyline(p) => p.vertices.iter().for_each(|v| roz(v.location.x, v.location.y)),
            EntityType::Text(t) => roz(t.insertion_point.x, t.insertion_point.y),
            _ => {}
        }
    }
    let okraj = 10.0 * m;
    let (x0, y0, x1, y1) = (x0 - okraj, y0 - okraj, x1 + okraj, y1 + okraj);
    // 1 mm papíru = 4 px
    let k = 4.0 / m;
    let px = |x: f64| (x - x0) * k;
    let py = |y: f64| (y1 - y) * k;
    let mm = 4.0;
    let mut s = String::new();
    let _ = writeln!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{:.0}" height="{:.0}"><rect width="100%" height="100%" fill="white"/>"#,
        (x1 - x0) * k,
        (y1 - y0) * k
    );
    for e in ent {
        let c = e.common();
        let b = barva(&c.color, &c.layer);
        match e {
            EntityType::Hatch(h) => {
                let alfa = match c.transparency {
                    Transparency::Explicit(t) => 1.0 - t as f64 / 255.0,
                    _ => 1.0,
                };
                for cesta in &h.paths {
                    for hrana in &cesta.edges {
                        if let BoundaryEdge::Polyline(pe) = hrana {
                            let body: Vec<String> = pe.vertices.iter().map(|q| format!("{:.1},{:.1}", px(q.x), py(q.y))).collect();
                            let _ = writeln!(s, r#"<polygon points="{}" fill="{b}" fill-opacity="{:.2}"/>"#, body.join(" "), (alfa * 1.6).min(1.0));
                        }
                    }
                }
            }
            EntityType::Line(l) => {
                let _ = writeln!(
                    s,
                    r#"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="{b}" stroke-width="{:.2}"/>"#,
                    px(l.start.x),
                    py(l.start.y),
                    px(l.end.x),
                    py(l.end.y),
                    sila(e, mm)
                );
            }
            EntityType::LwPolyline(p) => {
                let mut body: Vec<String> = p.vertices.iter().map(|v| format!("{:.1},{:.1}", px(v.location.x), py(v.location.y))).collect();
                if p.is_closed && !body.is_empty() {
                    body.push(body[0].clone());
                }
                let _ = writeln!(s, r#"<polyline points="{}" fill="none" stroke="{b}" stroke-width="{:.2}"/>"#, body.join(" "), sila(e, mm));
            }
            EntityType::Text(t) if !t.value.is_empty() => {
                let bod = t.alignment_point.unwrap_or(t.insertion_point);
                let anchor = match t.horizontal_alignment {
                    Hz::Center | Hz::Middle => "middle",
                    Hz::Right => "end",
                    _ => "start",
                };
                let base = match t.vertical_alignment {
                    Vt::Top => "hanging",
                    Vt::Middle => "central",
                    Vt::Bottom => "text-after-edge",
                    _ => "alphabetic",
                };
                let tucne = if t.style.ends_with("_B") { "bold" } else { "normal" };
                let rodina = if t.style.contains("ISOCPEUR") { "ISOCPEUR, Arial Narrow, sans-serif" } else { "Bahnschrift, Arial, sans-serif" };
                let _ = writeln!(
                    s,
                    r#"<text x="{x:.1}" y="{y:.1}" font-size="{:.1}" font-family="{rodina}" font-weight="{tucne}" fill="{b}" text-anchor="{anchor}" dominant-baseline="{base}" transform="rotate({:.1} {x:.1} {y:.1})">{}</text>"#,
                    t.height * k / 0.72,
                    -t.rotation.to_degrees(),
                    esc(&t.value),
                    x = px(bod.x),
                    y = py(bod.y),
                );
            }
            _ => {}
        }
    }
    s.push_str("</svg>\n");
    s
}

#[cfg(test)]
mod testy {
    use trasy::pohled::{nastaveni_s, DefinicePohledu, StylProfilu};

    #[test]
    #[ignore = "vývojová pomůcka, spouštět ručně s TEREN_SVG"]
    fn svg_stylu() {
        let Some(slozka) = std::env::var_os("TEREN_SVG") else { return };
        let slozka = std::path::PathBuf::from(slozka);
        let _ = std::fs::create_dir_all(&slozka);
        let pk = crate::okno::ukazkove_podklady();
        for s in StylProfilu::vestavene() {
            let mut d = DefinicePohledu::novy("P1", "T1", "TEREN");
            d.styl = String::new();
            d.vyjimky = s.hodnoty.clone();
            d.pasy = s.pasy.clone();
            d.meritko = s.meritko;
            d.prevyseni = s.prevyseni;
            let nast = nastaveni_s(&d, &[]);
            let (ent, _, _) = super::super::pohled::kresli(&pk, &d, &nast);
            std::fs::write(slozka.join(format!("{}.svg", s.id)), super::svg(&ent, d.meritko / 1000.0)).unwrap();
        }
    }
}
