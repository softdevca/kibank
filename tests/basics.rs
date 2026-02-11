use std::io::Cursor;

use kibank::Metadata;
use kibank::write::BankWriter;

/// Must not be able to add to a bank once it has been written.
#[test]
fn add_after_write() {
    let mut out = Vec::with_capacity(512);
    let mut writer = BankWriter::new(Cursor::new(&mut out));
    writer.write().unwrap();
    let result = writer.add_metadata(&Metadata::default());
    assert!(result.is_err());
}
