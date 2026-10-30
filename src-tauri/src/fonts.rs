//! Fonts beyond the bundled ones: the fonts installed on Windows, and font
//! files the user adds (copied into the app's data folder).
//!
//! The web view draws installed fonts by family name and added files through
//! the FontFace API, so this side only finds out what each font is called and
//! whether it covers Arabic letters and the timer's digits. For installed
//! fonts only the `name` and `cmap` tables are read, so listing a few hundred
//! fonts stays quick.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ttf_parser::{cmap, name, name_id, PlatformId, Tag};

/// Key of the added font files in the settings database.
pub const FILES_KEY: &str = "font_files";
/// Largest font file that can be added (big CJK fonts are around 20 MB).
pub const MAX_FILE_BYTES: usize = 30 * 1024 * 1024;
/// Bundled font used when nothing else is chosen (see js/font-list.js).
pub const DEFAULT_FONT: &str = "plex";
/// Timer font choice that follows the interface font.
pub const SAME_AS_UI: &str = "ui";

/// Letters a font needs to count as supporting Arabic.
const ARABIC_PROBE: &str = "ابتثجحخدذرزسشصضطظعغفقكلمنهوي";
/// What the timer face shows.
const DIGITS_PROBE: &str = "0123456789:";

/// A font family installed on the computer.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SystemFont {
    pub family: String,
    pub arabic: bool,
    pub digits: bool,
}

/// A font file the user added.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileFont {
    /// First 16 hex digits of the file's SHA-256: the same file is added once.
    pub id: String,
    pub family: String,
    /// Style within the family, e.g. "Bold" ("" for the regular style).
    pub style: String,
    /// "ttf" or "otf": the name of the copy is `<id>.<ext>`.
    pub ext: String,
    pub arabic: bool,
    pub digits: bool,
    /// Weights the file covers: one weight, or the range of a variable font.
    pub weight: (u16, u16),
    pub added_at: i64,
}

impl FileFont {
    pub fn file_name(&self) -> String {
        format!("{}.{}", self.id, self.ext)
    }
}

/// A stored font choice: a bundled font id, "system:<family>", "file:<id>",
/// or (for the timer only) "ui".
pub fn is_choice(value: &str, timer: bool) -> bool {
    if let Some(family) = value.strip_prefix("system:") {
        return is_family_name(family);
    }
    if let Some(id) = value.strip_prefix("file:") {
        return is_file_id(id);
    }
    if value == SAME_AS_UI {
        return timer;
    }
    !value.is_empty() && value.len() <= 32 && value.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Family names go into CSS (quoted, by the UI), so keep them to printable text.
fn is_family_name(s: &str) -> bool {
    let s_trim = s.trim();
    !s_trim.is_empty() && s.chars().count() <= 100 && !s.chars().any(|c| c.is_control())
}

pub fn is_file_id(s: &str) -> bool {
    s.len() == 16 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

// ------------------------------------------------------------------ reading

/// What a font is called and what it covers.
#[derive(Debug, Clone, PartialEq)]
struct FaceInfo {
    family: String,
    style: String,
    arabic: bool,
    digits: bool,
}

/// Names and coverage from a font's `name` and `cmap` tables. None for a font
/// without a usable name or without a Unicode character map (symbol fonts
/// such as Wingdings, which cannot show text).
fn face_info(names: Option<name::Table>, cmap: Option<cmap::Table>) -> Option<FaceInfo> {
    let names = names?;
    let family = best_name(&names, &[name_id::WWS_FAMILY, name_id::TYPOGRAPHIC_FAMILY, name_id::FAMILY])?;
    if family.starts_with(['@', '.']) || !is_family_name(&family) {
        return None;
    }
    let style = best_name(&names, &[name_id::WWS_SUBFAMILY, name_id::TYPOGRAPHIC_SUBFAMILY, name_id::SUBFAMILY])
        .unwrap_or_default();
    let cmap = cmap?;
    let unicode: Vec<cmap::Subtable> = cmap.subtables.into_iter().filter(|t| t.is_unicode()).collect();
    if unicode.is_empty() {
        return None;
    }
    let covers = |text: &str| {
        text.chars()
            .all(|c| unicode.iter().any(|t| t.glyph_index(c as u32).is_some_and(|g| g.0 != 0)))
    };
    Some(FaceInfo { family, style, arabic: covers(ARABIC_PROBE), digits: covers(DIGITS_PROBE) })
}

/// The first of `ids` the font names, preferring US English Windows names
/// (the ones Windows matches CSS family names against), then any English
/// name, then any readable one.
fn best_name(table: &name::Table, ids: &[u16]) -> Option<String> {
    for &id in ids {
        let mut best: Option<(u8, String)> = None;
        for record in table.names.into_iter().filter(|n| n.name_id == id) {
            let Some(text) = record.to_string() else { continue };
            let text = text.trim().to_string();
            if text.is_empty() {
                continue;
            }
            let rank = match (record.platform_id, record.language_id) {
                (PlatformId::Windows, 0x0409) => 0,
                (PlatformId::Windows, lang) if lang & 0xff == 0x09 => 1,
                (PlatformId::Unicode, _) => 2,
                _ => 3,
            };
            if best.as_ref().map_or(true, |(r, _)| rank < *r) {
                best = Some((rank, text));
            }
        }
        if let Some((_, text)) = best {
            return Some(text);
        }
    }
    None
}

/// Read only the `name` and `cmap` tables of every font in a file
/// (a collection such as a .ttc holds several).
fn read_tables(path: &Path) -> std::io::Result<Vec<(Vec<u8>, Vec<u8>)>> {
    let mut file = File::open(path)?;
    let mut head = [0u8; 12];
    file.read_exact(&mut head)?;
    let offsets: Vec<u64> = if &head[0..4] == b"ttcf" {
        let count = u32::from_be_bytes([head[8], head[9], head[10], head[11]]).min(64) as usize;
        let mut raw = vec![0u8; count * 4];
        file.read_exact(&mut raw)?;
        raw.chunks_exact(4).map(|c| u32::from_be_bytes([c[0], c[1], c[2], c[3]]) as u64).collect()
    } else {
        vec![0]
    };
    let mut faces = Vec::new();
    for offset in offsets {
        file.seek(SeekFrom::Start(offset))?;
        let mut header = [0u8; 12];
        file.read_exact(&mut header)?;
        if !matches!(&header[0..4], [0, 1, 0, 0] | b"OTTO" | b"true") {
            continue;
        }
        let count = u16::from_be_bytes([header[4], header[5]]).min(256) as usize;
        let mut records = vec![0u8; count * 16];
        file.read_exact(&mut records)?;
        let mut name_table = None;
        let mut cmap_table = None;
        for r in records.chunks_exact(16) {
            let at = u32::from_be_bytes([r[8], r[9], r[10], r[11]]) as u64;
            let len = u32::from_be_bytes([r[12], r[13], r[14], r[15]]) as usize;
            match &r[0..4] {
                b"name" if len <= 1 << 20 => name_table = Some((at, len)),
                b"cmap" if len <= 4 << 20 => cmap_table = Some((at, len)),
                _ => {}
            }
        }
        let (Some(n), Some(c)) = (name_table, cmap_table) else { continue };
        let mut read = |(at, len): (u64, usize)| -> std::io::Result<Vec<u8>> {
            let mut data = vec![0u8; len];
            file.seek(SeekFrom::Start(at))?;
            file.read_exact(&mut data)?;
            Ok(data)
        };
        faces.push((read(n)?, read(c)?));
    }
    Ok(faces)
}

/// Every font family in these folders (not their subfolders), by name.
/// Unreadable files are skipped. The styles of a family are merged: it
/// supports Arabic if any of its styles does.
pub fn scan(dirs: &[PathBuf]) -> Vec<SystemFont> {
    let mut fonts: Vec<SystemFont> = Vec::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            if !matches!(ext.as_str(), "ttf" | "otf" | "ttc" | "otc") {
                continue;
            }
            let Ok(faces) = read_tables(&path) else { continue };
            for (names, cmap) in faces {
                let Some(info) = face_info(name::Table::parse(&names), cmap::Table::parse(&cmap)) else { continue };
                match fonts.iter_mut().find(|f| f.family.eq_ignore_ascii_case(&info.family)) {
                    Some(f) => {
                        f.arabic |= info.arabic;
                        f.digits |= info.digits;
                    }
                    None => fonts.push(SystemFont { family: info.family, arabic: info.arabic, digits: info.digits }),
                }
            }
        }
    }
    fonts.sort_by_key(|f| f.family.to_lowercase());
    fonts
}

/// Where Windows keeps installed fonts: for everyone, and for this user only.
pub fn system_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if cfg!(windows) {
        let windows = std::env::var_os("WINDIR")
            .or_else(|| std::env::var_os("SystemRoot"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        dirs.push(windows.join("Fonts"));
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            dirs.push(PathBuf::from(local).join("Microsoft").join("Windows").join("Fonts"));
        }
    }
    dirs
}

/// Check a font file someone chose and describe it. Collections (.ttc) and
/// files that are not TrueType or OpenType fonts are refused.
pub fn inspect(data: &[u8], added_at: i64) -> Result<FileFont, &'static str> {
    if data.len() > MAX_FILE_BYTES {
        return Err("too_big");
    }
    let ext = match data.get(0..4) {
        Some(b"OTTO") => "otf",
        Some([0, 1, 0, 0]) | Some(b"true") => "ttf",
        _ => return Err("not_font"),
    };
    let face = ttf_parser::Face::parse(data, 0).map_err(|_| "not_font")?;
    let raw = face.raw_face();
    let info = face_info(
        raw.table(Tag::from_bytes(b"name")).and_then(name::Table::parse),
        raw.table(Tag::from_bytes(b"cmap")).and_then(cmap::Table::parse),
    )
    .ok_or("not_font")?;
    let weight = face
        .variation_axes()
        .into_iter()
        .find(|a| a.tag == Tag::from_bytes(b"wght"))
        .map(|a| (a.min_value.round().clamp(1.0, 1000.0) as u16, a.max_value.round().clamp(1.0, 1000.0) as u16))
        .unwrap_or_else(|| {
            let w = face.weight().to_number().clamp(1, 1000);
            (w, w)
        });
    let hash = Sha256::digest(data);
    let id: String = hash[..8].iter().map(|b| format!("{b:02x}")).collect();
    let style = if info.style.eq_ignore_ascii_case("regular") { String::new() } else { info.style };
    Ok(FileFont {
        id,
        family: info.family,
        style,
        ext: ext.to_string(),
        arabic: info.arabic,
        digits: info.digits,
        weight,
        added_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tiny fonts written by scripts/test-fonts.py (every glyph is a box).
    /// Arabic letters, digits and a few Latin letters.
    const ARABIC_TTF: &[u8] = include_bytes!("../tests/fonts/arabic-subset.ttf");
    /// Digits and Latin letters only.
    const LATIN_TTF: &[u8] = include_bytes!("../tests/fonts/latin-subset.ttf");
    /// A variable OpenType (CFF2) font with a 200-900 weight axis, Latin only.
    const VARIABLE_OTF: &[u8] = include_bytes!("../tests/fonts/variable-subset.otf");

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("awan-fonts-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_font_file_is_described() {
        let f = inspect(ARABIC_TTF, 7).unwrap();
        assert_eq!(f.family, "Awan Test Arabic");
        assert_eq!((f.style.as_str(), f.ext.as_str()), ("", "ttf"));
        assert!(f.arabic && f.digits);
        assert_eq!(f.weight, (400, 400));
        assert!(is_file_id(&f.id));
        assert_eq!(f.added_at, 7);
        assert_eq!(f.file_name(), format!("{}.ttf", f.id));
        // The same file always gets the same id.
        assert_eq!(inspect(ARABIC_TTF, 9).unwrap().id, f.id);
    }

    #[test]
    fn missing_arabic_letters_are_noticed() {
        let f = inspect(LATIN_TTF, 0).unwrap();
        assert!(!f.arabic);
        assert!(f.digits);
        assert_ne!(f.id, inspect(ARABIC_TTF, 0).unwrap().id);
    }

    #[test]
    fn a_variable_font_covers_its_weight_range() {
        let f = inspect(VARIABLE_OTF, 0).unwrap();
        assert_eq!(f.ext, "otf");
        assert_eq!(f.weight, (200, 900));
    }

    #[test]
    fn other_files_are_refused() {
        assert_eq!(inspect(b"not a font at all", 0), Err("not_font"));
        assert_eq!(inspect(b"", 0), Err("not_font"));
        let mut broken = ARABIC_TTF[..200].to_vec();
        broken.extend_from_slice(&[0; 64]);
        assert_eq!(inspect(&broken, 0), Err("not_font"));
        // A collection is refused: the web view loads single fonts only.
        let mut ttc = b"ttcf".to_vec();
        ttc.extend_from_slice(&ARABIC_TTF[4..]);
        assert_eq!(inspect(&ttc, 0), Err("not_font"));
        assert_eq!(inspect(&vec![0; MAX_FILE_BYTES + 1], 0), Err("too_big"));
    }

    #[test]
    fn installed_fonts_are_listed_by_family() {
        let dir = temp_dir("scan");
        std::fs::write(dir.join("arabic.ttf"), ARABIC_TTF).unwrap();
        std::fs::write(dir.join("ARABIC-COPY.TTF"), ARABIC_TTF).unwrap();
        std::fs::write(dir.join("latin.ttf"), LATIN_TTF).unwrap();
        std::fs::write(dir.join("variable.otf"), VARIABLE_OTF).unwrap();
        std::fs::write(dir.join("broken.ttf"), b"garbage").unwrap();
        std::fs::write(dir.join("notes.txt"), ARABIC_TTF).unwrap();
        let fonts = scan(&[dir.clone(), dir.join("missing")]);
        let names: Vec<&str> = fonts.iter().map(|f| f.family.as_str()).collect();
        assert_eq!(names, ["Awan Test Arabic", "Awan Test Latin", "Awan Test Variable"]);
        assert!(fonts[0].arabic && !fonts[1].arabic && !fonts[2].arabic);
        assert!(fonts.iter().all(|f| f.digits));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn collections_are_read_font_by_font() {
        // A .ttc holding the Arabic and the Latin font.
        let fonts = [ARABIC_TTF, LATIN_TTF];
        let mut ttc = b"ttcf\x00\x01\x00\x00".to_vec();
        ttc.extend_from_slice(&2u32.to_be_bytes());
        let mut body = Vec::new();
        let start = 12 + 8;
        for f in fonts {
            ttc.extend_from_slice(&((start + body.len()) as u32).to_be_bytes());
            body.extend_from_slice(&rebase(f, (start + body.len()) as u32));
        }
        ttc.extend_from_slice(&body);
        let dir = temp_dir("ttc");
        std::fs::write(dir.join("pair.ttc"), &ttc).unwrap();
        let names: Vec<String> = scan(std::slice::from_ref(&dir)).into_iter().map(|f| f.family).collect();
        assert_eq!(names, ["Awan Test Arabic", "Awan Test Latin"]);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A font's bytes with its table offsets moved by `by`, as inside a collection.
    fn rebase(font: &[u8], by: u32) -> Vec<u8> {
        let mut out = font.to_vec();
        let count = u16::from_be_bytes([font[4], font[5]]) as usize;
        for i in 0..count {
            let at = 12 + i * 16 + 8;
            let offset = u32::from_be_bytes([out[at], out[at + 1], out[at + 2], out[at + 3]]) + by;
            out[at..at + 4].copy_from_slice(&offset.to_be_bytes());
        }
        out
    }

    #[test]
    fn choices_are_checked() {
        for ok in ["plex", "cairo", "system:Segoe UI", "system:خط النسخ", "file:0123456789abcdef"] {
            assert!(is_choice(ok, false), "{ok}");
        }
        assert!(is_choice("ui", true));
        for bad in [
            "",
            "ui",
            "Plex",
            "system:",
            "system:   ",
            "system:a\u{0}b",
            "file:xyz",
            "file:0123456789ABCDEF",
            "font; color: red",
        ] {
            assert!(!is_choice(bad, false), "{bad:?}");
        }
        assert!(!is_choice(&format!("system:{}", "a".repeat(101)), false));
    }

    /// The real font folders of the Windows machine running the tests.
    #[cfg(windows)]
    #[test]
    fn the_windows_fonts_are_found() {
        let started = std::time::Instant::now();
        let fonts = scan(&system_dirs());
        println!("{} font families in {:?}", fonts.len(), started.elapsed());
        let find = |name: &str| fonts.iter().find(|f| f.family == name);
        let segoe = find("Segoe UI").expect("Segoe UI is listed");
        assert!(segoe.arabic && segoe.digits);
        assert!(find("Arial").is_some_and(|f| f.digits));
        // Symbol fonts cannot show text, so they are left out.
        assert!(find("Wingdings").is_none() && find("Marlett").is_none());
        // Styles are merged into their family ("Segoe UI Semibold" is "Segoe UI").
        assert!(find("Segoe UI Semibold").is_none());
    }

    #[test]
    fn windows_font_folders() {
        let dirs = system_dirs();
        if cfg!(windows) {
            assert!(dirs[0].ends_with("Fonts"));
        } else {
            assert!(dirs.is_empty());
        }
    }
}
