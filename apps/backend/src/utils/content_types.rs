pub const IMAGE_TYPES: &[&str] = &[
    "image/jpeg",
    "image/png",
    "image/gif",
    "image/webp",
    "image/avif",
    "image/svg+xml",
    "image/tiff",
    "image/bmp",
];

pub const VIDEO_TYPES: &[&str] = &[
    "video/mp4",
    "video/webm",
    "video/ogg",
    "video/quicktime",
    "video/x-msvideo",
];

pub const AUDIO_TYPES: &[&str] = &[
    "audio/mpeg",
    "audio/wav",
    "audio/ogg",
    "audio/webm",
    "audio/aac",
    "audio/flac",
];

pub const DOCUMENT_TYPES: &[&str] = &[
    "application/pdf",
    "application/msword",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.ms-excel",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/vnd.ms-powerpoint",
    "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    "text/plain",
    "text/csv",
    "text/html",
    "text/markdown",
];

pub const ARCHIVE_TYPES: &[&str] = &[
    "application/zip",
    "application/gzip",
    "application/x-tar",
    "application/x-7z-compressed",
    "application/x-rar-compressed",
];

pub fn all_allowed() -> Vec<&'static str> {
    IMAGE_TYPES
        .iter()
        .chain(VIDEO_TYPES)
        .chain(AUDIO_TYPES)
        .chain(DOCUMENT_TYPES)
        .chain(ARCHIVE_TYPES)
        .copied()
        .collect()
}

pub const CATEGORIES: &[&str] = &["image", "video", "audio", "document", "archive"];

pub fn types_for_category(category: &str) -> &'static [&'static str] {
    match category {
        "image" => IMAGE_TYPES,
        "video" => VIDEO_TYPES,
        "audio" => AUDIO_TYPES,
        "document" => DOCUMENT_TYPES,
        "archive" => ARCHIVE_TYPES,
        _ => &[],
    }
}

pub fn is_allowed(mime: &str) -> bool {
    IMAGE_TYPES.contains(&mime)
        || VIDEO_TYPES.contains(&mime)
        || AUDIO_TYPES.contains(&mime)
        || DOCUMENT_TYPES.contains(&mime)
        || ARCHIVE_TYPES.contains(&mime)
}

pub fn category_of(mime: &str) -> &'static str {
    if IMAGE_TYPES.contains(&mime) {
        "image"
    } else if VIDEO_TYPES.contains(&mime) {
        "video"
    } else if AUDIO_TYPES.contains(&mime) {
        "audio"
    } else if DOCUMENT_TYPES.contains(&mime) {
        "document"
    } else if ARCHIVE_TYPES.contains(&mime) {
        "archive"
    } else {
        "other"
    }
}

pub fn is_file_type_category(category: &str) -> bool {
    CATEGORIES.contains(&category)
}

pub fn filter_for_category(category: &str) -> Vec<String> {
    types_for_category(category).iter().map(|s| s.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_allowed_detects_common_types() {
        assert!(is_allowed("image/jpeg"));
        assert!(is_allowed("video/mp4"));
        assert!(is_allowed("audio/mpeg"));
        assert!(is_allowed("application/pdf"));
        assert!(is_allowed("application/zip"));
        assert!(!is_allowed("application/x-doom"));
    }

    #[test]
    fn category_of_maps_correctly() {
        assert_eq!(category_of("image/png"), "image");
        assert_eq!(category_of("video/webm"), "video");
        assert_eq!(category_of("audio/wav"), "audio");
        assert_eq!(category_of("application/pdf"), "document");
        assert_eq!(category_of("application/zip"), "archive");
        assert_eq!(category_of("application/x-doom"), "other");
    }

    #[test]
    fn types_for_category_returns_correct() {
        assert_eq!(types_for_category("image"), IMAGE_TYPES);
        assert_eq!(types_for_category("video"), VIDEO_TYPES);
        assert!(types_for_category("unknown").is_empty());
    }

    #[test]
    fn is_file_type_category_works() {
        assert!(is_file_type_category("image"));
        assert!(is_file_type_category("video"));
        assert!(is_file_type_category("audio"));
        assert!(is_file_type_category("document"));
        assert!(is_file_type_category("archive"));
        assert!(!is_file_type_category("text"));
    }
}
