//! Brand on the box (Soundcore, Samsung, Sony...). Not the chip vendor or ODM.
//! Family is the adapter key. Slug is the name shown in the TUI.

mod marks;

pub use marks::MarkBox;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Soundcore,
    Samsung,
    Sony,
    Xiaomi,
    Realme,
    OnePlus,
    Oppo,
    Vivo,
    Nothing,
    Google,
    Huawei,
    Honor,
    Jbl,
    Bose,
    Jabra,
    Sennheiser,
    Beats,
    Apple,
    Edifier,
    Marshall,
    Unknown,
}

impl Family {
    pub fn label(self) -> &'static str {
        match self {
            Self::Soundcore => "soundcore",
            Self::Samsung => "samsung",
            Self::Sony => "sony",
            Self::Xiaomi => "xiaomi",
            Self::Realme => "realme",
            Self::OnePlus => "oneplus",
            Self::Oppo => "oppo",
            Self::Vivo => "vivo",
            Self::Nothing => "nothing",
            Self::Google => "google",
            Self::Huawei => "huawei",
            Self::Honor => "honor",
            Self::Jbl => "jbl",
            Self::Bose => "bose",
            Self::Jabra => "jabra",
            Self::Sennheiser => "sennheiser",
            Self::Beats => "beats",
            Self::Apple => "apple",
            Self::Edifier => "edifier",
            Self::Marshall => "marshall",
            Self::Unknown => "unknown",
        }
    }

    /// How this Family yields named cells. None means OS cells only.
    pub fn cell_transport(self) -> CellTransport {
        match self {
            Self::Soundcore
            | Self::Samsung
            | Self::Sony
            | Self::Nothing
            | Self::Bose
            | Self::Oppo
            | Self::OnePlus
            | Self::Realme => CellTransport::Rfcomm,
            Self::Apple | Self::Beats => CellTransport::Aap,
            _ => CellTransport::None,
        }
    }

    pub fn cell_probe(self) -> bool {
        self.cell_transport() != CellTransport::None
    }
}

/// Wire used by a Family cell probe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellTransport {
    None,
    Rfcomm,
    Aap,
}

#[cfg(test)]
mod family_tests {
    use super::{CellTransport, Family};

    #[test]
    fn cell_transport_is_per_family_not_sku() {
        assert_eq!(Family::Soundcore.cell_transport(), CellTransport::Rfcomm);
        assert_eq!(Family::Samsung.cell_transport(), CellTransport::Rfcomm);
        assert_eq!(Family::Apple.cell_transport(), CellTransport::Aap);
        assert_eq!(Family::Beats.cell_transport(), CellTransport::Aap);
        assert_eq!(Family::Sony.cell_transport(), CellTransport::Rfcomm);
        assert_eq!(Family::Nothing.cell_transport(), CellTransport::Rfcomm);
        assert_eq!(Family::Bose.cell_transport(), CellTransport::Rfcomm);
        assert_eq!(Family::Oppo.cell_transport(), CellTransport::Rfcomm);
        assert_eq!(Family::OnePlus.cell_transport(), CellTransport::Rfcomm);
        assert_eq!(Family::Realme.cell_transport(), CellTransport::Rfcomm);
        assert!(Family::Apple.cell_probe());
        assert!(Family::Beats.cell_probe());
        assert!(Family::Sony.cell_probe());
        assert!(!Family::Unknown.cell_probe());
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Brand {
    pub family: Family,
    pub slug: &'static str,
}

impl Brand {
    pub const UNKNOWN: Self = Self {
        family: Family::Unknown,
        slug: "unknown",
    };

    pub fn detect(name: &str, uuids: &[String]) -> Self {
        let n = name.to_ascii_lowercase();
        if let Some(family) = family_from_uuids(uuids) {
            return slug_for(family, &n);
        }
        from_name(&n)
    }

    /// Half-block logo fitted to `mark`. Empty if this slug has no asset.
    pub fn logo(self, mark: MarkBox) -> Vec<String> {
        marks::render(self.slug, mark)
    }

    pub fn has_logo(self) -> bool {
        marks::is_available(self.slug)
    }

    /// RGB for the badge fill. Ink is dark on bright marks, light on dark marks.
    pub fn rgb(self) -> (u8, u8, u8) {
        match self.family {
            Family::Soundcore => (0, 173, 239),
            Family::Samsung => (20, 40, 160),
            Family::Sony => (0, 0, 0),
            Family::Xiaomi => (255, 105, 0),
            Family::Realme => (245, 197, 24),
            Family::OnePlus => (235, 0, 40),
            Family::Oppo => (0, 120, 90),
            Family::Vivo => (65, 95, 255),
            Family::Nothing => (209, 25, 33),
            Family::Google => (66, 133, 244),
            Family::Huawei => (207, 10, 44),
            Family::Honor => (16, 24, 48),
            Family::Jbl => (255, 102, 0),
            Family::Bose => (180, 20, 20),
            Family::Jabra => (230, 160, 20),
            Family::Sennheiser => (236, 28, 36),
            Family::Beats => (232, 17, 35),
            Family::Apple => (28, 28, 30),
            Family::Edifier => (200, 16, 46),
            Family::Marshall => (230, 170, 50),
            Family::Unknown => (66, 72, 82),
        }
    }

    pub fn ink_light(self) -> bool {
        matches!(
            self.family,
            Family::Samsung
                | Family::Sony
                | Family::Honor
                | Family::Apple
                | Family::Huawei
                | Family::Bose
                | Family::Unknown
        )
    }

    /// Name as printed on the box.
    pub fn display_name(self) -> &'static str {
        match self.slug {
            "soundcore" => "Soundcore",
            "anker" => "Anker",
            "samsung" => "Samsung",
            "sony" => "Sony",
            "xiaomi" => "Xiaomi",
            "redmi" => "Redmi",
            "poco" => "Poco",
            "realme" => "realme",
            "oneplus" => "OnePlus",
            "oppo" => "OPPO",
            "vivo" => "vivo",
            "nothing" => "Nothing",
            "cmf" => "CMF",
            "google" => "Google",
            "huawei" => "HUAWEI",
            "honor" => "HONOR",
            "jbl" => "JBL",
            "bose" => "Bose",
            "jabra" => "Jabra",
            "sennheiser" => "Sennheiser",
            "beats" => "Beats",
            "apple" => "Apple",
            "edifier" => "Edifier",
            "marshall" => "Marshall",
            _ => "Unknown",
        }
    }

    /// Brand plus model: `Soundcore P30i`, not a bare `P30i`.
    pub fn product_label(self, bt_name: &str) -> String {
        let name = collapse_ws(bt_name);
        if self.family == Family::Unknown {
            return if name.is_empty() {
                "unknown Device".into()
            } else {
                name
            };
        }
        let brand = self.display_name();
        let lower = name.to_ascii_lowercase();
        let needles = [brand.to_ascii_lowercase(), self.slug.to_string()];
        if needles.iter().any(|n| name_has_brand(&lower, n)) {
            return recase_leading_brand(&name, brand);
        }
        format!("{brand} {name}")
    }

    /// Brand RGB mixed toward mocha text so the mark stays readable on the TUI.
    pub fn ink(self) -> (u8, u8, u8) {
        let (r, g, b) = self.rgb();
        if (r as u16) + (g as u16) + (b as u16) < 48 {
            return (166, 173, 200);
        }
        mix((r, g, b), (166, 173, 200), 55)
    }

    pub fn wordmark(self) -> Vec<String> {
        self.logo(MarkBox::LOCKUP)
    }
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn name_has_brand(lower_name: &str, brand: &str) -> bool {
    if brand.is_empty() || brand == "unknown" {
        return false;
    }
    lower_name == brand
        || lower_name.starts_with(&format!("{brand} "))
        || lower_name.starts_with(&format!("{brand}-"))
        || lower_name
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(|w| w == brand)
}

fn recase_leading_brand(name: &str, brand: &str) -> String {
    let lower = name.to_ascii_lowercase();
    let b = brand.to_ascii_lowercase();
    if lower == b {
        return brand.to_string();
    }
    if lower.starts_with(&b)
        && name.get(b.len()..).is_some_and(|rest| {
            rest.starts_with(|c: char| !c.is_ascii_alphanumeric()) || rest.is_empty()
        })
    {
        return format!("{brand}{}", &name[b.len()..]);
    }
    name.to_string()
}

fn mix(a: (u8, u8, u8), b: (u8, u8, u8), percent_a: u16) -> (u8, u8, u8) {
    let p = percent_a.min(100);
    let q = 100 - p;
    (
        ((a.0 as u16 * p + b.0 as u16 * q) / 100) as u8,
        ((a.1 as u16 * p + b.1 as u16 * q) / 100) as u8,
        ((a.2 as u16 * p + b.2 as u16 * q) / 100) as u8,
    )
}

fn family_from_uuids(uuids: &[String]) -> Option<Family> {
    for u in uuids {
        let u = u.to_ascii_lowercase();
        if SOUNDCORE_UUIDS.iter().any(|k| u.contains(k)) {
            return Some(Family::Soundcore);
        }
        if SAMSUNG_UUIDS.iter().any(|k| u.contains(k)) {
            return Some(Family::Samsung);
        }
        if SONY_UUIDS.iter().any(|k| u.contains(k)) {
            return Some(Family::Sony);
        }
        if NOTHING_UUIDS.iter().any(|k| u.contains(k)) {
            return Some(Family::Nothing);
        }
        if OPO_UUIDS.iter().any(|k| u.contains(k)) {
            return Some(Family::Oppo);
        }
        if APPLE_UUIDS.iter().any(|k| u.contains(k)) {
            return Some(Family::Apple);
        }
    }
    None
}

fn from_name(n: &str) -> Brand {
    const RULES: &[(&str, Family)] = &[
        ("soundcore", Family::Soundcore),
        ("anker", Family::Soundcore),
        ("galaxy", Family::Samsung),
        ("samsung", Family::Samsung),
        ("buds2", Family::Samsung),
        ("buds3", Family::Samsung),
        ("buds fe", Family::Samsung),
        ("linkbuds", Family::Sony),
        ("wh-1000", Family::Sony),
        ("wh-ch", Family::Sony),
        ("wh-xb", Family::Sony),
        ("wh-ult", Family::Sony),
        ("ult wear", Family::Sony),
        ("wf-1000", Family::Sony),
        ("wf-c", Family::Sony),
        ("sony", Family::Sony),
        ("redmi", Family::Xiaomi),
        ("poco", Family::Xiaomi),
        ("xiaomi", Family::Xiaomi),
        ("mi buds", Family::Xiaomi),
        ("realme", Family::Realme),
        ("oneplus", Family::OnePlus),
        ("one plus", Family::OnePlus),
        ("nord buds", Family::OnePlus),
        ("cmf", Family::Nothing),
        ("nothing", Family::Nothing),
        ("pixel bud", Family::Google),
        ("pixelbuds", Family::Google),
        ("oppo", Family::Oppo),
        ("enco", Family::Oppo),
        ("vivo", Family::Vivo),
        ("iqoo", Family::Vivo),
        ("freebuds", Family::Huawei),
        ("huawei", Family::Huawei),
        ("honor", Family::Honor),
        ("jbl", Family::Jbl),
        ("quietcomfort", Family::Bose),
        ("bose", Family::Bose),
        ("jabra", Family::Jabra),
        ("sennheiser", Family::Sennheiser),
        ("momentum", Family::Sennheiser),
        ("beats", Family::Beats),
        ("airpods", Family::Apple),
        ("edifier", Family::Edifier),
        ("marshall", Family::Marshall),
    ];
    for (needle, family) in RULES {
        if n.contains(needle) {
            return slug_for(*family, n);
        }
    }
    Brand::UNKNOWN
}

fn slug_for(family: Family, n: &str) -> Brand {
    let (family, slug) = match family {
        Family::Soundcore if n.contains("anker") && !n.contains("soundcore") => {
            (Family::Soundcore, "anker")
        }
        Family::Soundcore => (Family::Soundcore, "soundcore"),
        Family::Samsung => (Family::Samsung, "samsung"),
        Family::Sony => (Family::Sony, "sony"),
        Family::Xiaomi if n.contains("redmi") => (Family::Xiaomi, "redmi"),
        Family::Xiaomi if n.contains("poco") => (Family::Xiaomi, "poco"),
        Family::Xiaomi => (Family::Xiaomi, "xiaomi"),
        Family::Realme => (Family::Realme, "realme"),
        Family::OnePlus => (Family::OnePlus, "oneplus"),
        Family::Oppo if n.contains("oneplus") || n.contains("nord") => (Family::OnePlus, "oneplus"),
        Family::Oppo if n.contains("realme") => (Family::Realme, "realme"),
        Family::Oppo => (Family::Oppo, "oppo"),
        Family::Vivo => (Family::Vivo, "vivo"),
        Family::Nothing if n.contains("cmf") => (Family::Nothing, "cmf"),
        Family::Nothing => (Family::Nothing, "nothing"),
        Family::Google => (Family::Google, "google"),
        Family::Huawei => (Family::Huawei, "huawei"),
        Family::Honor => (Family::Honor, "honor"),
        Family::Jbl => (Family::Jbl, "jbl"),
        Family::Bose => (Family::Bose, "bose"),
        Family::Jabra => (Family::Jabra, "jabra"),
        Family::Sennheiser => (Family::Sennheiser, "sennheiser"),
        Family::Beats => (Family::Beats, "beats"),
        Family::Apple if n.contains("beats") || n.contains("powerbeats") => {
            (Family::Beats, "beats")
        }
        Family::Apple => (Family::Apple, "apple"),
        Family::Edifier => (Family::Edifier, "edifier"),
        Family::Marshall => (Family::Marshall, "marshall"),
        Family::Unknown => (Family::Unknown, "unknown"),
    };
    Brand { family, slug }
}

const SOUNDCORE_UUIDS: &[&str] = &["0cf12d31-fac3-4553-bd80-d6832e7b3959"];

const SAMSUNG_UUIDS: &[&str] = &[
    "2e73a4ad-332d-41fc-90e2-16bef06523f2",
    "a23d00bc-217c-123b-9c00-fc44577136ee",
    "a7a473e9-19c6-491b-aea6-7ea92b8f043a",
    "b4a9d6a0-b2e3-4e40-976d-a69f167ea895",
];

const SONY_UUIDS: &[&str] = &[
    // v1 Tandem / MDR (96CC203E-…) and v2 (956C7B26-…).
    "96cc203e", "956c7b26", "0000fe0f",
];

const APPLE_UUIDS: &[&str] = &[
    // AAP / continuity: AirPods and Beats, not a single SKU.
    "74ec2172-0bad-4d01-8f77-997b2be0722a",
    "4715650b-5e9d-4ac2-b898-a4fc0aa5df78",
];

const NOTHING_UUIDS: &[&str] = &["aeac4a03"];

const OPO_UUIDS: &[&str] = &[
    "00001107-d102-11e1-9b23-00025b00a5a5",
    "0000079a-d102-11e1-9b23-00025b00a5a5",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn d(name: &str, uuids: &[&str]) -> Brand {
        Brand::detect(
            name,
            &uuids.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        )
    }

    #[test]
    fn p30i_is_soundcore_not_jieli() {
        let b = d("soundcore P30i", &["0cf12d31-fac3-4553-bd80-d6832e7b3959"]);
        assert_eq!(b.family, Family::Soundcore);
        assert_eq!(b.slug, "soundcore");
    }

    #[test]
    fn galaxy_uuid_without_helpful_name() {
        let b = d("headset", &["a23d00bc-217c-123b-9c00-fc44577136ee"]);
        assert_eq!(b.family, Family::Samsung);
        assert_eq!(b.slug, "samsung");
    }

    #[test]
    fn redmi_stays_xiaomi_family() {
        let b = d("Redmi Buds 5 Pro", &[]);
        assert_eq!(b.family, Family::Xiaomi);
        assert_eq!(b.slug, "redmi");
    }

    #[test]
    fn sony_wh() {
        let b = d("WH-1000XM5", &[]);
        assert_eq!(b.family, Family::Sony);
    }

    #[test]
    fn sony_v2_uuid_and_wh_ch_name() {
        let b = d("headset", &["956c7b26-d49a-4ba8-b03f-b17d393cb6e2"]);
        assert_eq!(b.family, Family::Sony);
        assert_eq!(d("WH-CH720N", &[]).family, Family::Sony);
        assert_eq!(d("ULT WEAR", &[]).family, Family::Sony);
    }

    #[test]
    fn nothing_and_cmf_are_one_family() {
        assert_eq!(d("Nothing Ear (2)", &[]).family, Family::Nothing);
        assert_eq!(d("CMF Buds Pro", &[]).slug, "cmf");
        assert_eq!(d("CMF Buds Pro", &[]).family, Family::Nothing);
        let b = d("headset", &["aeac4a03-dff5-498f-843a-34487cf133eb"]);
        assert_eq!(b.family, Family::Nothing);
    }

    #[test]
    fn opo_uuid_on_oneplus_stays_oneplus() {
        let b = d(
            "OnePlus Nord Buds 3",
            &["00001107-d102-11e1-9b23-00025b00a5a5"],
        );
        assert_eq!(b.family, Family::OnePlus);
        assert_eq!(b.slug, "oneplus");
        assert_eq!(d("OPPO Enco Air", &[]).family, Family::Oppo);
        assert_eq!(d("realme Buds Air", &[]).family, Family::Realme);
    }

    #[test]
    fn airpods_uuid_without_helpful_name() {
        let b = d("headset", &["74ec2172-0bad-4d01-8f77-997b2be0722a"]);
        assert_eq!(b.family, Family::Apple);
        assert_eq!(b.slug, "apple");
    }

    #[test]
    fn aap_uuid_on_beats_stays_beats() {
        let b = d("Beats Fit Pro", &["74ec2172-0bad-4d01-8f77-997b2be0722a"]);
        assert_eq!(b.family, Family::Beats);
        assert_eq!(b.slug, "beats");
    }

    #[test]
    fn buds3_name_is_samsung_family() {
        let b = d("Buds3", &[]);
        assert_eq!(b.family, Family::Samsung);
        assert_eq!(b.slug, "samsung");
    }

    #[test]
    fn unknown_stays_unknown() {
        assert_eq!(d("BT-123", &[]).family, Family::Unknown);
    }

    #[test]
    fn wordmark_is_the_real_mark_not_decoration() {
        let sc = d("soundcore P30i", &[]).wordmark();
        assert!(!sc.is_empty());
        assert!(sc
            .iter()
            .any(|row| row.chars().any(|c| c == '█' || c == '▀' || c == '▄')));
        assert!(d("BT-123", &[]).wordmark().is_empty());
    }

    #[test]
    fn product_label_keeps_the_brand() {
        assert_eq!(
            d("soundcore P30i", &[]).product_label("soundcore P30i"),
            "Soundcore P30i"
        );
        assert_eq!(
            d("Galaxy Buds2 Pro", &[]).product_label("Galaxy Buds2 Pro"),
            "Samsung Galaxy Buds2 Pro"
        );
        assert_eq!(
            d("WH-1000XM5", &[]).product_label("WH-1000XM5"),
            "Sony WH-1000XM5"
        );
        assert_eq!(
            d("Redmi Buds 5 Pro", &[]).product_label("Redmi Buds 5 Pro"),
            "Redmi Buds 5 Pro"
        );
        assert_eq!(
            d("AirPods Pro - Find My", &[]).product_label("AirPods Pro - Find My"),
            "Apple AirPods Pro - Find My"
        );
        assert_eq!(
            d("Beats Fit Pro", &[]).product_label("Beats Fit Pro"),
            "Beats Fit Pro"
        );
    }
}
