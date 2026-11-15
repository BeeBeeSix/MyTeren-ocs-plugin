//! Zápis staničení a čísel podle českých zvyklostí.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormatStanice {
    /// `1,234 56` – kilometry, čárka, metry, mezera, desetiny.
    Km,
    /// `1+234.56`
    Plus,
    /// `1234.56`
    Metry,
}

impl FormatStanice {
    pub const VSE: [FormatStanice; 3] = [FormatStanice::Km, FormatStanice::Plus, FormatStanice::Metry];

    pub fn kod(self) -> &'static str {
        match self {
            FormatStanice::Km => "km",
            FormatStanice::Plus => "plus",
            FormatStanice::Metry => "m",
        }
    }
    pub fn z_kodu(s: &str) -> Self {
        match s.trim() {
            "plus" => FormatStanice::Plus,
            "m" => FormatStanice::Metry,
            _ => FormatStanice::Km,
        }
    }
    /// Ukázka formátu pro nabídky.
    pub fn ukazka(self) -> &'static str {
        match self {
            FormatStanice::Km => "1,234 56",
            FormatStanice::Plus => "1+234.56",
            FormatStanice::Metry => "1234.56",
        }
    }
}

/// Staničení `s` [m] jako text s `desetin` desetinnými místy.
pub fn staniceni(s: f64, f: FormatStanice, desetin: usize, carka: bool) -> String {
    let desetin = desetin.min(6);
    let nas = 10u64.pow(desetin as u32);
    let znam = if s < 0.0 { "-" } else { "" };
    let celkem = (s.abs() * nas as f64).round() as u64;
    let frac = celkem % nas;
    let metry_celkem = celkem / nas;
    let km = metry_celkem / 1000;
    let m = metry_celkem % 1000;
    let tecka = if carka { ',' } else { '.' };
    match f {
        FormatStanice::Km => {
            if desetin == 0 {
                format!("{znam}{km},{m:03}")
            } else {
                format!("{znam}{km},{m:03} {frac:0desetin$}")
            }
        }
        FormatStanice::Plus => {
            if desetin == 0 {
                format!("{znam}{km}+{m:03}")
            } else {
                format!("{znam}{km}+{m:03}{tecka}{frac:0desetin$}")
            }
        }
        FormatStanice::Metry => cislo(s, desetin, carka),
    }
}

/// Číslo s pevným počtem desetin a zvoleným desetinným oddělovačem.
pub fn cislo(v: f64, desetin: usize, carka: bool) -> String {
    let t = format!("{v:.desetin$}");
    let t = if t.starts_with('-') && t.trim_start_matches(['-', '0', '.']).is_empty() { t[1..].to_string() } else { t };
    if carka { t.replace('.', ",") } else { t }
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn km() {
        assert_eq!(staniceni(1234.5649, FormatStanice::Km, 2, true), "1,234 56");
        assert_eq!(staniceni(100.0, FormatStanice::Km, 0, true), "0,100");
        assert_eq!(staniceni(5.0, FormatStanice::Km, 2, true), "0,005 00");
        assert_eq!(staniceni(999.999, FormatStanice::Km, 2, true), "1,000 00");
        assert_eq!(staniceni(-12.3, FormatStanice::Km, 1, true), "-0,012 3");
    }

    #[test]
    fn plus_a_metry() {
        assert_eq!(staniceni(1234.567, FormatStanice::Plus, 2, false), "1+234.57");
        assert_eq!(staniceni(1234.567, FormatStanice::Plus, 2, true), "1+234,57");
        assert_eq!(staniceni(1234.567, FormatStanice::Metry, 2, true), "1234,57");
        assert_eq!(cislo(-0.001, 2, true), "0,00");
    }
}
