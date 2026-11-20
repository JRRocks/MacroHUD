//! The two pages of `maquettes/multi-clic/`, from their models (`*.modele.html`): `apercu.html`
//! (P2.2) and `resultat.html` (P2.3). In a model, `{{name}}` becomes the image `name@2x.png` as a
//! data URI, `name` being a path from `maquettes/multi-clic/` (`a-1-reglages`,
//! `../rendu-egui/multi-1-reglages`); `{{name.l}}` and `{{name.h}}` its width and height in CSS
//! pixels (half its pixels); `{{diff a b}}` what differs between two such images, in words. The
//! pages hold their images and open on their own.

use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;

/// The « Aujourd'hui » images of the preview: renders of the HUD by MacroHUD's own test.
const TODAY: [(&str, &str); 2] = [("vue-a@2x.png", "aujourdhui-1-reglages"), ("vue-b@2x.png", "aujourdhui-2-active")];

/// Copies the « Aujourd'hui » images next to the preview.
pub fn copy_today(folder: &Path) {
    let renders = folder.join("..").join("rendu-egui");
    for (source, name) in TODAY {
        let from = renders.join(source);
        fs::copy(&from, folder.join(format!("{name}@2x.png"))).unwrap_or_else(|e| panic!("{}: {e}", from.display()));
    }
}

/// Writes the page `page` of `folder` from the model `model` of this program.
pub fn write(folder: &Path, model: &str, page: &str) {
    let model = Path::new(env!("CARGO_MANIFEST_DIR")).join(model);
    let model = fs::read_to_string(&model).unwrap_or_else(|e| panic!("{}: {e}", model.display()));
    let mut out = String::with_capacity(model.len() * 8);
    let mut rest = model.as_str();
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let end = rest[start..].find("}}").expect("« }} » manquant") + start;
        out.push_str(&fill(folder, &rest[start + 2..end]));
        rest = &rest[end + 2..];
    }
    out.push_str(rest);
    let out = french_spaces(&out);
    assert!(!out.contains("<script"), "la page reste sans script");
    let path = folder.join(page);
    fs::write(&path, &out).expect("écriture de la page");
    println!("{} ({} Kio)", path.display(), out.len() / 1024);
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
    if let Some(pair) = key.strip_prefix("diff ") {
        let (a, b) = pair.split_once(' ').expect("{{diff a b}}");
        return difference(folder, a, b);
    }
    let (name, part) = match (key.strip_suffix(".l"), key.strip_suffix(".h")) {
        (Some(name), _) => (name, Some(16)),
        (_, Some(name)) => (name, Some(20)),
        _ => (key, None),
    };
    let path = folder.join(format!("{name}@2x.png"));
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    match part {
        // Width (bytes 16 to 19 of a PNG) or height (20 to 23), halved.
        Some(at) => (u32::from_be_bytes(bytes[at..at + 4].try_into().expect("en-tête PNG")) / 2).to_string(),
        None => format!("data:image/png;base64,{}", base64(&bytes)),
    }
}

/// An 8-bit RGBA PNG: width, height, pixels.
fn read_rgba(path: &Path) -> (u32, u32, Vec<u8>) {
    let file = BufReader::new(File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())));
    let mut reader = png::Decoder::new(file).read_info().expect("PNG lisible");
    let mut buffer = vec![0; reader.output_buffer_size().expect("taille du PNG")];
    let info = reader.next_frame(&mut buffer).expect("image PNG");
    assert_eq!(info.color_type, png::ColorType::Rgba, "{}", path.display());
    buffer.truncate(info.buffer_size());
    (info.width, info.height, buffer)
}

/// What differs between the images `a` and `b`, in words: their sizes, else how many pixels and by
/// how much at most (0–255 per channel).
fn difference(folder: &Path, a: &str, b: &str) -> String {
    let (wa, ha, pa) = read_rgba(&folder.join(format!("{a}@2x.png")));
    let (wb, hb, pb) = read_rgba(&folder.join(format!("{b}@2x.png")));
    if (wa, ha) != (wb, hb) {
        return format!("tailles différentes : {} × {} et {} × {} pixels", wa / 2, ha / 2, wb / 2, hb / 2);
    }
    let (mut count, mut most) = (0u32, 0u8);
    for (x, y) in pa.chunks_exact(4).zip(pb.chunks_exact(4)) {
        let gap = x.iter().zip(y).map(|(u, v)| u.abs_diff(*v)).max().unwrap_or(0);
        if gap > 0 {
            count += 1;
            most = most.max(gap);
        }
    }
    let total = group_thousands(wa * ha);
    match (count, most) {
        (0, _) => format!("aucun, les {total} pixels sont identiques"),
        (_, 1) => format!(
            "aucun à l'œil ; {} pixels sur {total} diffèrent d'un seul niveau sur 255 (arrondis du dessin du texte)",
            group_thousands(count)
        ),
        _ => format!("{} pixels sur {total} diffèrent, jusqu'à {most} niveaux sur 255", group_thousands(count)),
    }
}

/// « 418 600 »: thousands apart, French style (narrow no-break space).
fn group_thousands(n: u32) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (k, c) in digits.chars().enumerate() {
        if k > 0 && (digits.len() - k).is_multiple_of(3) {
            out.push('\u{202f}');
        }
        out.push(c);
    }
    out
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

    #[test]
    fn thousands_stand_apart() {
        assert_eq!(super::group_thousands(45), "45");
        assert_eq!(super::group_thousands(418_600), "418\u{202f}600");
        assert_eq!(super::group_thousands(1_000_000), "1\u{202f}000\u{202f}000");
    }
}
