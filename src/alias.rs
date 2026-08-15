//! Alias: local nickname for a Device. The pack still records advertised name and address.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::brand::Brand;

/// Disambiguators when two Devices share a product name.
const TAGS: &[&str] = &[
    "Vega", "Rigel", "Polaris", "Altair", "Deneb", "Sirius", "Oslo", "Kyoto", "Lisbon", "Cairo",
    "Denver", "Bergen", "Atlas", "Orion", "Apollo", "Iris", "Nova", "Echo",
];

const LEGACY_ADJECTIVES: &[&str] = &[
    "Snow", "Iron", "Brass", "Ember", "Hollow", "Quiet", "Rapid", "Velvet", "Amber", "Cedar",
    "Dusk", "Flint", "Grove", "Haze", "Ivory", "Jade", "Kelp", "Lumen", "Moss", "North", "Opal",
    "Pewter", "Quill", "Rust",
];

const LEGACY_ANIMALS: &[&str] = &[
    "Bear", "Kite", "Moth", "Fox", "Wren", "Pike", "Hare", "Lynx", "Asp", "Crow", "Deer", "Elk",
    "Finch", "Gull", "Hawk", "Ibis", "Jay", "Koel", "Lark", "Mink", "Newt", "Owl", "Puma", "Rook",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AliasRecord {
    pub alias: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AliasBook {
    by_address: HashMap<String, AliasRecord>,
}

impl AliasBook {
    /// Load aliases. A missing file is an empty book. A corrupt file is copied
    /// aside and replaced with an empty book so the TUI still starts.
    pub fn load(path: &Path) -> (Self, Option<String>) {
        match std::fs::read_to_string(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Self::default(), None),
            Err(e) => (
                Self::default(),
                Some(format!("could not read aliases: {e}")),
            ),
            Ok(s) if s.trim().is_empty() => (Self::default(), None),
            Ok(s) => match serde_json::from_str(&s) {
                Ok(book) => (book, None),
                Err(e) => {
                    let bak = path.with_extension("json.bak");
                    let _ = std::fs::copy(path, &bak);
                    (
                        Self::default(),
                        Some(format!("aliases unreadable ({e}); started empty")),
                    )
                }
            },
        }
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)
    }

    pub fn get(&self, address: &str) -> Option<&AliasRecord> {
        self.by_address.get(&normalize_addr(address))
    }

    /// First sight: the product name. Later calls keep a custom Alias.
    /// Old generated `P30i-SnowBear` names are upgraded once.
    pub fn ensure(&mut self, address: &str, name: &str, brand: Brand) -> String {
        let key = normalize_addr(address);
        let taken: Vec<String> = self.by_address.values().map(|r| r.alias.clone()).collect();
        if let Some(r) = self.by_address.get_mut(&key) {
            if is_legacy_generated(&r.alias, &r.name, &key) {
                let alias = uniquify(&propose(name, &key, brand), &taken, &key);
                r.alias = alias.clone();
                r.name = name.to_string();
                return alias;
            }
            return r.alias.clone();
        }
        let alias = uniquify(&propose(name, &key, brand), &taken, &key);
        self.by_address.insert(
            key,
            AliasRecord {
                alias: alias.clone(),
                name: name.to_string(),
            },
        );
        alias
    }

    pub fn rename(&mut self, address: &str, new_alias: &str) -> Option<String> {
        let key = normalize_addr(address);
        let rec = self.by_address.get_mut(&key)?;
        let old = rec.alias.clone();
        rec.alias = sanitize(new_alias);
        Some(old)
    }

    pub fn alias_of(&self, address: &str) -> Option<&str> {
        self.by_address
            .get(&normalize_addr(address))
            .map(|r| r.alias.as_str())
    }
}

pub fn propose(name: &str, address: &str, brand: Brand) -> String {
    let _ = address;
    brand.product_label(name)
}

fn uniquify(base: &str, taken: &[String], address: &str) -> String {
    if !taken.iter().any(|a| a == base) {
        return base.to_string();
    }
    let tagged = format!("{base} {}", known_tag(address));
    if !taken.iter().any(|a| a == &tagged) {
        return tagged;
    }
    unique(&tagged, taken)
}

fn known_tag(address: &str) -> &'static str {
    TAGS[fnv(address.as_bytes()) as usize % TAGS.len()]
}

fn is_legacy_generated(alias: &str, name: &str, address: &str) -> bool {
    alias == legacy_propose(name, address)
}

fn legacy_propose(name: &str, address: &str) -> String {
    let model = short_model(name);
    let h = fnv(address.as_bytes());
    let adj = LEGACY_ADJECTIVES[h as usize % LEGACY_ADJECTIVES.len()];
    let animal = LEGACY_ANIMALS[(h as usize / LEGACY_ADJECTIVES.len()) % LEGACY_ANIMALS.len()];
    format!("{model}-{adj}{animal}")
}

pub fn short_model(name: &str) -> String {
    let tokens: Vec<&str> = name.split_whitespace().filter(|t| !t.is_empty()).collect();
    let picked = tokens
        .iter()
        .rev()
        .find(|t| t.chars().any(|c| c.is_ascii_digit()))
        .copied()
        .or_else(|| tokens.last().copied())
        .unwrap_or("bt");
    let mut s: String = picked
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if s.is_empty() {
        s = "bt".into();
    }
    s
}

fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn unique(base: &str, taken: &[String]) -> String {
    for n in 2..1000 {
        let candidate = format!("{base} {n}");
        if !taken.iter().any(|a| a == &candidate) {
            return candidate;
        }
    }
    format!("{base} x")
}

pub fn sanitize(raw: &str) -> String {
    let s: String = raw
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' => '-',
            c if c.is_control() => '-',
            c => c,
        })
        .collect::<String>()
        .trim()
        .trim_matches('.')
        .to_string();
    if s.is_empty() {
        "unnamed".into()
    } else {
        s
    }
}

pub fn normalize_addr(address: &str) -> String {
    address.trim().to_ascii_uppercase()
}

pub fn alias_file(root: &Path) -> PathBuf {
    root.join("aliases.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn soundcore() -> Brand {
        Brand::detect("soundcore P30i", &[])
    }

    #[test]
    fn p30i_model_token() {
        assert_eq!(short_model("soundcore P30i"), "P30i");
        assert_eq!(short_model("Galaxy Buds2 Pro"), "Buds2");
    }

    #[test]
    fn same_address_same_proposal() {
        let a = propose("soundcore P30i", "18:9C:2C:34:0B:D4", soundcore());
        let b = propose("soundcore P30i", "18:9C:2C:34:0B:D4", soundcore());
        assert_eq!(a, b);
        assert_eq!(a, "Soundcore P30i");
    }

    #[test]
    fn ensure_is_stable() {
        let mut book = AliasBook::default();
        let a = book.ensure("18:9c:2c:34:0b:d4", "soundcore P30i", soundcore());
        let b = book.ensure("18:9C:2C:34:0B:D4", "soundcore P30i", soundcore());
        assert_eq!(a, b);
        assert_eq!(a, "Soundcore P30i");
    }

    #[test]
    fn second_same_model_gets_a_known_tag() {
        let mut book = AliasBook::default();
        let first = book.ensure("AA:AA:AA:AA:AA:AA", "soundcore P30i", soundcore());
        let second = book.ensure("BB:BB:BB:BB:BB:BB", "soundcore P30i", soundcore());
        assert_eq!(first, "Soundcore P30i");
        assert!(second.starts_with("Soundcore P30i "), "{second}");
        assert_ne!(second, first);
    }

    #[test]
    fn upgrades_legacy_snowbear() {
        let addr = "18:9C:2C:34:0B:D4";
        let old = legacy_propose("soundcore P30i", addr);
        let json =
            format!(r#"{{"by_address":{{"{addr}":{{"alias":"{old}","name":"soundcore P30i"}}}}}}"#);
        let mut book: AliasBook = serde_json::from_str(&json).unwrap();
        let now = book.ensure(addr, "soundcore P30i", soundcore());
        assert_eq!(now, "Soundcore P30i");
        assert_ne!(now, old);
    }

    #[test]
    fn rename_keeps_address() {
        let mut book = AliasBook::default();
        book.ensure("AA:BB:CC:DD:EE:FF", "X1", Brand::UNKNOWN);
        book.rename("aa:bb:cc:dd:ee:ff", "desk");
        assert_eq!(book.alias_of("AA:BB:CC:DD:EE:FF"), Some("desk"));
    }

    #[test]
    fn custom_alias_is_left_alone() {
        let mut book = AliasBook::default();
        book.ensure("18:9C:2C:34:0B:D4", "soundcore P30i", soundcore());
        book.rename("18:9C:2C:34:0B:D4", "desk");
        let again = book.ensure("18:9C:2C:34:0B:D4", "soundcore P30i", soundcore());
        assert_eq!(again, "desk");
    }

    #[test]
    fn corrupt_alias_file_is_empty_book_not_a_panic() {
        let dir =
            std::env::temp_dir().join(format!("tws-tester-alias-corrupt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("aliases.json");
        std::fs::write(&p, "{not json").unwrap();
        let (book, warn) = AliasBook::load(&p);
        assert!(warn.is_some());
        assert!(book.alias_of("18:9C:2C:34:0B:D4").is_none());
        assert!(p.with_extension("json.bak").exists());
    }
}
