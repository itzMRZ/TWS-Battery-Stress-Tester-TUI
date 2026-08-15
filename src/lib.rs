//! Soak a Bluetooth audio device until playback is gone.

pub mod alias;
pub mod brand;
pub mod cells;
pub mod cli;
pub mod death;
pub mod device;
pub mod host;
pub mod pack;
pub mod probe;
pub mod reference;
pub mod ui;

pub use alias::AliasBook;
pub use death::{DeathWatch, Decision, Observation};
pub use device::FoundDevice;

#[cfg(test)]
mod docs {
    const REPO: &str = "https://github.com/itzMRZ/TWS-Battery-Stress-Tester-TUI";

    #[test]
    fn github_repo_is_linked() {
        let cargo = include_str!("../Cargo.toml");
        let sh = include_str!("../scripts/install.sh");
        let ps = include_str!("../scripts/install.ps1");
        let readme = include_str!("../README.md");
        let contributing = include_str!("../CONTRIBUTING.md");
        for src in [cargo, sh, ps, readme, contributing] {
            assert!(src.contains(REPO), "repo URL missing");
            assert!(
                !src.contains("PLACEHOLDER/tws-tester"),
                "placeholder URL still present"
            );
        }
    }

    #[test]
    fn public_markdown_has_no_em_dash() {
        let files = [
            ("README.md", include_str!("../README.md")),
            ("CONTRIBUTING.md", include_str!("../CONTRIBUTING.md")),
            ("AGENTS.md", include_str!("../AGENTS.md")),
            ("SECURITY.md", include_str!("../SECURITY.md")),
            ("docs/families.md", include_str!("../docs/families.md")),
            (
                "docs/adr/0001-rust.md",
                include_str!("../docs/adr/0001-rust.md"),
            ),
            (
                "docs/adr/0002-tui-is-the-soak.md",
                include_str!("../docs/adr/0002-tui-is-the-soak.md"),
            ),
            (
                "docs/adr/0003-death-not-percent.md",
                include_str!("../docs/adr/0003-death-not-percent.md"),
            ),
            (
                "assets/logo/README.md",
                include_str!("../assets/logo/README.md"),
            ),
            (
                ".github/ISSUE_TEMPLATE/bug.md",
                include_str!("../.github/ISSUE_TEMPLATE/bug.md"),
            ),
            (
                ".github/ISSUE_TEMPLATE/device.md",
                include_str!("../.github/ISSUE_TEMPLATE/device.md"),
            ),
        ];
        for (name, src) in files {
            assert!(!src.contains('\u{2014}'), "{name} contains an em dash");
            assert!(!src.contains('\u{2013}'), "{name} contains an en dash");
        }
    }

    #[test]
    fn crate_version_is_0_1_1() {
        assert_eq!(env!("CARGO_PKG_VERSION"), "0.1.1");
        assert!(include_str!("../Cargo.toml").contains("version = \"0.1.1\""));
    }

    #[test]
    fn install_is_one_verified_linux_command() {
        let readme = include_str!("../README.md");
        assert!(readme.contains(
            "curl -sSfL https://github.com/itzMRZ/TWS-Battery-Stress-Tester-TUI/releases/latest/download/install.sh | sh"
        ));
        assert!(readme.contains("experimental"));
        assert!(readme.contains("tws-tester --history"));
        assert!(readme.contains("tws-tester --update"));
        assert!(!readme.contains("raw/main"));
        assert!(!readme.contains("From this tree"));
        assert!(!readme.contains("cargo install"));

        let sh = include_str!("../scripts/install.sh");
        assert!(sh.contains("sha256"));
        assert!(sh.contains("releases/latest/download"));
        assert!(!sh.contains("cargo install"));
        assert!(!sh.contains("raw/main"));

        let ps = include_str!("../scripts/install.ps1");
        assert!(ps.contains("SHA256") || ps.contains("SHA-256"));
        assert!(ps.contains("experimental"));
        assert!(!ps.contains("cargo install"));
        assert!(!ps.contains("raw/main"));

        let rel = include_str!("../.github/workflows/release.yml");
        assert!(rel.contains(".sha256"));
        assert!(rel.contains("tag matches Cargo.toml version"));
        assert!(rel.contains("Get-FileHash"));
        assert!(
            rel.contains("UTF8Encoding"),
            "Windows checksum file must be UTF-8 without BOM so Linux parsers can read it"
        );
    }

    #[cfg(unix)]
    #[test]
    fn install_sh_parses() {
        let script = concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/install.sh");
        let st = std::process::Command::new("sh")
            .args(["-n", script])
            .status()
            .expect("sh");
        assert!(st.success(), "sh -n scripts/install.sh");
    }
}
