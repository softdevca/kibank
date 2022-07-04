//! Test banks that are legal but do not have any contents. See `empty` for testing
//! various forms of invalid incomplete banks.

use std::io;
use std::io::{BufRead, Cursor, Read, Seek};

use kibank::read::BankReader;
use kibank::write::BankWriter;

fn create_blank() -> io::Result<Cursor<Vec<u8>>> {
    let mut out = Vec::with_capacity(512);
    let mut writer = BankWriter::new(Cursor::new(&mut out));
    writer.write()?;
    Ok(Cursor::new(out))
}

fn verify_blank<ReaderType: Read + Seek + BufRead>(reader: &mut BankReader<ReaderType>) {
    // First and only item is the metadata
    let items = reader.items();
    let metadata_item = items.first().unwrap();
    assert!(metadata_item.is_metadata_file());

    let metadata = reader.read_metadata(metadata_item).unwrap();
    assert!(metadata.id.is_empty());
    assert!(metadata.author.is_empty());
    assert!(metadata.name.is_empty());
    assert!(metadata.description.is_empty());
    assert!(metadata.hash.unwrap_or_default().is_empty());
}

/// Create then reload a blank bank.
#[test]
fn create_and_load_blank() {
    let blank_file = create_blank().unwrap();
    let mut reader = BankReader::new(blank_file).unwrap();
    verify_blank(&mut reader);
}

/// Load an empty back created with Kilohearts Bank Maker on 2022-05-01 on a Windows 11 machine.
#[test]
fn load_blank_from_file() {
    let mut reader = BankReader::open("tests/blank.bank").unwrap();
    verify_blank(&mut reader);
}
