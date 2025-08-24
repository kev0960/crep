use std::{cmp::min, collections::HashMap};

use aho_corasick::AhoCorasick;
use anyhow::Result;

pub struct Utf8FileChecker {
    marks: AhoCorasick,
    ext_to_magic_numbers: HashMap<&'static str, &'static [Signature]>,
}

static BYTE_ORDER_MARKS: &[&[u8]] =
    &[&[0xEF, 0xBB, 0xBF], &[0xFE, 0xFF], &[0xFF, 0xFE]];

const NUM_BYTES_TO_CHECK: usize = 1024 * 8;

impl Utf8FileChecker {
    pub fn new() -> Result<Self> {
        let marks = AhoCorasick::builder().build(BYTE_ORDER_MARKS)?;

        Ok(Self {
            marks,
            ext_to_magic_numbers: HashMap::from_iter(
                EXT_MAGIC.iter().map(|(k, v)| (*k, *v)),
            ),
        })
    }

    pub fn is_utf8_document(&self, content: &[u8], file_ext: &str) -> bool {
        if self.check_magic(content, file_ext) {
            return true;
        }

        self.marks
            .find(&content[0..min(NUM_BYTES_TO_CHECK, content.len())])
            .is_none()
    }

    fn check_magic(&self, content: &[u8], file_ext: &str) -> bool {
        if let Some(signatures) = self.ext_to_magic_numbers.get(file_ext) {
            for signature in *signatures {
                if content.len() < signature.bytes.len() + signature.offset {
                    continue;
                }

                if &content
                    [signature.offset..signature.offset + signature.bytes.len()]
                    == signature.bytes
                {
                    return true;
                }
            }
        }

        false
    }
}

#[derive(Debug)]
struct Signature {
    pub offset: usize,
    pub bytes: &'static [u8],
}

static EXT_MAGIC: &[(&str, &[Signature])] = &[
    (
        "png",
        &[Signature {
            offset: 0,
            bytes: b"\x89PNG\r\n\x1A\n",
        }],
    ),
    (
        "jpg",
        &[Signature {
            offset: 0,
            bytes: b"\xFF\xD8\xFF",
        }],
    ),
    (
        "jpeg",
        &[Signature {
            offset: 0,
            bytes: b"\xFF\xD8\xFF",
        }],
    ),
    (
        "gif",
        &[
            Signature {
                offset: 0,
                bytes: b"GIF87a",
            },
            Signature {
                offset: 0,
                bytes: b"GIF89a",
            },
        ],
    ),
    (
        "bmp",
        &[Signature {
            offset: 0,
            bytes: b"BM",
        }],
    ),
    (
        "tif",
        &[
            Signature {
                offset: 0,
                bytes: b"II*\x00",
            },
            Signature {
                offset: 0,
                bytes: b"MM\x00*",
            },
        ],
    ),
    (
        "tiff",
        &[
            Signature {
                offset: 0,
                bytes: b"II*\x00",
            },
            Signature {
                offset: 0,
                bytes: b"MM\x00*",
            },
        ],
    ),
    (
        "ico",
        &[Signature {
            offset: 0,
            bytes: b"\x00\x00\x01\x00",
        }],
    ),
    (
        "webp",
        &[
            Signature {
                offset: 0,
                bytes: b"RIFF",
            },
            Signature {
                offset: 8,
                bytes: b"WEBP",
            }, // needs both
        ],
    ),
    (
        "elf",
        &[Signature {
            offset: 0,
            bytes: b"\x7FELF",
        }],
    ),
    (
        "so",
        &[Signature {
            offset: 0,
            bytes: b"\x7FELF",
        }],
    ),
    (
        "o",
        &[Signature {
            offset: 0,
            bytes: b"\x7FELF",
        }],
    ),
    (
        "exe",
        &[Signature {
            offset: 0,
            bytes: b"MZ",
        }],
    ),
    (
        "dll",
        &[Signature {
            offset: 0,
            bytes: b"MZ",
        }],
    ),
    (
        "sys",
        &[Signature {
            offset: 0,
            bytes: b"MZ",
        }],
    ),
    (
        "zip",
        &[
            Signature {
                offset: 0,
                bytes: b"\x50\x4B\x03\x04",
            },
            Signature {
                offset: 0,
                bytes: b"\x50\x4B\x05\x06",
            },
            Signature {
                offset: 0,
                bytes: b"\x50\x4B\x07\x08",
            },
        ],
    ),
    (
        "jar",
        &[Signature {
            offset: 0,
            bytes: b"\x50\x4B\x03\x04",
        }],
    ),
    (
        "apk",
        &[Signature {
            offset: 0,
            bytes: b"\x50\x4B\x03\x04",
        }],
    ),
    (
        "docx",
        &[Signature {
            offset: 0,
            bytes: b"\x50\x4B\x03\x04",
        }],
    ),
    (
        "xlsx",
        &[Signature {
            offset: 0,
            bytes: b"\x50\x4B\x03\x04",
        }],
    ),
    (
        "pptx",
        &[Signature {
            offset: 0,
            bytes: b"\x50\x4B\x03\x04",
        }],
    ),
    (
        "gz",
        &[Signature {
            offset: 0,
            bytes: b"\x1F\x8B",
        }],
    ),
    (
        "bz2",
        &[Signature {
            offset: 0,
            bytes: b"BZh",
        }],
    ),
    (
        "7z",
        &[Signature {
            offset: 0,
            bytes: b"\x37\x7A\xBC\xAF\x27\x1C",
        }],
    ),
    (
        "rar",
        &[
            Signature {
                offset: 0,
                bytes: b"\x52\x61\x72\x21\x1A\x07\x00",
            },
            Signature {
                offset: 0,
                bytes: b"\x52\x61\x72\x21\x1A\x07\x01\x00",
            },
        ],
    ),
    (
        "xz",
        &[Signature {
            offset: 0,
            bytes: b"\xFD\x37\x7A\x58\x5A\x00",
        }],
    ),
    (
        "wav",
        &[
            Signature {
                offset: 0,
                bytes: b"RIFF",
            },
            Signature {
                offset: 8,
                bytes: b"WAVE",
            },
        ],
    ),
    (
        "avi",
        &[
            Signature {
                offset: 0,
                bytes: b"RIFF",
            },
            Signature {
                offset: 8,
                bytes: b"AVI ",
            },
        ],
    ),
    (
        "ogg",
        &[Signature {
            offset: 0,
            bytes: b"OggS",
        }],
    ),
    (
        "opus",
        &[Signature {
            offset: 0,
            bytes: b"OggS",
        }],
    ),
    (
        "ogv",
        &[Signature {
            offset: 0,
            bytes: b"OggS",
        }],
    ),
    (
        "mp3",
        &[
            Signature {
                offset: 0,
                bytes: b"ID3",
            },
            Signature {
                offset: 0,
                bytes: b"\xFF\xFB",
            },
            Signature {
                offset: 0,
                bytes: b"\xFF\xF3",
            },
            Signature {
                offset: 0,
                bytes: b"\xFF\xF2",
            },
        ],
    ),
    (
        "mp4",
        &[Signature {
            offset: 4,
            bytes: b"ftyp",
        }],
    ),
    (
        "mov",
        &[Signature {
            offset: 4,
            bytes: b"ftyp",
        }],
    ),
    (
        "3gp",
        &[Signature {
            offset: 4,
            bytes: b"ftyp",
        }],
    ),
    (
        "3g2",
        &[Signature {
            offset: 4,
            bytes: b"ftyp",
        }],
    ),
    (
        "m4a",
        &[Signature {
            offset: 4,
            bytes: b"ftyp",
        }],
    ),
    (
        "mkv",
        &[Signature {
            offset: 0,
            bytes: b"\x1A\x45\xDF\xA3",
        }],
    ),
    (
        "webm",
        &[Signature {
            offset: 0,
            bytes: b"\x1A\x45\xDF\xA3",
        }],
    ),
    (
        "pdf",
        &[Signature {
            offset: 0,
            bytes: b"%PDF",
        }],
    ),
    (
        "ps",
        &[Signature {
            offset: 0,
            bytes: b"%!PS",
        }],
    ),
    (
        "rtf",
        &[Signature {
            offset: 0,
            bytes: br"{\rtf",
        }],
    ),
    (
        "doc",
        &[Signature {
            offset: 0,
            bytes: b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1",
        }],
    ),
    (
        "xls",
        &[Signature {
            offset: 0,
            bytes: b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1",
        }],
    ),
    (
        "ppt",
        &[Signature {
            offset: 0,
            bytes: b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1",
        }],
    ),
    // ==== Code / Data ====
    (
        "class",
        &[Signature {
            offset: 0,
            bytes: b"\xCA\xFE\xBA\xBE",
        }],
    ),
    (
        "wasm",
        &[Signature {
            offset: 0,
            bytes: b"\x00asm",
        }],
    ),
];
