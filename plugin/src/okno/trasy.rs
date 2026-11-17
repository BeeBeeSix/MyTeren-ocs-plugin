//! Trasy, nivelety a pohledy podélných profilů v Prospektoru: větev stromu
//! a karty (přehled tras, trasa, pohled).

use teren::preklad::tr;
use teren::trf;

use eframe::egui::{self, RichText, Sense};
use egui::collapsing_header::CollapsingState;

use super::strom::{ik, polozka, Ikona, Vpravo};
use super::{cz, czi, karta, shoda, smazat, Okno, Uzel, CERVENA, MODRA, ORANZOVA, SEDA, ZELENA};
use crate::spravce::{self, Akce, StavSpravce};
use crate::tra::prospektor::{PrehledPohledu, PrehledTrasy};

// Příkaz pluginu Prospektor spustit neumí (Open CAD Studio ho z jiného
// vlákna nepustí) – objekt vybere a poradí tlačítko na pásu karet.
const N_VLASTNOSTI: &str = "Trasa je vybraná ve výkrese – výjimky stylu a vrcholy upravíte tlačítkem Vlastnosti trasy na pásu karet Terén.";
const N_POHLED: &str = "Trasa je vybraná ve výkrese – na pásu karet Terén klikněte na Podélný profil a potom do výkresu, kam profil umístit.";
const N_Z_POLYLINIE: &str = "Na pásu karet Terén klikněte na Nová trasa › Z polylinie a potom na polylinii osy.";
const N_KRESLIT: &str = "Na pásu karet Terén klikněte na Nová trasa › Kreslit vrcholy a klikejte vrcholy trasy.";
const N_NIV_TABULKA: &str = "Pohled je vybraný ve výkrese – na pásu karet Terén klikněte na Vrcholy nivelety.";
const N_NIVELETA: &str = "Pohled je vybraný ve výkrese – na pásu karet Terén klikněte na Niveleta a naklikejte vrcholy v pohledu.";

fn navod(h: Vec<ocs_plugin_api::host::Handle>, text: &str) {
    spravce::posli(Akce::Navod(h, text.to_string()));
}

fn stanice(s: f64) -> String {
    trasy::stanice::staniceni(s, trasy::stanice::FormatStanice::Km, 2, !teren::preklad::anglicky())
}

fn ikona_trasy() -> Ikona {
    Ikona::Svg(ik!("tra_editor"))
}

fn ikona_pohledu() -> Ikona {
    Ikona::Svg(ik!("tra_profil"))
}

fn ikona_nivelety() -> Ikona {
    Ikona::Svg(ik!("tra_niveleta"))
}

fn nazev_pohledu(p: &PrehledPohledu) -> String {
    trf!("Pohled {} · {}", p.id, p.povrch)
}

fn popis_trasy(t: &PrehledTrasy) -> String {
    match &t.chyba {
        Some(e) => trf!("chyba geometrie: {}", e),
        None => trf!(
            "{} m · {} vrcholů · {} oblouků · {}",
            cz(t.delka, 2),
            t.vrcholu,
            t.oblouku,
            t.styl
        ),
    }
}

/// Stav trasy pro řádek seznamu: (text, barva).
fn stav_trasy(t: &PrehledTrasy) -> (String, egui::Color32) {
    if t.chyba.is_some() {
        (tr("chyba").into(), CERVENA)
    } else if t.pohledy.iter().any(|p| p.zastaraly) {
        (tr("profil zastaralý").into(), ORANZOVA)
    } else {
        (tr("aktuální").into(), ZELENA)
    }
}

impl Okno {
    /// Větev stromu Trasy (v sekci Data).
    pub(super) fn strom_tras(&mut self, ui: &mut egui::Ui, stav: &StavSpravce) {
        let h = self.hledat.clone();
        let trasy: Vec<&PrehledTrasy> = stav.trasy.iter().filter(|t| shoda(&t.jmeno, &h)).collect();
        let zastarale = stav.trasy.iter().flat_map(|t| &t.pohledy).filter(|p| p.zastaraly).count();
        CollapsingState::load_with_default_open(ui.ctx(), egui::Id::new("u_trasy"), true)
            .show_header(ui, |ui| {
                let mut vpravo = Vec::new();
                if zastarale > 0 {
                    vpravo.push(Vpravo::Tecka(ORANZOVA));
                }
                vpravo.push(Vpravo::Pocet(stav.trasy.len().to_string()));
                let r = polozka(ui, self.uzel == Uzel::Trasy, &ikona_trasy(), RichText::new(tr("Trasy")), &vpravo, false);
                let r = if zastarale > 0 { r.on_hover_text(trf!("zastaralé pohledy profilu: {}", zastarale)) } else { r };
                if r.clicked() {
                    self.vyber_uzel(Uzel::Trasy);
                }
                r.context_menu(|ui| menu_tras(ui, stav));
            })
            .body(|ui| {
                if trasy.is_empty() {
                    ui.label(RichText::new(if stav.pripojeno { tr("zatím žádná trasa") } else { tr("načítám…") }).color(SEDA).small());
                }
                for t in trasy {
                    self.uzel_trasy(ui, stav, t);
                }
            });
    }

    fn uzel_trasy(&mut self, ui: &mut egui::Ui, stav: &StavSpravce, t: &PrehledTrasy) {
        let u = Uzel::Trasa(t.id.clone());
        let vybrana = stav.vyber_trasy.contains(&t.id);
        let deti = t.nivelety.len() + t.pohledy.len();
        let radek = |s: &mut Self, ui: &mut egui::Ui| {
            let mut vpravo = Vec::new();
            if t.chyba.is_some() {
                vpravo.push(Vpravo::Tecka(CERVENA));
            } else if t.pohledy.iter().any(|p| p.zastaraly) {
                vpravo.push(Vpravo::Tecka(ORANZOVA));
            }
            if vybrana {
                vpravo.push(Vpravo::Tecka(MODRA));
            }
            let r = polozka(ui, s.uzel == u, &ikona_trasy(), RichText::new(&t.jmeno), &vpravo, false).on_hover_text(popis_trasy(t));
            if r.clicked() {
                s.vyber_uzel(u.clone());
            }
            if r.double_clicked() {
                spravce::posli(Akce::Ukazat(t.stred, t.polomer, vec![t.insert]));
            }
            r.context_menu(|ui| s.menu_trasy(ui, t));
        };
        if deti == 0 {
            radek(self, ui);
            return;
        }
        CollapsingState::load_with_default_open(ui.ctx(), egui::Id::new(("u_trasa", t.id.as_str())), false)
            .show_header(ui, |ui| radek(self, ui))
            .body(|ui| {
                for n in &t.nivelety {
                    let r = polozka(
                        ui,
                        false,
                        &ikona_nivelety(),
                        RichText::new(trf!("Niveleta {}", n.jmeno)),
                        &[Vpravo::Pocet(trf!("{} vrch.", n.vrcholu))],
                        false,
                    );
                    if r.clicked() {
                        self.vyber_uzel(u.clone());
                    }
                }
                for p in &t.pohledy {
                    let up = Uzel::Pohled(p.id.clone());
                    let mut vpravo = Vec::new();
                    if p.zastaraly {
                        vpravo.push(Vpravo::Tecka(ORANZOVA));
                    }
                    if stav.vyber_pohledy.contains(&p.id) {
                        vpravo.push(Vpravo::Tecka(MODRA));
                    }
                    let r = polozka(ui, self.uzel == up, &ikona_pohledu(), RichText::new(nazev_pohledu(p)), &vpravo, false);
                    let r = if p.zastaraly { r.on_hover_text(tr("trasa nebo povrch se změnily – pohled je potřeba překreslit")) } else { r };
                    if r.clicked() {
                        self.vyber_uzel(up.clone());
                    }
                    if r.double_clicked() {
                        spravce::posli(Akce::Ukazat(p.stred, p.polomer, vec![p.insert]));
                    }
                    r.context_menu(|ui| menu_pohledu(ui, t, p));
                }
            });
    }

    fn menu_trasy(&mut self, ui: &mut egui::Ui, t: &PrehledTrasy) {
        if ui.button(tr("Ukázat ve výkrese")).clicked() {
            spravce::posli(Akce::Ukazat(t.stred, t.polomer, vec![t.insert]));
            ui.close();
        }
        if ui.button(tr("Editor trasy")).clicked() {
            spravce::posli(Akce::TrasaEditor(t.insert));
            ui.close();
        }
        if ui.button(tr("Vlastnosti trasy…")).clicked() {
            navod(vec![t.insert], tr(N_VLASTNOSTI));
            ui.close();
        }
        if ui.button(tr("Přejmenovat…")).clicked() {
            self.vyber_uzel(Uzel::Trasa(t.id.clone()));
            self.jmeno_trasy = (t.id.clone(), t.jmeno.clone(), t.pocatek, t.styl_id.clone());
            ui.close();
        }
        ui.separator();
        if ui.button(tr("Nový podélný profil…")).clicked() {
            navod(vec![t.insert], tr(N_POHLED));
            ui.close();
        }
        ui.separator();
        if ui.button(RichText::new(tr("Smazat trasu")).color(CERVENA)).clicked() {
            smazat_trasu(t);
            self.vyber_uzel(Uzel::Trasy);
            ui.close();
        }
    }

    /// Karta se seznamem tras.
    pub(super) fn detail_trasy(&mut self, ui: &mut egui::Ui, stav: &StavSpravce) {
        karta::nadpis_karty(ui, &ikona_trasy(), tr("Trasy"), |ui| {
            karta::pilulka(ui, &stav.trasy.len().to_string(), SEDA);
        });
        ui.add_space(4.0);
        if stav.trasy.is_empty() {
            karta::prazdny_stav(
                ui,
                ik!("tra_editor"),
                tr("Ve výkrese zatím není žádná trasa"),
                tr("Trasu (osu) vytvoříte z polylinie, nebo ji nakreslíte klikáním. Potom k ní přidáte podélný profil a niveletu."),
                |ui| {
                    if karta::tlacitko(ui, Some(ik!("tra_z_polylinie")), tr("Trasa z polylinie"), true).clicked() {
                        navod(Vec::new(), tr(N_Z_POLYLINIE));
                    }
                    if karta::tlacitko(ui, Some(ik!("tra_kreslit")), tr("Nakreslit trasu"), false).clicked() {
                        navod(Vec::new(), tr(N_KRESLIT));
                    }
                },
            );
            return;
        }
        ui.horizontal_wrapped(|ui| {
            if karta::tlacitko(ui, Some(ik!("tra_z_polylinie")), tr("Trasa z polylinie"), true).clicked() {
                navod(Vec::new(), tr(N_Z_POLYLINIE));
            }
            if karta::tlacitko(ui, Some(ik!("tra_kreslit")), tr("Nakreslit trasu"), false).clicked() {
                navod(Vec::new(), tr(N_KRESLIT));
            }
            if karta::tlacitko(ui, Some(ik!("tra_profil")), tr("Nový podélný profil…"), false).clicked() {
                navod(Vec::new(), tr(N_POHLED));
            }
            let zastarale = stav.trasy.iter().flat_map(|t| &t.pohledy).filter(|p| p.zastaraly).count();
            if zastarale > 0 && karta::tlacitko(ui, Some(ik!("tra_prepocitat")), &trf!("Překreslit zastaralé pohledy ({})", zastarale), false).clicked() {
                spravce::posli(Akce::PohledyPrekreslit(None));
            }
        });
        ui.add_space(6.0);
        let hledat = self.hledat.clone();
        egui::ScrollArea::vertical().id_salt("seznam_tras").auto_shrink([false, false]).show(ui, |ui| {
            for t in stav.trasy.iter().filter(|t| shoda(&t.jmeno, &hledat)) {
                let (st, barva) = stav_trasy(t);
                let mut popis = popis_trasy(t);
                if !t.pohledy.is_empty() || !t.nivelety.is_empty() {
                    popis.push_str(&trf!(" · profilů {} · nivelet {}", t.pohledy.len(), t.nivelety.len()));
                }
                let r = karta::radek_seznamu(ui, &ikona_trasy(), &t.jmeno, &popis, &st, barva);
                if r.clicked() {
                    self.vyber_uzel(Uzel::Trasa(t.id.clone()));
                }
                if r.double_clicked() {
                    spravce::posli(Akce::Ukazat(t.stred, t.polomer, vec![t.insert]));
                }
                r.context_menu(|ui| self.menu_trasy(ui, t));
            }
        });
    }

    /// Karta jedné trasy.
    pub(super) fn karta_trasy(&mut self, ui: &mut egui::Ui, stav: &StavSpravce, id: &str) {
        let Some(t) = stav.trasy.iter().find(|t| t.id == id).cloned() else {
            ui.label(RichText::new(tr("Trasa už ve výkrese není.")).color(SEDA));
            return;
        };
        let (st, barva) = stav_trasy(&t);
        karta::nadpis_karty(ui, &ikona_trasy(), &t.jmeno, |ui| {
            karta::pilulka(ui, &format!("● {st}"), barva);
            karta::pilulka(ui, &t.styl, SEDA).on_hover_text(if t.vyjimek > 0 {
                trf!("styl trasy, výjimek objektu: {}", t.vyjimek)
            } else {
                tr("styl trasy").to_string()
            });
            if stav.vyber_trasy.contains(&t.id) {
                karta::pilulka(ui, tr("vybraná ve výkrese"), MODRA);
            }
        });
        if let Some(e) = &t.chyba {
            ui.label(RichText::new(trf!("Geometrie trasy nejde spočítat: {}", e)).color(CERVENA));
        }
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            karta::kpi(ui, &format!("{} m", cz(t.delka, 2)), tr("délka"));
            karta::kpi(ui, &format!("{} – {}", stanice(t.pocatek), stanice(t.pocatek + t.delka)), tr("staničení"));
            karta::kpi(ui, &t.vrcholu.to_string(), tr("vrcholů"));
            karta::kpi(ui, &t.oblouku.to_string(), tr("oblouků"));
            if let Some(r) = t.min_r {
                karta::kpi(ui, &format!("{} m", cz(r, 1)), tr("nejmenší R"));
            }
        });
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            if karta::tlacitko(ui, Some(ik!("tra_editor")), tr("Editor trasy"), true).clicked() {
                spravce::posli(Akce::TrasaEditor(t.insert));
            }
            if karta::tlacitko(ui, Some(ik!("tra_vlastnosti")), tr("Vlastnosti…"), false).clicked() {
                navod(vec![t.insert], tr(N_VLASTNOSTI));
            }
            if karta::tlacitko(ui, None, tr("Ukázat ve výkrese"), false).clicked() {
                spravce::posli(Akce::Ukazat(t.stred, t.polomer, vec![t.insert]));
            }
            if karta::tlacitko(ui, Some(ik!("tra_smazat")), tr("Smazat"), false).clicked() {
                smazat_trasu(&t);
                self.vyber_uzel(Uzel::Trasy);
            }
        });

        // vlastnosti
        ui.add_space(10.0);
        if self.jmeno_trasy.0 != t.id {
            self.jmeno_trasy = (t.id.clone(), t.jmeno.clone(), t.pocatek, t.styl_id.clone());
        }
        let styly = crate::tra::prospektor::styly();
        egui::Grid::new(("vlastnosti_trasy", t.id.as_str())).num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            ui.label(tr("Jméno"));
            ui.add(egui::TextEdit::singleline(&mut self.jmeno_trasy.1).desired_width(220.0));
            ui.end_row();
            ui.label(tr("Počáteční staničení"));
            ui.add(egui::DragValue::new(&mut self.jmeno_trasy.2).speed(1.0).max_decimals(3).suffix(" m"));
            ui.end_row();
            ui.label(tr("Styl"));
            let nazev = styly.iter().find(|(id, _)| id == &self.jmeno_trasy.3).map_or_else(|| self.jmeno_trasy.3.clone(), |(_, n)| n.clone());
            egui::ComboBox::from_id_salt(("styl_trasy", t.id.as_str())).selected_text(nazev).show_ui(ui, |ui| {
                for (id, n) in &styly {
                    ui.selectable_value(&mut self.jmeno_trasy.3, id.clone(), n);
                }
            });
            ui.end_row();
        });
        let e = self.jmeno_trasy.clone();
        let zmeneno = !e.1.trim().is_empty() && (e.1.trim() != t.jmeno || (e.2 - t.pocatek).abs() > 1e-9 || e.3 != t.styl_id);
        ui.horizontal(|ui| {
            if ui.add_enabled(zmeneno, egui::Button::new(tr("Uložit změny"))).clicked() {
                spravce::posli(Akce::TrasaNastav(t.insert, e.1.trim().to_string(), e.2, e.3.clone()));
            }
            if zmeneno && ui.button(tr("Vrátit")).clicked() {
                self.jmeno_trasy.0.clear();
            }
        });

        // podélné profily
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr("Podélné profily")).strong());
            if ui.small_button(tr("+ nový")).on_hover_text(tr("vloží nový pohled podélného profilu této trasy")).clicked() {
                navod(vec![t.insert], tr(N_POHLED));
            }
        });
        if t.pohledy.is_empty() {
            ui.label(RichText::new(tr("Trasa zatím nemá podélný profil.")).color(SEDA));
        }
        for p in &t.pohledy {
            let (st, barva) = if p.zastaraly { (tr("zastaralý"), ORANZOVA) } else { (tr("aktuální"), ZELENA) };
            let popis = popis_pohledu(p);
            let r = karta::radek_seznamu(ui, &ikona_pohledu(), &nazev_pohledu(p), &popis, st, barva);
            if r.clicked() {
                self.vyber_uzel(Uzel::Pohled(p.id.clone()));
            }
            if r.double_clicked() {
                spravce::posli(Akce::Ukazat(p.stred, p.polomer, vec![p.insert]));
            }
            r.context_menu(|ui| menu_pohledu(ui, &t, p));
        }

        // nivelety
        ui.add_space(12.0);
        ui.label(RichText::new(tr("Nivelety")).strong());
        if t.nivelety.is_empty() {
            ui.label(RichText::new(if t.pohledy.is_empty() {
                tr("Niveletu nakreslíte v podélném profilu (nejdřív vložte profil).")
            } else {
                tr("Trasa zatím nemá niveletu – nakreslete ji tlačítkem Niveleta v pohledu profilu.")
            })
            .color(SEDA));
        }
        for n in &t.nivelety {
            let sklony: Vec<String> = n.sklony.iter().map(|i| format!("{} ‰", cz(*i, 1))).collect();
            let popis = trf!(
                "{} vrcholů · {} oblouků · {} – {} · výšky {} – {} m · sklony {}",
                n.vrcholu,
                n.oblouku,
                stanice(n.od),
                stanice(n.do_),
                cz(n.z_min, 2),
                cz(n.z_max, 2),
                sklony.join(", ")
            );
            let r = karta::radek_seznamu(ui, &ikona_nivelety(), &trf!("Niveleta {}", n.jmeno), &popis, "", SEDA);
            r.context_menu(|ui| {
                if let Some(p) = t.pohledy.first() {
                    if ui.button(tr("Vrcholy nivelety…")).clicked() {
                        navod(vec![p.insert], tr(N_NIV_TABULKA));
                        ui.close();
                    }
                }
            });
        }
    }

    /// Karta pohledu podélného profilu.
    pub(super) fn karta_pohledu(&mut self, ui: &mut egui::Ui, stav: &StavSpravce, id: &str) {
        let Some((t, p)) = stav.trasy.iter().find_map(|t| t.pohledy.iter().find(|p| p.id == id).map(|p| (t.clone(), p.clone()))) else {
            ui.label(RichText::new(tr("Pohled profilu už ve výkrese není.")).color(SEDA));
            return;
        };
        karta::nadpis_karty(ui, &ikona_pohledu(), &trf!("Podélný profil {}", p.id), |ui| {
            let (st, barva) = if p.zastaraly { (tr("● zastaralý"), ORANZOVA) } else { (tr("● aktuální"), ZELENA) };
            karta::pilulka(ui, st, barva);
            if stav.vyber_pohledy.contains(&p.id) {
                karta::pilulka(ui, tr("vybraný ve výkrese"), MODRA);
            }
        });
        if p.zastaraly {
            ui.label(RichText::new(tr("Trasa nebo povrch se od posledního kreslení změnily – pohled je potřeba překreslit.")).color(ORANZOVA));
        }
        if p.povrch_chybi {
            ui.label(RichText::new(trf!("Povrch {} ve výkrese není – terénní profil nejde spočítat.", p.povrch)).color(CERVENA));
        }
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            if karta::kpi(ui, &t.jmeno, tr("trasa")).interact(Sense::click()).on_hover_text(tr("otevřít trasu")).clicked() {
                self.vyber_uzel(Uzel::Trasa(t.id.clone()));
            }
            karta::kpi(ui, &p.povrch, tr("povrch"));
            if let Some((od, do_)) = p.rozsah {
                karta::kpi(ui, &format!("{} – {}", stanice(od), stanice(do_)), tr("staničení"));
            }
            karta::kpi(ui, &format!("1:{}", cz(p.meritko, 0)), tr("měřítko délek"));
            karta::kpi(ui, &format!("{}×", cz(p.prevyseni, 0)), tr("převýšení"));
            karta::kpi(ui, &czi(p.pasu), tr("pásů"));
        });
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            if karta::tlacitko(ui, None, tr("Ukázat ve výkrese"), true).clicked() {
                spravce::posli(Akce::Ukazat(p.stred, p.polomer, vec![p.insert]));
            }
            if karta::tlacitko(ui, Some(ik!("tra_prepocitat")), tr("Překreslit"), false).clicked() {
                spravce::posli(Akce::PohledyPrekreslit(Some(p.insert)));
            }
            if karta::tlacitko(ui, Some(ik!("tra_niveleta")), tr("Nakreslit niveletu"), false)
                .on_hover_text(tr("klikáním vrcholů v tomto pohledu; nahradí dosavadní niveletu trasy"))
                .clicked()
            {
                navod(vec![p.insert], tr(N_NIVELETA));
            }
            if !t.nivelety.is_empty() && karta::tlacitko(ui, Some(ik!("tra_vlastnosti")), tr("Vrcholy nivelety…"), false).clicked() {
                navod(vec![p.insert], tr(N_NIV_TABULKA));
            }
            if karta::tlacitko(ui, Some(ik!("tra_smazat")), tr("Smazat"), false).clicked() {
                smazat(trf!("Pohled profilu {} je smazaný.", p.id), Akce::PohledSmazat(p.insert));
                self.vyber_uzel(Uzel::Trasa(t.id.clone()));
            }
        });
        ui.add_space(10.0);
        self.editor_pohledu(ui, stav, &p);
    }
}

fn popis_pohledu(p: &PrehledPohledu) -> String {
    let rozsah = p.rozsah.map_or_else(String::new, |(od, do_)| format!("{} – {} · ", stanice(od), stanice(do_)));
    trf!("{}1:{} · převýšení {}× · pásů {}", rozsah, cz(p.meritko, 0), cz(p.prevyseni, 0), p.pasu)
}

fn smazat_trasu(t: &PrehledTrasy) {
    let text = if t.pohledy.is_empty() {
        trf!("Trasa „{}“ je smazaná.", t.jmeno)
    } else {
        trf!("Trasa „{}“ je smazaná i s pohledy profilu ({}).", t.jmeno, t.pohledy.len())
    };
    smazat(text, Akce::TrasaSmazat(t.insert));
}

fn menu_tras(ui: &mut egui::Ui, stav: &StavSpravce) {
    if ui.button(tr("Trasa z polylinie")).clicked() {
        navod(Vec::new(), tr(N_Z_POLYLINIE));
        ui.close();
    }
    if ui.button(tr("Nakreslit trasu")).clicked() {
        navod(Vec::new(), tr(N_KRESLIT));
        ui.close();
    }
    ui.separator();
    let zastarale = stav.trasy.iter().flat_map(|t| &t.pohledy).any(|p| p.zastaraly);
    if ui.add_enabled(zastarale, egui::Button::new(tr("Překreslit zastaralé pohledy"))).clicked() {
        spravce::posli(Akce::PohledyPrekreslit(None));
        ui.close();
    }
}

fn menu_pohledu(ui: &mut egui::Ui, t: &PrehledTrasy, p: &PrehledPohledu) {
    if ui.button(tr("Ukázat ve výkrese")).clicked() {
        spravce::posli(Akce::Ukazat(p.stred, p.polomer, vec![p.insert]));
        ui.close();
    }
    if ui.button(tr("Překreslit")).clicked() {
        spravce::posli(Akce::PohledyPrekreslit(Some(p.insert)));
        ui.close();
    }
    if ui.button(tr("Nakreslit niveletu")).clicked() {
        navod(vec![p.insert], tr(N_NIVELETA));
        ui.close();
    }
    if !t.nivelety.is_empty() && ui.button(tr("Vrcholy nivelety…")).clicked() {
        navod(vec![p.insert], tr(N_NIV_TABULKA));
        ui.close();
    }
    ui.separator();
    if ui.button(RichText::new(tr("Smazat pohled")).color(CERVENA)).clicked() {
        smazat(trf!("Pohled profilu {} je smazaný.", p.id), Akce::PohledSmazat(p.insert));
        ui.close();
    }
}

// ── Editor vzhledu a pásů pohledu ───────────────────────────────────────────

use trasy::pasy::{sada_sablony, vychozi_sada, DruhPasu, Pas, Poloha, Usek, Zdroj};
use trasy::pohled::{nastaveni, DefinicePohledu, SABLONY};

/// Hodnota nastavení bez výjimky pohledu (jen ze šablony).
fn ze_sablony(d: &DefinicePohledu, klic: &str) -> String {
    let mut c = d.clone();
    c.vyjimky.remove(klic);
    nastaveni(&c).text(klic).to_string()
}

/// Nastaví hodnotu jako výjimku pohledu (nebo ji zruší, když je stejná jako šablona).
fn nastav(d: &mut DefinicePohledu, klic: &str, hodnota: &str) {
    if ze_sablony(d, klic) == hodnota {
        d.vyjimky.remove(klic);
    } else {
        d.vyjimky.insert(klic.to_string(), hodnota.to_string());
    }
}

/// Stejné definice bez ohledu na otisk a osy (ty mění až kreslení).
fn stejne(a: &DefinicePohledu, b: &DefinicePohledu) -> bool {
    let (mut a, mut b) = (a.clone(), b.clone());
    a.otisk.clear();
    b.otisk.clear();
    a.osy = None;
    b.osy = None;
    a == b
}

fn volba(ui: &mut egui::Ui, d: &mut DefinicePohledu, salt: &str, klic: &str, moznosti: &[(&'static str, &'static str)]) {
    let akt = nastaveni(d).text(klic).to_string();
    let nazev = moznosti.iter().find(|m| m.0 == akt).map_or(akt.clone(), |m| tr(m.1).to_string());
    egui::ComboBox::from_id_salt((salt, klic)).selected_text(nazev).width(190.0).show_ui(ui, |ui| {
        for (k, n) in moznosti {
            if ui.selectable_label(akt == *k, tr(n)).clicked() {
                nastav(d, klic, k);
            }
        }
    });
}

fn zaskrt(ui: &mut egui::Ui, d: &mut DefinicePohledu, klic: &str, popis: &'static str) {
    let mut b = nastaveni(d).ano(klic);
    if ui.checkbox(&mut b, tr(popis)).changed() {
        nastav(d, klic, if b { "1" } else { "0" });
    }
}

fn nazev_zdroje(z: &Zdroj, povrchy: &[(String, String)]) -> String {
    match z {
        Zdroj::Teren => tr("Povrch pohledu").to_string(),
        Zdroj::Niveleta => tr("Niveleta").to_string(),
        Zdroj::Povrch(id) => povrchy.iter().find(|p| p.0 == *id).map_or_else(|| id.clone(), |p| p.1.clone()),
    }
}

fn vyber_zdroje(ui: &mut egui::Ui, salt: (&str, usize, &str), z: &mut Zdroj, povrchy: &[(String, String)]) {
    egui::ComboBox::from_id_salt(salt).selected_text(nazev_zdroje(z, povrchy)).width(190.0).show_ui(ui, |ui| {
        let mut moznosti = vec![Zdroj::Teren, Zdroj::Niveleta];
        moznosti.extend(povrchy.iter().map(|p| Zdroj::Povrch(p.0.clone())));
        for m in moznosti {
            let n = nazev_zdroje(&m, povrchy);
            if ui.selectable_label(*z == m, n).clicked() {
                *z = m;
            }
        }
    });
}

fn detail_pasu(ui: &mut egui::Ui, salt: &str, i: usize, pas: &mut Pas, povrchy: &[(String, String)]) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        egui::Grid::new((salt, i, "detail")).num_columns(2).spacing([10.0, 5.0]).show(ui, |ui| {
            ui.label(tr("Co pás ukazuje"));
            egui::ComboBox::from_id_salt((salt, i, "druh")).selected_text(tr(pas.druh.nazev())).width(190.0).show_ui(ui, |ui| {
                for d in DruhPasu::VSE {
                    if ui.selectable_label(pas.druh == d, tr(d.nazev())).clicked() && pas.druh != d {
                        let novy = Pas::novy(d);
                        pas.druh = d;
                        pas.zdroj = novy.zdroj;
                        pas.zdroj2 = novy.zdroj2;
                    }
                }
            });
            ui.end_row();
            ui.label(tr("Umístění"));
            ui.horizontal(|ui| {
                ui.radio_value(&mut pas.poloha, Poloha::Pod, tr("pod grafem"));
                ui.radio_value(&mut pas.poloha, Poloha::Nad, tr("nad grafem"));
            });
            ui.end_row();
            if matches!(pas.druh, DruhPasu::Kota | DruhPasu::Rozdil) {
                ui.label(if pas.druh == DruhPasu::Rozdil { tr("Výška A") } else { tr("Z čeho číst výšku") });
                vyber_zdroje(ui, (salt, i, "z1"), &mut pas.zdroj, povrchy);
                ui.end_row();
            }
            if pas.druh == DruhPasu::Rozdil {
                ui.label(tr("minus výška B"));
                vyber_zdroje(ui, (salt, i, "z2"), &mut pas.zdroj2, povrchy);
                ui.end_row();
            }
            if pas.druh.hodnotovy() || pas.druh == DruhPasu::Vzdalenosti {
                ui.label(tr("Hodnoty"));
                ui.horizontal(|ui| {
                    ui.label(tr("po"));
                    ui.add(egui::DragValue::new(&mut pas.krok).range(0.0..=1000.0).speed(1.0).suffix(" m"))
                        .on_hover_text(tr("0 = bez pravidelného kroku"));
                    ui.checkbox(&mut pas.lomy, tr("v lomových bodech"));
                    ui.checkbox(&mut pas.body_trasy, tr("v hlavních bodech trasy"));
                });
                ui.end_row();
            }
            if pas.druh.hodnotovy() || pas.druh == DruhPasu::Sklony {
                ui.label(tr("Desetinná místa"));
                ui.add(egui::DragValue::new(&mut pas.desetin).range(0..=4));
                ui.end_row();
            }
            if pas.druh == DruhPasu::Text {
                ui.label(tr("Text"));
                ui.add(egui::TextEdit::singleline(&mut pas.text).desired_width(220.0));
                ui.end_row();
            }
            if pas.druh.textovy() {
                ui.label("");
                ui.checkbox(&mut pas.na_stred, tr("text na střed úseku (jinak vlevo)"));
                ui.end_row();
            }
        });
        if pas.druh == DruhPasu::Useky {
            ui.add_space(4.0);
            ui.label(RichText::new(tr("Úseky (staničení od – do, text)")).color(SEDA));
            let mut smazat_usek = None;
            for (j, u) in pas.useky.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut u.od).speed(1.0).max_decimals(2).suffix(" m"));
                    ui.label("–");
                    // „do“ úseku přes celou trasu se zobrazí jako konec
                    ui.add(egui::DragValue::new(&mut u.do_).speed(1.0).max_decimals(2).suffix(" m").custom_formatter(|v, _| {
                        if v >= 1e8 { tr("konec").to_string() } else { cz(v, 2) }
                    }));
                    ui.add(egui::TextEdit::singleline(&mut u.text).desired_width(170.0));
                    if ui.small_button("×").on_hover_text(tr("odebrat úsek")).clicked() {
                        smazat_usek = Some(j);
                    }
                });
            }
            if let Some(j) = smazat_usek {
                pas.useky.remove(j);
            }
            if ui.small_button(tr("+ úsek")).clicked() {
                let od = pas.useky.last().map_or(0.0, |u| if u.do_ >= 1e8 { u.od } else { u.do_ });
                pas.useky.push(Usek { od, do_: 1e9, text: String::new() });
            }
        }
    });
}

impl Okno {
    /// Sekce „Vzhled a pásy“ v kartě pohledu: šablona, měřítko, nastavení, pásy.
    pub(super) fn editor_pohledu(&mut self, ui: &mut egui::Ui, stav: &StavSpravce, p: &PrehledPohledu) {
        if self.uprava_pohledu.as_ref().is_none_or(|(id, _)| *id != p.id) {
            self.uprava_pohledu = Some((p.id.clone(), p.def.clone()));
            self.otevreny_pas = None;
        }
        let povrchy: Vec<(String, String)> = stav.povrchy.iter().map(|z| (z.id.clone(), z.jmeno.clone())).collect();
        let salt = p.id.as_str();
        let mut otevreny = self.otevreny_pas;
        let mut pouzit = false;
        let mut vratit = false;
        let Some((_, d)) = self.uprava_pohledu.as_mut() else { return };
        egui::CollapsingHeader::new(RichText::new(tr("Vzhled a pásy")).strong()).id_salt(("vzhled_pohledu", salt)).default_open(true).show(ui, |ui| {
            egui::Grid::new(("nastaveni_pohledu", salt)).num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                ui.label(tr("Šablona"));
                let akt = SABLONY.iter().find(|s| s.0 == d.styl).map_or_else(|| d.styl.clone(), |s| trasy::preklad::tr_dyn(s.1));
                egui::ComboBox::from_id_salt(("sablona", salt)).selected_text(akt).width(190.0).show_ui(ui, |ui| {
                    for (sid, nazev, sada) in SABLONY {
                        if ui.selectable_label(d.styl == sid, trasy::preklad::tr_dyn(nazev)).clicked() && d.styl != sid {
                            d.styl = sid.to_string();
                            d.vyjimky.clear();
                            d.pasy = if sada.is_empty() { vychozi_sada() } else { sada_sablony(sada) };
                            otevreny = None;
                        }
                    }
                })
                .response
                .on_hover_text(tr("Změna šablony nahradí pásy i nastavení pohledu."));
                ui.end_row();
                ui.label(tr("Měřítko délek 1:"));
                ui.add(egui::DragValue::new(&mut d.meritko).range(10.0..=100000.0).speed(10.0).max_decimals(0));
                ui.end_row();
                ui.label(tr("Převýšení (výšky ×)"));
                ui.add(egui::DragValue::new(&mut d.prevyseni).range(0.5..=100.0).speed(0.5).max_decimals(1));
                ui.end_row();
                ui.label(tr("Čáry pásů"));
                volba(ui, d, salt, "cary_pasu", &[("svislice", "jen svislice"), ("ramecek", "rámeček a vodorovné čáry")]);
                ui.end_row();
                ui.label(tr("Výšková stupnice"));
                volba(ui, d, salt, "stupnice", &[("vlevo", "vlevo"), ("obe", "vlevo i vpravo"), ("skryt", "skrýt")]);
                ui.end_row();
                ui.label(tr("Formát staničení"));
                volba(ui, d, salt, "staniceni", &[("km", "0,040 00 (km)"), ("plus", "0+040.00"), ("m", "40.00 (m)")]);
                ui.end_row();
                ui.label(tr("Sklon"));
                volba(ui, d, salt, "sklon", &[("promile", "‰"), ("procento", "%")]);
                ui.end_row();
                ui.label(tr("Výška písma"));
                let mut pismo = nastaveni(d).cislo("pismo");
                if ui.add(egui::DragValue::new(&mut pismo).range(1.0..=10.0).speed(0.1).max_decimals(1).suffix(" mm")).changed() {
                    nastav(d, "pismo", &format!("{pismo}"));
                }
                ui.end_row();
                ui.label("");
                ui.horizontal(|ui| {
                    zaskrt(ui, d, "mrizka", "Mřížka");
                    zaskrt(ui, d, "ram", "Rámeček grafu");
                });
                ui.end_row();
            });

            ui.add_space(8.0);
            ui.label(RichText::new(tr("Pásy")).strong());
            ui.label(RichText::new(tr("název · výška · pořadí · ⚙ co pás ukazuje")).color(SEDA).size(11.0));
            let mut presun: Option<(usize, isize)> = None;
            let mut odebrat: Option<usize> = None;
            let n = d.pasy.len();
            for (i, pas) in d.pasy.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(if pas.poloha == Poloha::Nad { tr("nad") } else { tr("pod") }).color(SEDA).size(11.0))
                        .on_hover_text(tr("umístění vůči grafu"));
                    let mut nazev = if pas.nazev.trim().is_empty() { tr(pas.druh.nazev()).to_string() } else { pas.nazev.clone() };
                    if ui.add(egui::TextEdit::singleline(&mut nazev).desired_width(200.0)).changed() {
                        pas.nazev = nazev;
                    }
                    ui.add(egui::DragValue::new(&mut pas.vyska).range(3.0..=60.0).speed(0.5).max_decimals(1).suffix(" mm"));
                    if ui.add_enabled(i > 0, egui::Button::new("⬆").small()).on_hover_text(tr("výš")).clicked() {
                        presun = Some((i, -1));
                    }
                    if ui.add_enabled(i + 1 < n, egui::Button::new("⬇").small()).on_hover_text(tr("níž")).clicked() {
                        presun = Some((i, 1));
                    }
                    let zap = otevreny == Some(i);
                    if ui.selectable_label(zap, "⚙").on_hover_text(tr("co pás ukazuje")).clicked() {
                        otevreny = if zap { None } else { Some(i) };
                    }
                    if ui.small_button("×").on_hover_text(tr("odebrat pás")).clicked() {
                        odebrat = Some(i);
                    }
                });
                if otevreny == Some(i) {
                    detail_pasu(ui, salt, i, pas, &povrchy);
                }
            }
            if let Some((i, smer)) = presun {
                let j = (i as isize + smer) as usize;
                d.pasy.swap(i, j);
                if otevreny == Some(i) {
                    otevreny = Some(j);
                }
            }
            if let Some(i) = odebrat {
                d.pasy.remove(i);
                otevreny = None;
            }
            ui.menu_button(tr("+ Přidat pás"), |ui| {
                for druh in DruhPasu::VSE {
                    if ui.button(tr(druh.nazev())).clicked() {
                        d.pasy.push(Pas::novy(druh));
                        otevreny = Some(d.pasy.len() - 1);
                        ui.close();
                    }
                }
            });

            ui.add_space(8.0);
            let zmeneno = !stejne(d, &p.def);
            ui.horizontal(|ui| {
                if ui.add_enabled(zmeneno, egui::Button::new(tr("Použít"))).on_hover_text(tr("překreslí pohled ve výkrese (Ctrl+Z vrátí)")).clicked() {
                    pouzit = true;
                }
                if zmeneno && ui.button(tr("Vrátit změny")).clicked() {
                    vratit = true;
                }
            });
        });
        if pouzit {
            if let Some((_, d)) = &self.uprava_pohledu {
                spravce::posli(Akce::PohledUpravit(p.insert, Box::new(d.clone())));
            }
        }
        if vratit {
            self.uprava_pohledu = Some((p.id.clone(), p.def.clone()));
            otevreny = None;
        }
        self.otevreny_pas = otevreny;
    }
}
