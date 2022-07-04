use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::io::{Error, ErrorKind, Write};
use std::mem::size_of;
use std::path::Path;

use byteorder::{LittleEndian, WriteBytesExt};
use log::debug;

use crate::{
    ItemKind, Location, Metadata, CORRUPTION_CHECK_BYTES, FILE_ID, FORMAT_VERSION, PATH_SEPARATOR,
};

pub struct Item {
    kind: ItemKind,
    contents: Vec<u8>,

    /// Path of the file within the bank, including any leading directory.
    path_os: OsString,
}

impl Item {
    #[must_use]
    pub fn file_name_bytes(&self) -> Vec<u8> {
        self.path_os.to_string_lossy().as_bytes().to_owned()
    }
}

pub struct BankWriter<WriterType: Write> {
    inner: WriterType,
    items: Vec<Item>,

    /// If the data has already been committed with a call to `write()`.
    written: bool,
}

impl<WriterType: Write> BankWriter<WriterType> {
    pub fn new(inner: WriterType) -> BankWriter<WriterType> {
        BankWriter {
            inner,
            items: Vec::new(),
            written: false,
        }
    }

    /// Adding an item with empty contents results in the item being treated as a directory
    /// instead of a file. It is a limitation of the format that there is no way to have
    /// zero-length contents.
    ///
    /// * `kind` - type of the file
    /// * `file_name` - name of the file within the bank, without any leading directory
    /// * `contents` - the data to include in the bank
    ///
    /// # Errors
    ///
    /// Will return `Err` if the bank has already been written
    pub fn add(&mut self, kind: ItemKind, file_name: &OsStr, contents: Vec<u8>) -> io::Result<()> {
        if self.written {
            return Err(Error::new(
                ErrorKind::Other,
                "Cannot add to a bank that has already been written",
            ));
        }

        // Add the leading directory so the item is ready to use.
        let file_name = if let Some(dir_name) = kind.directory() {
            let mut path_str = OsString::from(dir_name);
            path_str.push(PATH_SEPARATOR.to_string());
            path_str.push(file_name);
            path_str
        } else {
            file_name.to_owned()
        };

        self.items.push(Item {
            kind,
            contents,
            path_os: file_name,
        });
        Ok(())
    }

    /// * `kind` - type of the file
    /// * `file_name` - name of the file within the bank
    /// * `data_path` - location of the file that contains the data to include
    ///
    /// # Errors
    ///
    /// Will return `Err` if the bank has already been written
    pub fn add_file<P: AsRef<Path>>(
        &mut self,
        kind: ItemKind,
        file_name: &OsStr,
        data_path: P,
    ) -> io::Result<()> {
        let contents = fs::read(data_path)?;
        self.add(kind, file_name, contents)
    }

    /// A default ID will be created if one is not provided.
    ///
    /// # Errors
    ///
    /// Will return `Err` if the bank has already been written
    pub fn add_metadata(&mut self, metadata: &Metadata) -> io::Result<()> {
        // Create the ID from the author and name if there isn't one.
        let contents = if metadata.id.is_empty() {
            let mut id_parts = Vec::with_capacity(2);
            let author_part = Metadata::sanitize_id(&metadata.author);
            let name_part = Metadata::sanitize_id(&metadata.name);
            if !author_part.is_empty() {
                id_parts.push(author_part);
            }
            if !name_part.is_empty() {
                id_parts.push(name_part);
            }
            let metadata = Metadata {
                version: metadata.version,
                id: id_parts.join("."),
                name: metadata.name.clone(),
                author: metadata.author.clone(),
                description: metadata.description.clone(),
                hash: metadata.hash.clone(),
                extra: metadata.extra.clone(),
            };

            // Pretty-print the JSON to match what Bank Maker does. Bank
            // Maker uses \n\r end of line on Windows and \n on Mac.
            serde_json::to_vec_pretty(&metadata)?
        } else {
            serde_json::to_vec_pretty(metadata)?
        };

        debug!(
            "Adding metadata contents: {}",
            String::from_utf8_lossy(&contents)
        );
        self.add(
            ItemKind::Metadata,
            OsStr::new(Metadata::FILE_NAME),
            contents,
        )
    }

    /// Commit the contents added to the bank. All bytes will be written to the
    /// underlying stream before returning.
    ///
    /// # Errors
    ///
    /// Will return `Err` if the bank has already been written
    pub fn write(&mut self) -> io::Result<()> {
        // The file is written in one pass, without seeking backwards, to allow
        // the possibility of streaming the output.
        if self.written {
            return Err(Error::new(
                ErrorKind::Other,
                "The bank has already been written",
            ));
        }

        // Include metadata if it hasn't been provided.
        if !self
            .items
            .iter()
            .any(|item| item.kind == ItemKind::Metadata)
        {
            debug!("Adding default metadata");
            self.add_metadata(&Metadata::default())?;
        }

        let kinds = self
            .items
            .iter()
            .map(|item| item.kind)
            .collect::<BTreeSet<ItemKind>>();
        debug!("Kinds of items in this bank are {:?}", kinds);

        // Header
        self.inner.write_all(FILE_ID)?;
        self.inner.write_all(CORRUPTION_CHECK_BYTES)?;
        self.inner.write_all(FORMAT_VERSION)?;

        // Number of files and directories added to the bank.
        let file_count = self.items.len();
        let directory_count = kinds.iter().filter_map(ItemKind::directory).count();
        let location_count = file_count + directory_count;
        self.inner
            .write_u64::<LittleEndian>(location_count as u64)?;
        debug!("Number of location is {location_count}");

        // Offsets
        let location_block_start =
            FILE_ID.len() + CORRUPTION_CHECK_BYTES.len() + FORMAT_VERSION.len() + size_of::<u64>();

        let file_name_block_length: usize = kinds
            .iter()
            .map(|kind| {
                // All the filenames and directory names for the kind.
                let dir_name_len = kind.directory().map_or(0, |dir| dir.as_bytes().len() + 1);

                let file_names_len = self
                    .items
                    .iter()
                    .map(|item| {
                        if item.kind == *kind {
                            item.file_name_bytes().len() + 1
                        } else {
                            0
                        }
                    })
                    .sum::<usize>();

                file_names_len + dir_name_len
            })
            .sum();

        let mut data_offset = (location_block_start
            + (location_count * Location::BLOCK_SIZE)
            + size_of::<u64>()
            + file_name_block_length) as u64;

        // Locations
        let mut file_name_block = Vec::new();
        for kind in &kinds {
            // Some kinds of items require a directory entry.
            if let Some(directory) = kind.directory() {
                debug!("Writing directory {directory}");
                self.inner
                    .write_u64::<LittleEndian>(file_name_block.len() as u64)?;
                file_name_block.extend_from_slice(directory.as_bytes());
                file_name_block.push(0_u8);

                self.inner.write_u64::<LittleEndian>(0)?; // Data offset
                self.inner.write_u64::<LittleEndian>(0)?; // Data size
            }

            for item in self.items.iter().filter(|item| item.kind == *kind) {
                self.inner
                    .write_u64::<LittleEndian>(file_name_block.len() as u64)?;
                file_name_block.extend(item.file_name_bytes());
                file_name_block.push(0_u8);

                let contents_len = item.contents.len() as u64;
                self.inner.write_u64::<LittleEndian>(data_offset)?;
                self.inner.write_u64::<LittleEndian>(contents_len)?;
                data_offset += contents_len;
            }
        }

        debug!("File name block length is {file_name_block_length}");
        self.inner
            .write_u64::<LittleEndian>(file_name_block_length as u64)?;
        self.inner.write_all(&file_name_block)?;

        // Write the contents of each item.
        for kind in kinds {
            for item in self.items.iter().filter(|item| kind == item.kind) {
                debug!(
                    "Writing item {} ({} bytes)",
                    item.path_os.to_string_lossy(),
                    item.contents.len()
                );
                self.inner.write_all(&item.contents)?;
            }
        }

        self.inner.flush()?;
        self.written = true;
        Ok(())
    }
}
