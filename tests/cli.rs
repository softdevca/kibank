//! Test the command line interface.

use std::process::Command;

use assert_cmd::cargo;
use assert_cmd::prelude::*;
use predicates::prelude::*;

#[test]
fn file_doesnt_exist() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!());
    cmd.arg("extract").arg("test/file/doesnt/exist.bank");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("No such file"));
    Ok(())
}

#[test]
fn create_and_list() -> Result<(), Box<dyn std::error::Error>> {
    let file = assert_fs::NamedTempFile::new("create_and_list.bank")?;

    // Create the bank. At least one file is required so add a sample image.
    let mut cmd = Command::new(cargo::cargo_bin!());
    cmd.arg("create")
        .arg(file.path())
        .arg("tests/images/background.jpg");
    cmd.assert().success();

    // List the contents.
    let mut cmd = Command::new(cargo::cargo_bin!());
    cmd.arg("list").arg(file.path());
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("index.json"))
        .stdout(predicate::str::contains("background.jpg"));

    Ok(())
}
