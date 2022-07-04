//! Support for [Kilohearts](https://kilohearts.com) banks.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt::Debug;
use std::mem::size_of;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod read;
pub mod write;

/// First bytes that identify the kind of the file.
const FILE_ID: &[u8] = &[137_u8, b'k', b'H', b's'];

/// Every bank contains these characters so it seems logical they identify
/// something about the format.
const FORMAT_VERSION: &[u8] = "Bank0001".as_bytes();

/// First part the background image file name without the trailing dot.
pub const BACKGROUND_FILE_STEM: &str = "background";

/// These bytes are written as part of the header to check to detect incorrect
/// end of line format conversion. The same sequence of bytes is used by the
/// [PNG format](https://en.wikipedia.org/wiki/Portable_Network_Graphics#File_header).
const CORRUPTION_CHECK_BYTES: &[u8] = &[0x0d, 0x0a, 0x1a, 0x0a];

/// A file in the bank has a directory then they are separated with this
/// character. This may be different from the separator used by the operating
/// system.
pub const PATH_SEPARATOR: char = '/';

/// Types of files supported in banks, in the order they appear in the bank.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ItemKind {
    Background,
    Metadata,
    Sample,
    MultipassPreset,
    PhasePlantPreset,
    SnapHeapPreset,
    ThreeBandEq,
    Bitcrush,
    CarveEq,
    Chorus,
    CombFilter,
    Compressor,
    Convolver,
    Delay,
    Disperser,
    Distortion,
    Dynamics,
    Ensemble,
    Faturator,
    Filter,
    Flanger,
    FormatFilter,
    FrequencyShifter,
    Gain,
    Gate,
    Haas,
    LadderFilter,
    Limiter,
    NonlinearFilter,
    PhaseDistortion,
    Phaser,
    PitchShifter,
    Resonator,
    Reverb,
    Reverser,
    RingMod,
    SliceEq,
    Stereo,
    TapeStop,
    TranceGate,
    TransientShaper,
}

impl ItemKind {
    /// Name of the directory that contains files of this type inside the bank.
    #[must_use]
    pub fn directory(&self) -> Option<&'static str> {
        match self {
            ItemKind::Background | ItemKind::Metadata => None,
            ItemKind::Sample => Some("samples"),
            kind => kind.extensions().first().copied(),
        }
    }

    /// File name extensions that are used for the type of files, without the
    /// leading dot.
    #[must_use]
    pub fn extensions(&self) -> Vec<&'static str> {
        match self {
            Self::Background => vec!["jpg", "png"],
            Self::Metadata => vec!["json"],
            Self::Sample => vec!["flac", "mp3", "wav"],
            Self::MultipassPreset => vec!["multipass"],
            Self::PhasePlantPreset => vec!["phaseplant"],
            Self::SnapHeapPreset => vec!["snapheap"],
            Self::ThreeBandEq => vec!["ksqe"],
            Self::Bitcrush => vec!["ksbc"],
            Self::CarveEq => vec!["ksge"],
            Self::Chorus => vec!["ksch"],
            Self::CombFilter => vec!["kscf"],
            Self::Compressor => vec!["kscp"],
            Self::Convolver => vec!["ksco"],
            Self::Delay => vec!["ksdl"],
            Self::Disperser => vec!["kdsp"],
            Self::Distortion => vec!["ksdt"],
            Self::Dynamics => vec!["ksot"],
            Self::Ensemble => vec!["ksun"],
            Self::Faturator => vec!["kfat"],
            Self::Filter => vec!["ksfi"],
            Self::Flanger => vec!["ksfl"],
            Self::FormatFilter => vec!["ksvf"],
            Self::FrequencyShifter => vec!["ksfs"],
            Self::Gain => vec!["ksgn"],
            Self::Gate => vec!["ksgt"],
            Self::Haas => vec!["ksha"],
            Self::LadderFilter => vec!["ksla"],
            Self::Limiter => vec!["kslt"],
            Self::NonlinearFilter => vec!["ksdf"],
            Self::PhaseDistortion => vec!["kspd"],
            Self::Phaser => vec!["ksph"],
            Self::PitchShifter => vec!["ksps"],
            Self::Resonator => vec!["ksre"],
            Self::Reverb => vec!["ksrv"],
            Self::Reverser => vec!["ksrr"],
            Self::RingMod => vec!["ksrm"],
            Self::SliceEq => vec!["kpeq"],
            Self::Stereo => vec!["ksst"],
            Self::TapeStop => vec!["ksts"],
            Self::TranceGate => vec!["kstg"],
            Self::TransientShaper => vec!["kstr"],
        }
    }

    /// Returns `true` if the given extension is used by this type of item. Case-insensitive and without a leading dot.
    #[must_use]
    pub fn has_extension(&self, extension: &OsStr) -> bool {
        let extension = extension.to_string_lossy();
        self.extensions()
            .iter()
            .any(|ext| ext.eq_ignore_ascii_case(&extension))
    }

    /// Every supported item kind
    #[must_use]
    pub const fn all() -> [ItemKind; 41] {
        [
            Self::Background,
            Self::Metadata,
            Self::MultipassPreset,
            Self::PhasePlantPreset,
            Self::SnapHeapPreset,
            Self::Sample,
            Self::ThreeBandEq,
            Self::Bitcrush,
            Self::CarveEq,
            Self::Chorus,
            Self::CombFilter,
            Self::Compressor,
            Self::Convolver,
            Self::Delay,
            Self::Disperser,
            Self::Distortion,
            Self::Dynamics,
            Self::Ensemble,
            Self::Faturator,
            Self::Filter,
            Self::Flanger,
            Self::FormatFilter,
            Self::FrequencyShifter,
            Self::Gain,
            Self::Gate,
            Self::Haas,
            Self::LadderFilter,
            Self::Limiter,
            Self::NonlinearFilter,
            Self::PhaseDistortion,
            Self::Phaser,
            Self::PitchShifter,
            Self::Resonator,
            Self::Reverb,
            Self::Reverser,
            Self::RingMod,
            Self::SliceEq,
            Self::Stereo,
            Self::TapeStop,
            Self::TranceGate,
            Self::TransientShaper,
        ]
    }

    /// Find the kind of a file from the file name extension. The background
    /// and metadata also require a specific file name.
    #[must_use]
    pub fn from<P: AsRef<Path>>(path: P) -> Option<ItemKind> {
        // Assumes the well-known file names and file extensions are ASCII.
        let file_name = path.as_ref().file_name();
        if file_name
            .unwrap_or_default()
            .eq_ignore_ascii_case(Metadata::FILE_NAME)
        {
            return Some(ItemKind::Metadata);
        } else if file_name
            .unwrap_or_default()
            .eq_ignore_ascii_case(BACKGROUND_FILE_STEM)
        {
            return Some(ItemKind::Background);
        }

        // Match file name extension to see if it should be included in the bank.
        path.as_ref().extension().and_then(|extension| {
            ItemKind::all().into_iter().find(|kind| {
                kind.extensions()
                    .iter()
                    .any(|ext| ext.eq_ignore_ascii_case(&extension.to_string_lossy()))
            })
        })
    }
}

/// The metadata stored in the bank may be `Some("")` or `None` when no value has
/// been set.
///
/// Some fields have only been found in Kilohearts factory content banks and not
/// in those created with Kilohearts Bank Maker.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Metadata {
    /// Only found in Kilohearts factory content banks.
    pub version: Option<u32>,

    /// A unique identifier for the bank, typically of the form "author.name"
    pub id: String,

    pub name: String,
    pub author: String,
    pub description: String,

    /// A 160-bit hash as a hex string. Only found in Kilohearts factory content banks.
    /// The hash of a bank appears to be the same no matter who downloaded it or with
    /// which version of the application.
    pub hash: Option<String>,

    /// Values found in the JSON but not part of the model.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl Metadata {
    /// Name of the file inside and outside of the bank that contains the metadata.
    pub const FILE_NAME: &'static str = "index.json";

    /// Bank IDs are lowercase and alphanumeric, plus a dot used as a separator.
    #[must_use]
    pub fn sanitize_id(str: &str) -> String {
        str.chars()
            .filter_map(|c| {
                if c.is_alphanumeric() || c == '.' {
                    Some(c.to_ascii_lowercase())
                } else {
                    None
                }
            })
            .collect::<String>()
    }
}

#[derive(Clone, Debug)]
struct Location {
    /// From start of file name block
    file_name_offset: u64,

    /// From the start of the file
    data_offset: u64,

    data_size: u64,
}

impl Location {
    /// Number of bytes used to store the structure on disk.
    const BLOCK_SIZE: usize = size_of::<u64>() * 3;

    pub fn data_end(&self) -> u64 {
        self.data_offset + self.data_size
    }
}
