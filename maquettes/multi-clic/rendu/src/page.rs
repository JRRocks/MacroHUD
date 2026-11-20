//! `apercu.html`: the page model `apercu.modele.html` with every `{{name}}` replaced by the image
//! `name@2x.png` of the preview folder as a data URI, and `{{name.l}}` / `{{name.h}}` by its width
//! and height in CSS pixels (half its pixels). The page holds its images and opens on its own.

use std::fs;
use std::path::Path;

/// The « Aujourd'hui » images: renders of the current HUD by MacroHUD's own test.
const TODAY: [(&str, &str); 2] = [("vue-a@2x.png", "aujourdhui-1-reglages"), ("vue-b@2x.png", "aujourdhui-2-active")];

pub fn write(folder: &Path) {
    let renders = folder.join("..").join("rendu-egui");
    for (source, name) in TODAY {
        let from = renders.join(source);
        fs::copy(&from, folder.join(format!("{name}@2x.png"))).unwrap_or_else(|e| panic!("{}: {e}", from.display()));
    }
    let model = Path::new(env!("CARGO_MANIFEST_DIR")).join("apercu.modele.html");
    let model = fs::read_to_string(&model).unwrap_or_else(|e| panic!("{}: {e}", model.display()));
    let mut page = String::with_capacity(model.len() * 8);
    let mut rest = model.as_str();
    while let Some(start) = rest.find("{{") {
        page.push_str(&rest[..start]);
        let end = rest[start..].find("}}").expect("« }} » manquant") + start;
        page.push_str(&fill(folder, &rest[start + 2..end]));
        rest = &rest[end + 2..];
    }
    page.push_str(rest);
    let page = french_spaces(&page);
    assert!(!page.contains("<script"), "la page reste sans script");
    let path = folder.join("apercu.html");
    fs::write(&path, &page).expect("écriture de la page");
    println!("{} ({} Kio)", path.display(), page.len() / 1024);
}

/// French spacing in the text of the page (after its `<style>`): no line break inside « … » nor
/// before « : », « ; », « ? », « ! ».
fn french_spaces(page: &str) -> String {
    let (head, body) = page.split_at(page.find("</style>").expect("« </style> » dans le modèle"));
    let mut body = body.to_owned();
    for (from, to) in [
        ("« ", "«\u{a0}"),
        (" »", "\u{a0}»"),
        (" :", "\u{a0}:"),
        (" ;", "\u{a0};"),
        (" ?", "\u{a0}?"),
        (" !", "\u{a0}!"),
    ] {
        body = body.replace(from, to);
    }
    format!("{head}{body}")
}

/// The value of one `{{…}}`.
fn fill(folder: &Path, key: &str) -> String {
    let (name, part) = key.split_once('.').map_or((key, None), |(name, part)| (name, Some(part)));
    let path = folder.join(format!("{name}@2x.png"));
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let size = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().expect("en-tête PNG")) / 2;
    match part {
        None => format!("data:image/png;base64,{}", base64(&bytes)),
        Some("l") => size(16).to_string(),
        Some("h") => size(20).to_string(),
        Some(other) => panic!("{{{{{key}}}}} : « .{other} » inconnu"),
    }
}

/// Standard base64, with padding.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (k, &b)| n | u32::from(b) << (16 - 8 * k));
        for k in 0..4 {
            if k <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * k) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn base64_matches_the_standard() {
        assert_eq!(super::base64(b""), "");
        assert_eq!(super::base64(b"f"), "Zg==");
        assert_eq!(super::base64(b"fo"), "Zm8=");
        assert_eq!(super::base64(b"foo"), "Zm9v");
        assert_eq!(super::base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(super::base64(&[0xff, 0xfe, 0xfd]), "//79");
    }
}
