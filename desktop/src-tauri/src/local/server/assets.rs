//! Static files of the embedded judge UI (see `build.rs`) and the small
//! HTML pages the server renders itself.

include!(concat!(env!("OUT_DIR"), "/judge_assets.rs"));

/// Served under `/judge/` when the frontend judge build was not available
/// at compile time.
pub const PLACEHOLDER_INDEX: &str = r#"<!doctype html>
<html lang="de"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Strafrichter – KI-DO Timer</title>
<style>body{font-family:system-ui,sans-serif;margin:0;padding:3rem 1.5rem;text-align:center;color:#201c16;background:#fbfaf6}h1{font-size:1.4rem}</style>
</head><body><h1>Strafrichter-App nicht enthalten</h1>
<p>Diese Desktop-Version wurde ohne die Strafrichter-Oberfläche gebaut
(<code>npm run build:judge</code> im Ordner <code>frontend/</code> fehlte).</p>
<p>Bitte eine offizielle Version der KiDo-Timer Desktop-App installieren.</p>
</body></html>
"#;

/// Target of the judge app's redirect on `401` (rotated / wrong access
/// code): explain what to do instead of a 404.
pub const LOGIN_PAGE: &str = r#"<!doctype html>
<html lang="de"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Zugangscode ungültig – KI-DO Timer</title>
<style>body{font-family:system-ui,sans-serif;margin:0;padding:3rem 1.5rem;text-align:center;color:#201c16;background:#fbfaf6}h1{font-size:1.4rem}</style>
</head><body><h1>Zugangscode ungültig</h1>
<p>Der Zugangscode wurde am Wettkampf-Laptop erneuert oder ist falsch.</p>
<p>Bitte den <strong>aktuellen QR-Code</strong> am Wettkampf-Laptop neu scannen.</p>
</body></html>
"#;

/// A file to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
    /// Content-hashed bundle file → cache forever; everything else no-cache.
    pub immutable: bool,
}

pub fn content_type(path: &str) -> &'static str {
    let ext = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("html") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("json") | Some("webmanifest") => "application/json",
        Some("woff2") => "font/woff2",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn lookup_in<'a>(assets: &'a [(&'a str, &'a [u8])], path: &str) -> Option<&'a [u8]> {
    assets.iter().find(|(p, _)| *p == path).map(|(_, b)| *b)
}

/// Escape a string for a JS string literal inside an inline `<script>`.
fn js_string(s: &str) -> String {
    serde_json::to_string(s)
        .unwrap_or_else(|_| "\"\"".into())
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
}

/// Inject the runtime config (API base = same origin, owner sub) before
/// `</head>` (or at the start if there is none).
pub fn inject_config(index_html: &str, owner_sub: Option<&str>) -> String {
    let mut script = String::from("<script>window.__KIDO_API_BASE__=\"\";");
    if let Some(owner) = owner_sub {
        script.push_str(&format!("window.__KIDO_OWNER_SUB__={};", js_string(owner)));
    }
    script.push_str("</script>");
    match index_html.find("</head>") {
        Some(i) => format!("{}{}{}", &index_html[..i], script, &index_html[i..]),
        None => format!("{script}{index_html}"),
    }
}

/// Resolve `rel` (path below `/judge/`, without leading slash) against
/// `assets`. Unknown paths without a file extension fall back to the SPA
/// index; unknown files are `404`.
pub fn resolve(assets: &[(&str, &[u8])], built: bool, rel: &str, owner_sub: Option<&str>) -> Asset {
    let rel = rel.trim_start_matches('/');
    let index = || {
        let html = if built {
            lookup_in(assets, "index.html")
                .map(|b| String::from_utf8_lossy(b).into_owned())
                .unwrap_or_else(|| PLACEHOLDER_INDEX.to_string())
        } else {
            PLACEHOLDER_INDEX.to_string()
        };
        Asset {
            status: 200,
            content_type: "text/html; charset=utf-8",
            body: inject_config(&html, owner_sub).into_bytes(),
            immutable: false,
        }
    };
    if rel.is_empty() || rel == "index.html" || rel.contains("..") {
        return index();
    }
    if let Some(bytes) = lookup_in(assets, rel) {
        return Asset {
            status: 200,
            content_type: content_type(rel),
            body: bytes.to_vec(),
            immutable: rel.starts_with("assets/"),
        };
    }
    let last = rel.rsplit('/').next().unwrap_or(rel);
    if last.contains('.') {
        return Asset {
            status: 404,
            content_type: "text/plain; charset=utf-8",
            body: b"Nicht gefunden".to_vec(),
            immutable: false,
        };
    }
    index()
}

/// [`resolve`] against the embedded build.
pub fn judge_asset(rel: &str, owner_sub: Option<&str>) -> Asset {
    resolve(JUDGE_ASSETS, JUDGE_UI_BUILT, rel, owner_sub)
}

#[cfg(test)]
mod tests {
    use super::*;

    const INDEX: &[u8] = b"<html><head><title>x</title></head><body></body></html>";
    const JS: &[u8] = b"console.log(1)";
    const ASSETS: &[(&str, &[u8])] = &[
        ("index.html", INDEX),
        ("assets/index-abc.js", JS),
        ("favicon.svg", b"<svg/>"),
    ];

    #[test]
    fn index_gets_runtime_config() {
        let a = resolve(ASSETS, true, "", Some("sub-1"));
        assert_eq!(a.status, 200);
        let html = String::from_utf8(a.body).unwrap();
        assert!(html.contains("window.__KIDO_API_BASE__=\"\";"));
        assert!(html.contains("window.__KIDO_OWNER_SUB__=\"sub-1\";"));
        assert!(html.find("<script>").unwrap() < html.find("</head>").unwrap());
        assert!(!a.immutable);
    }

    #[test]
    fn spa_fallback_for_paths_without_extension() {
        let a = resolve(ASSETS, true, "some/route", None);
        assert_eq!(a.status, 200);
        assert_eq!(a.content_type, "text/html; charset=utf-8");
        assert!(!String::from_utf8(a.body)
            .unwrap()
            .contains("__KIDO_OWNER_SUB__"));
    }

    #[test]
    fn bundle_files_are_immutable() {
        let a = resolve(ASSETS, true, "assets/index-abc.js", None);
        assert_eq!(a.body, JS);
        assert!(a.immutable);
        assert_eq!(a.content_type, "text/javascript; charset=utf-8");
        let f = resolve(ASSETS, true, "favicon.svg", None);
        assert_eq!(f.content_type, "image/svg+xml");
        assert!(!f.immutable);
    }

    #[test]
    fn missing_file_is_404() {
        assert_eq!(resolve(ASSETS, true, "assets/missing.js", None).status, 404);
    }

    #[test]
    fn traversal_is_never_a_file() {
        let a = resolve(ASSETS, true, "../Cargo.toml", None);
        assert_eq!(a.content_type, "text/html; charset=utf-8");
    }

    #[test]
    fn placeholder_when_not_built() {
        let a = resolve(&[], false, "", Some("s"));
        let html = String::from_utf8(a.body).unwrap();
        assert!(html.contains("nicht enthalten"));
        assert!(html.contains("__KIDO_OWNER_SUB__"));
    }

    #[test]
    fn owner_is_escaped_in_script() {
        let html = inject_config("<head></head>", Some("</script><b>\""));
        assert!(!html.contains("</script><b>"));
        assert!(html.contains("\\u003c/script\\u003e"));
        assert_eq!(
            inject_config("<p>", None),
            "<script>window.__KIDO_API_BASE__=\"\";</script><p>"
        );
    }

    #[test]
    fn content_types() {
        assert_eq!(content_type("a.CSS"), "text/css; charset=utf-8");
        assert_eq!(content_type("x.woff2"), "font/woff2");
        assert_eq!(content_type("noext"), "application/octet-stream");
    }

    #[test]
    fn embedded_build_is_consistent() {
        if JUDGE_UI_BUILT {
            assert!(JUDGE_ASSETS.iter().any(|(p, _)| *p == "index.html"));
            let a = judge_asset("", Some("sub"));
            assert!(String::from_utf8(a.body)
                .unwrap()
                .contains("__KIDO_API_BASE__"));
        } else {
            assert!(String::from_utf8(judge_asset("", None).body)
                .unwrap()
                .contains("nicht enthalten"));
        }
        assert!(LOGIN_PAGE.contains("QR-Code"));
    }
}
