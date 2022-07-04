use kibank::read::BankReader;

/// Read metadata
#[test]
fn read() {
    let mut reader = BankReader::open("tests/metadata.bank").unwrap();
    let items = reader.items();
    let item = items.first().unwrap();
    let metadata = reader.read_metadata(item).unwrap();
    assert_eq!(metadata.author, "Author");
    assert_eq!(metadata.name, "Title");
    assert_eq!(metadata.description, "Description");
    assert_eq!(metadata.id, "author.title");
}
