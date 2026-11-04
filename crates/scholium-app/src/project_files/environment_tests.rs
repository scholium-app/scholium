use super::*;
use scholium_latex::toolchain::{BundleStatus, Issue, ProgramStatus, TexStatus, ToolchainStatus};

fn issue(id: &'static str, action: &'static str) -> Issue {
    Issue {
        message: Message {
            id,
            arguments: Default::default(),
        },
        action: Message {
            id: action,
            arguments: Default::default(),
        },
    }
}
fn status() -> ToolchainStatus {
    ToolchainStatus {
        rtex: BundleStatus::default(),
        development_override: false,
        tex_live: TexStatus {
            found: true,
            version: Some("This is LuaHBTeX, Version 1.24.0 (TeX Live 2026/Debian)".into()),
            kpsewhich: None,
        },
        engines: ["lualatex", "xelatex", "pdflatex", "biber", "bibtex"]
            .into_iter()
            .map(|name| ProgramStatus {
                name,
                path: Some("/bin/tool".into()),
                version: Some("version".into()),
            })
            .collect(),
        font_cache_warm: true,
        issues: vec![],
    }
}
#[test]
fn distribution_summary_extracts_name_and_year_without_the_program_banner() {
    for (banner, expected) in [
        (
            "This is LuaHBTeX, Version 1.24.0 (TeX Live 2026/Debian)",
            "TeX Live 2026",
        ),
        ("XeTeX 3.14 (MacTeX 2025)", "MacTeX 2025"),
        ("MiKTeX-pdfTeX 4.21 (MiKTeX 26.1)", "MiKTeX"),
        ("kpathsea version 6.4.2", "TeX"),
    ] {
        assert_eq!(distribution(banner), expected);
    }
    let rows = summary(&status(), i18n::Locale::ZhHans, false);
    assert_eq!(
        rows[0].text,
        "TeX Live 2026 · LuaLaTeX、XeLaTeX、pdfLaTeX、biber、bibtex"
    );
    assert!(!rows[0].warning);
}
#[test]
fn environment_groups_duplicate_issues_and_keeps_one_action_per_item() {
    let mut status = status();
    status.tex_live.found = false;
    status.issues = vec![
        issue("toolchain.tex-missing", "toolchain.install-tex"),
        issue("toolchain.tex-missing", "toolchain.install-tex"),
        issue(
            "toolchain.program-unavailable",
            "toolchain.install-tex-component",
        ),
        issue("toolchain.bundle-missing", "toolchain.reinstall"),
        issue("toolchain.bundle-invalid", "toolchain.reinstall"),
    ];
    let rows = summary(&status, i18n::Locale::En, false);
    assert_eq!(rows.len(), 3);
    assert!(rows[0].warning && rows[1].warning);
    assert!(rows.iter().all(|r| r.text.matches(" — ").count() <= 1));
    assert!(!rows[0].text.contains("LuaLaTeX"));
    assert!(!rows[1].text.contains("—:"));
    status.rtex.valid = true;
    let rows = summary(&status, i18n::Locale::En, false);
    assert!(!rows[1].warning);
    assert!(!rows[1].text.contains("Reinstall"));
}
#[test]
fn release_summary_never_exposes_development_overrides_and_debug_prefers_invalid() {
    let mut status = status();
    status.development_override = true;
    status.issues = vec![
        issue("toolchain.dev-override", "toolchain.remove-dev-override"),
        issue("toolchain.override-invalid", "toolchain.fix-dev-override"),
        issue("toolchain.override-invalid", "toolchain.fix-dev-override"),
    ];
    for locale in i18n::Locale::ALL {
        let release = summary(&status, locale, false);
        let debug = summary(&status, locale, true);
        assert_eq!(release.len(), 3);
        assert!(
            !release
                .iter()
                .any(|r| r.text.contains(&message(locale, &status.issues[1].message)))
        );
        assert_eq!(debug.len(), 3);
        assert_eq!(debug[1].text.matches(" — ").count(), 1);
        assert!(
            debug[1]
                .text
                .contains(&message(locale, &status.issues[1].message))
        );
        assert!(
            !debug[1]
                .text
                .contains(&message(locale, &status.issues[0].message))
        );
    }
}
#[test]
fn all_toolchain_messages_are_localized() {
    for locale in i18n::Locale::ALL {
        for key in super::super::EXTRA_CATALOG_KEYS
            .iter()
            .filter(|k| k.starts_with("toolchain."))
        {
            assert!(i18n::defines(locale, key), "{key}");
        }
    }
}

#[test]
fn failed_detection_does_not_invent_missing_installations() {
    let mut status = status();
    status
        .issues
        .push(issue("toolchain.detection-failed", "toolchain.refresh"));
    let rows = summary(&status, i18n::Locale::En, false);
    assert_eq!(rows.len(), 1);
    assert!(rows[0].warning);
    assert!(rows[0].text.contains("Refresh"));
}
#[test]
fn failed_version_probes_are_not_listed_as_usable_engines() {
    let mut status = status();
    status.engines[0].version = None;
    let rows = summary(&status, i18n::Locale::En, false);
    assert!(rows[0].warning);
    assert!(!rows[0].text.contains("LuaLaTeX"));
    assert!(rows[0].text.contains("XeLaTeX"));
    assert_eq!(rows[0].text.matches(" — ").count(), 1);
}
