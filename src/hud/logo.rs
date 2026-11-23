//! The logo of MacroHUD (D-33, `maquettes/logo/`) as the icon of the window: taskbar and Alt+Tab.
//!
//! `icone.ico` is `maquettes/logo/MacroHUD.ico` as validated: ten sizes from 16 to 256 px, 32 bits
//! with alpha, the 256 px one in PNG. `build.rs` also makes it the icon of `macrohud.exe`.

use eframe::egui::IconData;

/// The icon file, in the executable.
const ICO: &[u8] = include_bytes!("icone.ico");

/// Size of the window's icon (D-50): Windows' large icon at 200 % (taskbar, Alt+Tab); at smaller
/// scales Windows shrinks it. The image of that size is the one drawn for it in `icone.ico`.
const WINDOW_ICON_SIZE: u32 = 64;

/// The window's icon: the 64 px image of the logo.
pub fn window_icon() -> IconData {
    image(ICO, WINDOW_ICON_SIZE).unwrap_or_default()
}

/// One image of an ICO directory.
#[derive(Clone, Copy, Debug)]
struct Entry {
    size: u32,
    bits: u16,
    offset: usize,
    length: usize,
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// The images of an ICO file (type 1), as its directory lists them.
fn entries(ico: &[u8]) -> Option<Vec<Entry>> {
    if u16_at(ico, 0)? != 0 || u16_at(ico, 2)? != 1 {
        return None;
    }
    let count = usize::from(u16_at(ico, 4)?);
    (0..count)
        .map(|k| {
            let at = 6 + 16 * k;
            // 0 stands for 256 px.
            let size = match *ico.get(at)? {
                0 => 256,
                n => u32::from(n),
            };
            let bits = u16_at(ico, at + 6)?;
            let length = usize::try_from(u32_at(ico, at + 8)?).ok()?;
            let offset = usize::try_from(u32_at(ico, at + 12)?).ok()?;
            Some(Entry { size, bits, offset, length })
        })
        .collect()
}

/// The square image of `size` px from an ICO file, as straight RGBA.
fn image(ico: &[u8], size: u32) -> Option<IconData> {
    // Only 32-bit images (alpha) are read: those of the logo.
    let entry = entries(ico)?.into_iter().find(|e| e.size == size && e.bits == 32)?;
    let data = ico.get(entry.offset..entry.offset.checked_add(entry.length)?)?;
    if data.starts_with(b"\x89PNG") {
        return eframe::icon_data::from_png_bytes(data).ok();
    }
    bitmap(data, size)
}

/// An icon bitmap: BITMAPINFOHEADER, 32-bit BGRA rows from the bottom up, then the 1-bit AND mask
/// (it counts only if every alpha is 0, as Windows does).
fn bitmap(data: &[u8], size: u32) -> Option<IconData> {
    let header = usize::try_from(u32_at(data, 0)?).ok()?;
    let (width, height) = (u32_at(data, 4)?, u32_at(data, 8)?);
    let (bits, compression) = (u16_at(data, 14)?, u32_at(data, 16)?);
    // The height counts the colours and the mask.
    if width != size || height != 2 * size || bits != 32 || compression != 0 {
        return None;
    }
    let side = usize::try_from(size).ok()?;
    let pixels = data.get(header..header + side * side * 4)?;
    let mask_stride = side.div_ceil(32) * 4;
    let mask = data.get(header + side * side * 4..)?;
    let use_mask = pixels.chunks_exact(4).all(|p| p[3] == 0);
    let mut rgba = Vec::with_capacity(side * side * 4);
    for y in 0..side {
        let row = side - 1 - y;
        for x in 0..side {
            let p = &pixels[(row * side + x) * 4..][..4];
            let alpha = if use_mask {
                let byte = *mask.get(row * mask_stride + x / 8)?;
                if byte & (0x80 >> (x % 8)) == 0 { 255 } else { 0 }
            } else {
                p[3]
            };
            rgba.extend_from_slice(&[p[2], p[1], p[0], alpha]);
        }
    }
    Some(IconData { rgba, width: size, height: size })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(icon: &IconData, x: u32, y: u32) -> [u8; 4] {
        let at = ((y * icon.width + x) * 4) as usize;
        icon.rgba[at..at + 4].try_into().expect("pixel")
    }

    /// `icone.ico` is the validated logo: ten sizes, 32 bits, the largest in PNG.
    #[test]
    fn the_icon_file_holds_the_ten_sizes_of_the_logo() {
        let entries = entries(ICO).expect("répertoire ICO");
        let sizes: Vec<u32> = entries.iter().map(|e| e.size).collect();
        assert_eq!(sizes, [256, 128, 96, 64, 48, 40, 32, 24, 20, 16]);
        assert!(entries.iter().all(|e| e.bits == 32));
        assert!(ICO[entries[0].offset..].starts_with(b"\x89PNG"), "256 px en PNG");
        // Not redrawn: the very file of the validated logo.
        assert!(ICO == include_bytes!("../../maquettes/logo/MacroHUD.ico"), "copie exacte de maquettes/logo");
    }

    /// The window's icon: 64 px, transparent corners, the dark square, the white « M » and the
    /// green dot of the logo.
    #[test]
    fn the_window_icon_is_the_logo() {
        let icon = window_icon();
        assert_eq!((icon.width, icon.height, icon.rgba.len()), (64, 64, 64 * 64 * 4));
        assert_eq!(pixel(&icon, 0, 0)[3], 0, "coin transparent");
        assert_eq!(pixel(&icon, 63, 63)[3], 0, "coin transparent");
        let [r, g, b, a] = pixel(&icon, 32, 8);
        assert!(a == 255 && r < 40 && g < 40 && b < 40, "fond sombre : {r} {g} {b} {a}");
        let [r, g, b, a] = pixel(&icon, 16, 20);
        assert!(a == 255 && r > 220 && g > 220 && b > 220, "« M » blanc : {r} {g} {b} {a}");
        let [r, g, b, a] = pixel(&icon, 22, 51);
        assert!(a == 255 && g > 150 && r < 120 && b < 160, "point vert : {r} {g} {b} {a}");
    }

    /// The PNG image (256 px) and the bitmaps decode alike: same logo at every size.
    #[test]
    fn every_size_decodes() {
        for size in [256, 128, 96, 64, 48, 40, 32, 24, 20, 16] {
            let icon = image(ICO, size).unwrap_or_else(|| panic!("{size} px"));
            assert_eq!((icon.width, icon.height), (size, size));
            assert_eq!(pixel(&icon, 0, 0)[3], 0, "{size} px : coin transparent");
            assert_eq!(pixel(&icon, size / 2, size / 2)[3], 255, "{size} px : centre opaque");
        }
        assert!(image(ICO, 33).is_none());
        assert!(image(b"pas une icone", 64).is_none());
    }
}
