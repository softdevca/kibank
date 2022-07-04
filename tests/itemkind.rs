use std::ffi::OsStr;

use kibank::ItemKind;

/// Must not be able to add to a bank once it has been written.
#[test]
fn extensions() {
    assert_eq!(ItemKind::Metadata.extensions(), vec!["json"]);

    // All kinds must have at least one extension.
    assert!(ItemKind::all()
        .iter()
        .all(|kind| !kind.extensions().is_empty()));
}

#[test]
fn has_extension() {
    assert!(ItemKind::Metadata.has_extension(OsStr::new("json")));
    assert!(ItemKind::Metadata.has_extension(OsStr::new("JSON")));
    assert!(!ItemKind::Metadata.has_extension(OsStr::new("txt")));
}
