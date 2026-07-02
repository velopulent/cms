//! Content sniffing for uploads: compare the declared MIME type against the
//! file's magic bytes so a payload can't claim to be something it isn't
//! (e.g. an executable declared as `image/png`).
//!
//! Only types with reliable signatures are checked; signature-less formats
//! (plain text, CSV, SVG, ...) and declared types we have no table entry for
//! are skipped rather than rejected.

/// How many leading bytes the sniffer needs. Callers should buffer up to this
/// many bytes (or the whole body, if smaller) before checking.
pub const SNIFF_LEN: usize = 8192;

/// Result of a sniff check.
#[derive(Debug, PartialEq, Eq)]
pub enum Sniff {
    /// Detected type is compatible with the declared type.
    Match,
    /// Detected type contradicts the declared type (detected type inside, if any).
    Mismatch(Option<&'static str>),
    /// Declared type has no reliable signature — nothing to verify.
    Skip,
}

/// MIME types accepted as a detection result for each checkable declared type.
/// Groups exist because containers overlap (mp4/quicktime brands, ogg family,
/// webm-is-matroska, OOXML-is-zip, legacy Office CFB).
fn accepted_detections(declared: &str) -> Option<&'static [&'static str]> {
    const OOXML_OR_ZIP: &[&str] = &[
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "application/zip",
    ];
    const LEGACY_OFFICE: &[&str] = &[
        "application/msword",
        "application/vnd.ms-excel",
        "application/vnd.ms-powerpoint",
        "application/x-ole-storage",
    ];
    const OGG: &[&str] = &["application/ogg", "audio/ogg", "video/ogg"];
    const MP4_FAMILY: &[&str] = &["video/mp4", "video/quicktime", "video/x-m4v"];
    const MATROSKA: &[&str] = &["video/webm", "video/x-matroska", "audio/webm"];

    Some(match declared {
        "image/jpeg" | "image/jpg" => &["image/jpeg"],
        "image/png" => &["image/png"],
        "image/gif" => &["image/gif"],
        "image/webp" => &["image/webp"],
        "image/avif" => &["image/avif", "image/heif", "image/heic"],
        "image/tiff" => &["image/tiff"],
        "image/bmp" => &["image/bmp"],
        "video/mp4" | "video/quicktime" => MP4_FAMILY,
        "video/webm" | "audio/webm" => MATROSKA,
        "video/ogg" | "audio/ogg" => OGG,
        "video/x-msvideo" => &["video/x-msvideo"],
        "audio/mpeg" => &["audio/mpeg", "audio/mp3"],
        "audio/wav" => &["audio/wav", "audio/x-wav"],
        "audio/aac" => &["audio/aac"],
        "audio/flac" => &["audio/flac", "audio/x-flac"],
        "application/pdf" => &["application/pdf"],
        "application/msword" | "application/vnd.ms-excel" | "application/vnd.ms-powerpoint" => LEGACY_OFFICE,
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        | "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        | "application/vnd.openxmlformats-officedocument.presentationml.presentation"
        | "application/zip" => OOXML_OR_ZIP,
        "application/gzip" => &["application/gzip"],
        "application/x-tar" => &["application/x-tar"],
        "application/x-7z-compressed" => &["application/x-7z-compressed"],
        "application/x-rar-compressed" => &["application/vnd.rar", "application/x-rar-compressed"],
        // Signature-less or unknown declared types: nothing to verify.
        _ => return None,
    })
}

/// Check `prefix` (the first bytes of the payload, up to [`SNIFF_LEN`]) against
/// the declared MIME type.
pub fn check(declared_mime: &str, prefix: &[u8]) -> Sniff {
    let Some(accepted) = accepted_detections(declared_mime) else {
        return Sniff::Skip;
    };

    match infer::get(prefix) {
        Some(kind) if accepted.contains(&kind.mime_type()) => Sniff::Match,
        Some(kind) => Sniff::Mismatch(Some(kind.mime_type())),
        None => Sniff::Mismatch(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_signature_matches() {
        let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0];
        assert_eq!(check("image/png", &png), Sniff::Match);
    }

    #[test]
    fn jpeg_signature_matches() {
        let jpeg = [0xFF, 0xD8, 0xFF, 0xE0, 0, 0, 0, 0];
        assert_eq!(check("image/jpeg", &jpeg), Sniff::Match);
    }

    #[test]
    fn pdf_signature_matches() {
        assert_eq!(check("application/pdf", b"%PDF-1.7 rest of file"), Sniff::Match);
    }

    #[test]
    fn text_declared_as_png_is_mismatch() {
        assert_eq!(check("image/png", b"definitely not a png"), Sniff::Mismatch(None));
    }

    #[test]
    fn png_declared_as_jpeg_is_mismatch() {
        let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0];
        assert_eq!(check("image/jpeg", &png), Sniff::Mismatch(Some("image/png")));
    }

    #[test]
    fn signature_less_types_are_skipped() {
        assert_eq!(check("text/plain", b"hello"), Sniff::Skip);
        assert_eq!(check("text/csv", b"a,b,c"), Sniff::Skip);
        assert_eq!(check("image/svg+xml", b"<svg/>"), Sniff::Skip);
        assert_eq!(check("application/octet-stream", &[0, 1, 2]), Sniff::Skip);
    }

    #[test]
    fn ooxml_zip_family_is_interchangeable() {
        // Minimal ZIP local-file-header signature.
        let zip = [b'P', b'K', 0x03, 0x04, 0, 0, 0, 0];
        assert_eq!(
            check(
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                &zip
            ),
            Sniff::Match
        );
        assert_eq!(check("application/zip", &zip), Sniff::Match);
    }
}
