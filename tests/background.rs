use std::ffi::OsStr;
use std::io::Cursor;

use kibank::read::BankReader;
use kibank::write::BankWriter;
use kibank::{ItemKind, BACKGROUND_FILE_STEM};

#[test]
fn png() {
    let mut out = Vec::with_capacity(512);

    let mut writer = BankWriter::new(Cursor::new(&mut out));
    let file_name = format!("{BACKGROUND_FILE_STEM}.png");
    writer
        .add_file(
            ItemKind::Background,
            OsStr::new(&file_name),
            "tests/images/background.png",
        )
        .unwrap();
    writer.write().unwrap();

    // Verify
    let reader = BankReader::new(Cursor::new(out)).unwrap();
    let items = reader.items();
    let item = items.first().unwrap();
    assert!(item.is_background_file());
    assert_eq!(item.path_bytes, file_name.as_bytes());
}
