use std::borrow::Cow;
use std::fmt::Debug;
use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader, Error, ErrorKind, Read, Seek, SeekFrom};
use std::path::Path;

use byteorder::{LittleEndian, ReadBytesExt};
use log::{debug, trace};

use crate::{
    Location, Metadata, BACKGROUND_FILE_STEM, CORRUPTION_CHECK_BYTES, FILE_ID, FORMAT_VERSION,
};

#[derive(Clone, Debug)]
pub struct Item<'a> {
    /// The name of the file as it appears in the bank. The name is not a string because paths
    /// are not guaranteed to be valid UTF-8 on all platforms. Names are case-insensitive.
    /// Directories are separated by `BANK_PATH_SEPARATOR` regardless of platform.
    pub path_bytes: Cow<'a, [u8]>,

    location: Location,
}

/// Read a Kilohearts bank file.
impl<'a> Item<'a> {
    #[must_use]
    pub fn is_directory(&self) -> bool {
        self.location.data_size == 0
    }

    #[must_use]
    pub fn is_file(&self) -> bool {
        self.location.data_size != 0
    }

    #[must_use]
    pub fn is_background_file(&self) -> bool {
        self.is_file()
            && Path::new(&self.file_name_lossy())
                .file_stem()
                .unwrap_or_default()
                .eq_ignore_ascii_case(BACKGROUND_FILE_STEM)
    }

    #[must_use]
    pub fn is_metadata_file(&self) -> bool {
        self.is_file()
            && self
                .path_bytes
                .eq_ignore_ascii_case(Metadata::FILE_NAME.as_ref())
    }

    /// The file name converted to text. File names are not guaranteed to be valid UTF-8.
    #[must_use]
    pub fn file_name_lossy(&self) -> String {
        String::from_utf8_lossy(&self.path_bytes).to_string()
    }
}

pub struct BankReader<'a, ReaderType: Read + Seek + BufRead> {
    inner: ReaderType,
    items: Vec<Item<'a>>,
}

impl<'a, ReaderType: Read + Seek + BufRead> BankReader<'a, ReaderType> {
    /// # Errors
    ///
    /// Will return `Err` if the file is not a Kilohearts bank or if it is malformed.
    pub fn new(mut inner: ReaderType) -> io::Result<Self> {
        let mut file_id = [0_u8; FILE_ID.len()];
        inner.read_exact(&mut file_id)?;
        if file_id != FILE_ID {
            return Err(Error::new(ErrorKind::InvalidData, "Not a Kilohearts bank"));
        }

        let mut check_bytes = [0_u8; CORRUPTION_CHECK_BYTES.len()];
        inner.read_exact(&mut check_bytes)?;
        if check_bytes != CORRUPTION_CHECK_BYTES {
            let msg = format!("Unexpected check bytes {}", check_bytes.escape_ascii());
            return Err(Error::new(ErrorKind::InvalidData, msg));
        }

        let mut format_version = [0_u8; FORMAT_VERSION.len()];
        inner.read_exact(&mut format_version)?;
        if format_version != FORMAT_VERSION {
            let msg = format!(
                "Unexpected format version {}",
                format_version.escape_ascii()
            );
            return Err(Error::new(ErrorKind::InvalidData, msg));
        }

        let location_count = inner.read_u64::<LittleEndian>()?;
        trace!("Number of locations is {location_count}");

        let mut locations = Vec::with_capacity(location_count.min(1000) as u32 as usize);
        trace!("Location block start is {}", inner.stream_position()?);
        for _ in 0..location_count {
            let file_name_offset = inner.read_u64::<LittleEndian>()?;
            let data_offset = inner.read_u64::<LittleEndian>()?;
            let data_size = inner.read_u64::<LittleEndian>()?;
            trace!("File name offset is {file_name_offset}, data offset is {data_offset}, daa size is {data_size}");
            locations.push(Location {
                file_name_offset,
                data_offset,
                data_size,
            });
        }

        // File names
        let file_name_block_length = inner.read_u64::<LittleEndian>()?;
        let file_name_block_start = inner.stream_position()?;
        debug!("File name block length is {file_name_block_length} starting at {file_name_block_start}");

        let mut items = Vec::with_capacity(locations.len());
        for location in locations {
            let mut file_name_bytes = Vec::with_capacity(32);

            let file_name_pos = location.file_name_offset + file_name_block_start;
            inner.seek(SeekFrom::Start(file_name_pos))?;

            // This guarantees the file name will never contain a null.
            let read_count = inner.read_until(0_u8, &mut file_name_bytes)?;
            if read_count == 0 {
                return Err(Error::new(
                    ErrorKind::Other,
                    "Zero length read of file name at position {file_name_pos}",
                ));
            }

            // Ensure the file name is within bounds.
            if file_name_pos + read_count as u64 > file_name_block_start + file_name_block_length {
                return Err(Error::new(
                    ErrorKind::Other,
                    "Read past the end of the file name block",
                ));
            }

            // Remove the trailing null, which won't be there if we hit the end fo the file.
            if let Some(0_u8) = file_name_bytes.last() {
                file_name_bytes.pop();
            }

            debug!("File name {}", file_name_bytes.escape_ascii());
            items.push(Item {
                location,
                path_bytes: Cow::from(file_name_bytes),
            });
        }

        // Verify no ranges overlap. Besides being an indicator of a corrupt file, overlapping
        // data ranges can also be an amplification attack where many files can use the same
        // bytes in the file and consume all disk space.
        let mut file_items = items
            .iter()
            .filter(|item| (*item).is_file())
            .collect::<Vec<&Item>>();
        file_items.sort_by_key(|item| item.location.data_offset);
        for window in file_items.windows(2) {
            if window[0].location.data_end() > window[1].location.data_offset {
                let msg = format!(
                    "Bank item {} overlaps item {}",
                    String::from_utf8_lossy(&window[0].path_bytes),
                    String::from_utf8_lossy(&window[1].path_bytes)
                );
                return Err(Error::new(ErrorKind::Other, msg));
            }
        }

        Ok(BankReader { inner, items })
    }

    /// All of the items in the bank.
    pub fn items(&self) -> Vec<Item<'a>> {
        self.items.clone()
    }

    /// # Errors
    ///
    /// Will return `Err` on read or seek failure.
    pub fn read_contents(&mut self, item: &Item) -> io::Result<Vec<u8>> {
        // Accept the 32-bit limit on item contents for files on platforms with 32-bit pointers.
        #![allow(clippy::cast_possible_truncation)]
        let mut result = vec![0_u8; item.location.data_size as usize];
        self.inner
            .seek(SeekFrom::Start(item.location.data_offset))?;
        self.inner.read_exact(&mut result)?;
        Ok(result)
    }

    /// # Errors
    ///
    /// Will return `Err` if the item does not refer to metadata and on read or seek failure.
    pub fn read_metadata(&mut self, item: &Item) -> io::Result<Metadata> {
        if !item.is_metadata_file() {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Item does not contain a metadata file",
            ));
        }

        let data = self.read_contents(item)?;
        let metadata: Metadata = BankReader::parse_metadata(&data)?;
        Ok(metadata)
    }

    /// Write the contents of the item to a new file.
    ///
    /// # Errors
    ///
    /// Will return `Err` if the contents of the item cannot be read from the underlying stream.
    pub fn copy<P: AsRef<Path>>(&mut self, item: &Item, path: P) -> io::Result<()> {
        // Accept the 32-bit limit on item contents for files on platforms with 32-bit pointers.
        #![allow(clippy::cast_possible_truncation)]
        let mut result = vec![0_u8; item.location.data_size as usize];
        self.inner.read_exact(&mut result)?;
        std::fs::write(path, result)
    }
}

impl<'a> BankReader<'a, BufReader<File>> {
    /// # Errors
    ///
    /// Will return `Err` if the path cannot be opened as a file.
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let path_ref = path.as_ref();
        let file = File::open(path_ref)?;
        debug!("File {} opened", path_ref.display());
        let reader: BufReader<File> = BufReader::new(file);
        Self::new(reader)
    }

    /// # Errors
    ///
    /// Will return `Err` if the bytes cannot be parsed as JSON.
    pub fn parse_metadata(json: &[u8]) -> io::Result<Metadata> {
        serde_json::from_slice(json).map_err(Into::into)
    }
}
