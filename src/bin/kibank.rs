use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf, MAIN_SEPARATOR};

use anyhow::{anyhow, Context, Result};
use clap::builder::{ArgAction, OsStringValueParser};
use clap::{
    crate_authors, crate_description, crate_name, crate_version, value_parser, Arg, ArgMatches,
    Command, ValueHint,
};
use log::{debug, info, warn, LevelFilter};
use os_str_bytes::OsStrBytes;
use simplelog::{ColorChoice, ConfigBuilder, TermLogger, TerminalMode};

use kibank::read::BankReader;
use kibank::write::BankWriter;
use kibank::{ItemKind, Metadata, BACKGROUND_FILE_STEM, PATH_SEPARATOR};

fn main() -> Result<()> {
    // Command line arguments
    let app = Command::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .action(ArgAction::Count),
        )
        .subcommand(
            Command::new("create")
                .about("Create a new bank")
                .visible_alias("c")
                .arg(
                    Arg::new("name")
                        .help("Title of the new bank")
                        .long("name")
                        .short('n')
                        .takes_value(true),
                )
                .arg(
                    Arg::new("author")
                        .help("Creator of the new bank")
                        .long("author")
                        .short('a')
                        .takes_value(true),
                )
                .arg(
                    Arg::new("description")
                        .help("Overview of the new bank")
                        .long("description")
                        .alias("desc")
                        .short('d')
                        .takes_value(true),
                )
                .arg(
                    Arg::new("id")
                        .help("Unique identifier for the new bank")
                        .long("id")
                        .short('i')
                        .hide(true)
                        .takes_value(true),
                )
                //
                // These hash and version fields occur in the metadata in the
                // Kilohearts factory content banks but not those made with
                // Kilohearts Bank Maker. These fields is not well understood
                // so these options are hidden.
                //
                .arg(
                    Arg::new("version")
                        .help("Version number of the new bank")
                        .long("version")
                        .value_parser(value_parser!(u32))
                        .takes_value(true)
                        .hide(true),
                )
                .arg(
                    Arg::new("hash")
                        .help("Hash digest for new bank in hex, 160 bits")
                        .long("hash")
                        .takes_value(true)
                        .hide(true),
                )
                //
                .arg(
                    Arg::new("BANK_FILE")
                        .help("File name of new bank")
                        .value_hint(ValueHint::AnyPath)
                        .value_parser(OsStringValueParser::new())
                        .required(true),
                )
                .arg(
                    Arg::new("IN_FILES")
                        .help("Files and directories to add to the bank")
                        .value_hint(ValueHint::AnyPath)
                        .value_parser(OsStringValueParser::new())
                        .multiple_values(true)
                        .required(true),
                ),
        )
        .subcommand(
            Command::new("extract")
                .about("Extract the contents of a bank")
                .visible_alias("x")
                .arg(
                    Arg::new("dest")
                        .long("dest")
                        .short('d')
                        .value_hint(ValueHint::DirPath)
                        .value_parser(OsStringValueParser::new())
                        .help("Destination directory")
                        .required(false),
                )
                .arg(
                    Arg::new("BANK_FILE")
                        .value_hint(ValueHint::FilePath)
                        .value_parser(OsStringValueParser::new())
                        .help("File name of the bank")
                        .required(true),
                ),
        )
        .subcommand(
            Command::new("info")
                .about("Display the details of a bank")
                .visible_alias("i")
                .arg(
                    Arg::new("BANK_FILE")
                        .help("File name of the bank")
                        .value_hint(ValueHint::AnyPath)
                        .value_parser(OsStringValueParser::new())
                        .required(true),
                ),
        )
        .subcommand(
            Command::new("list")
                .about("Display the contents of a bank")
                .visible_alias("l")
                .arg(
                    Arg::new("BANK_FILE")
                        .help("File name of the bank")
                        .value_hint(ValueHint::AnyPath)
                        .value_parser(OsStringValueParser::new())
                        .required(true),
                ),
        );
    let cli_matches = app.get_matches();

    let log_level_filter = [
        LevelFilter::Off,
        LevelFilter::Error,
        LevelFilter::Warn,
        LevelFilter::Info, // One --verbose given
        LevelFilter::Debug,
        LevelFilter::Trace,
    ]
    .get(*cli_matches.get_one::<u8>("verbose").unwrap() as usize + 2)
    .unwrap_or(&LevelFilter::Trace);

    // Logging
    let log_config = ConfigBuilder::new()
        .set_time_level(LevelFilter::Debug)
        .set_thread_level(LevelFilter::Trace)
        .set_target_level(*log_level_filter)
        .build();

    TermLogger::init(
        *log_level_filter,
        log_config,
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )?;

    match cli_matches.subcommand() {
        Some(("create", args)) => create(args),
        Some(("extract", args)) => extract(args),
        Some(("gui", app)) => extract(app),
        Some(("info", args)) => info(args),
        Some(("list", args)) => list(args),
        _ => Err(anyhow!("Missing command (-h for help)")),
    }
}

/// Make a new bank.
fn create(args: &ArgMatches) -> Result<()> {
    // Information about the files to include in the bank.
    #[derive(Eq, Hash, PartialEq)]
    struct Item {
        path: PathBuf,
        kind: ItemKind,
    }

    let bank_file_name = args
        .get_one::<OsString>("BANK_FILE")
        .with_context(|| "Expected a bank file name")?;
    let bank_file = File::create(bank_file_name)
        .with_context(|| format!("Cannot create bank {}", bank_file_name.to_string_lossy()))?;
    let mut writer = BankWriter::new(bank_file);

    // Collect files to include.
    let mut items = Vec::with_capacity(32);
    let dir_entries = args
        .get_many::<OsString>("IN_FILES")
        .unwrap_or_default()
        .into_iter()
        .flat_map(walkdir::WalkDir::new);
    for entry in dir_entries {
        match entry {
            Err(error) => warn!("{error}"),
            Ok(entry) => match entry.metadata() {
                Err(error) => warn!("{error}"),
                Ok(entry_metadata) if entry_metadata.is_dir() => {}
                _ => {
                    if let Some(kind) = ItemKind::from(entry.path()) {
                        debug!("Adding {:?} from {}", kind, entry.path().display());
                        items.push(Item {
                            path: entry.path().to_owned(),
                            kind,
                        });
                    } else {
                        info!(
                            "Skipping {} because it is an unknown type of file",
                            entry.path().to_string_lossy()
                        );
                    }
                }
            },
        }
    }

    // Remove duplicates of files listed multiple times on the command line.
    // let (items, _) = items.partition_dedup(); // Unstable feature
    let items = items.iter().collect::<HashSet<&Item>>();
    debug!(
        "Creating bank {} from {} items",
        bank_file_name.to_string_lossy(),
        items.len()
    );

    // Background is first.
    let background_items = items
        .iter()
        .filter(|item| item.kind == ItemKind::Background);
    if background_items.count() > 1 {
        warn!("More than one background found");
    }
    if let Some(item) = items.iter().find(|item| item.kind == ItemKind::Background) {
        let mut file_name = OsString::from(BACKGROUND_FILE_STEM);
        debug!("Background is from the file {}", item.path.display());
        if let Some(extension) = item.path.extension() {
            if ItemKind::Background.has_extension(extension) {
                file_name.push(".");
                file_name.push(extension);
                writer.add_file(item.kind, &file_name, &item.path)?;
            } else {
                warn!(
                    "Unsupported type of background file, extension {} is not {}",
                    extension.to_string_lossy(),
                    ItemKind::Background.extensions().join(" or ")
                );
            }
        } else {
            warn!(
                "Cannot find the extension for the background image {}",
                item.path.display()
            );
        }
    }

    // Merge metadata given on the command line and from the files. Leave
    // the original metadata file untouched if there are no options supplied.
    let metadata_items = items.iter().filter(|item| item.kind == ItemKind::Metadata);
    let multiple_metadata = metadata_items.count() > 1;
    if multiple_metadata {
        warn!("More than one metadata file found");
    }

    let cli_author = args.get_one::<String>("author");
    let cli_name = args.get_one::<String>("name");
    let cli_description = args.get_one::<String>("description");
    let cli_id = args.get_one::<String>("id");
    let cli_version = args.get_one::<u32>("version");
    let cli_hash = args.get_one::<String>("hash");
    let metadata_from_cli = cli_author.is_some()
        || cli_name.is_some()
        || cli_description.is_some()
        || cli_id.is_some()
        || cli_version.is_some()
        || cli_hash.is_some();

    if multiple_metadata || metadata_from_cli {
        let metadata_from_file = match items.iter().find(|item| item.kind == ItemKind::Metadata) {
            Some(item) => {
                debug!("Metadata is from the file {}", item.path.display());
                let json = fs::read(&item.path)?;
                BankReader::parse_metadata(&json).with_context(|| {
                    format!(
                        "Cannot read {} as a metadata JSON file",
                        item.path.display()
                    )
                })
            }
            _ => Ok(Metadata::default()),
        }?;

        let metadata = Metadata {
            author: cli_author.cloned().unwrap_or(metadata_from_file.author),
            name: cli_name.cloned().unwrap_or(metadata_from_file.name),
            description: cli_description
                .cloned()
                .unwrap_or(metadata_from_file.description),
            id: cli_id.cloned().unwrap_or(metadata_from_file.id),
            version: cli_version.copied().or(metadata_from_file.version),
            hash: cli_hash.cloned().or(metadata_from_file.hash),
            ..metadata_from_file
        };
        writer.add_metadata(&metadata)?;
    } else if let Some(item) = items.iter().find(|item| item.kind == ItemKind::Metadata) {
        // Leave the original metadata file untouched if there is just one.
        writer.add_file(item.kind, OsStr::new(Metadata::FILE_NAME), &item.path)?;
    }

    // The rest of the items.
    for item in items
        .iter()
        .filter(|item| item.kind != ItemKind::Metadata && item.kind != ItemKind::Background)
    {
        if let Some(file_name) = item.path.file_name() {
            let contents = fs::read(&item.path)?;
            writer
                .add(item.kind, file_name, contents)
                .with_context(|| format!("Cannot add {} to write", item.path.display()))?;
        } else {
            warn!(
                "Skipping file {} because the file name cannot be extracted",
                item.path.display()
            );
        }
    }

    writer.write().map_err(Into::into)
}

/// Extract the contents of the bank. Existing files will be overwritten.
fn extract(args: &ArgMatches) -> Result<()> {
    // Default destination is the current directory
    let dest_dir = match args.get_one::<OsString>("dest") {
        None => std::env::current_dir()?,
        Some(osstr) => PathBuf::from(osstr),
    };
    info!("Destination dir is {}", dest_dir.display());

    // Open the bank
    let bank_file_name = args
        .get_one::<OsString>("BANK_FILE")
        .with_context(|| "Expected a bank file name")?;
    let bank_path = Path::new(bank_file_name);
    let mut reader = BankReader::open(bank_path)
        .with_context(|| format!("Cannot open bank {}", bank_path.display()))?;

    for item in reader.items() {
        // Verify the item file name is not interpreted as an absolute path
        // because Path::join() will replace entire path and allow the bank to
        // write outside the destination. See Rust issue #16507 at
        // https://github.com/rust-lang/rust/issues/16507

        // Banks have a consistent separator that needs to be changed to match the current platform.
        let platform_path = item
            .path_bytes
            .iter()
            .map(|c| {
                if *c == (MAIN_SEPARATOR as u8) {
                    MAIN_SEPARATOR as u8
                } else {
                    *c
                }
            })
            .collect::<Vec<u8>>();

        let item_path = Path::from_raw_bytes(platform_path)?;
        if item_path.is_absolute() {
            return Err(anyhow!(
                "File {} is absolute and cannot be extracted",
                item.file_name_lossy()
            ));
        }

        let dest_path = dest_dir.join(item_path);
        if item.is_directory() {
            info!("Creating directory {}", dest_path.display());
            fs::create_dir_all(&dest_path)
                .with_context(|| format!("Cannot create directory {}", dest_path.display()))?;
        } else {
            info!(
                "Extracting {} to {}",
                item.file_name_lossy(),
                dest_path.display()
            );

            // Create missing intermediate directories
            if let Some(parent_dir) = dest_path.parent() {
                fs::create_dir_all(&parent_dir).with_context(|| {
                    format!("Cannot create parent directory {}", parent_dir.display())
                })?;
            }

            reader.copy(&item, dest_path)?;
        }
    }

    Ok(())
}

/// Display the bank metadata.
fn info(args: &ArgMatches) -> Result<()> {
    let bank_file_name = args
        .get_one::<OsString>("BANK_FILE")
        .with_context(|| "Expected a bank file name")?;
    let bank_path = Path::new(bank_file_name);
    let mut reader = BankReader::open(bank_path)
        .with_context(|| format!("Cannot open bank {}", bank_path.display()))?;

    let mut metadata = Metadata::default();
    for item in reader.items() {
        if item.is_metadata_file() {
            metadata = reader.read_metadata(&item).with_context(|| {
                format!("Cannot read the metadata for bank {}", bank_path.display())
            })?;
            break;
        }
    }

    println!("ID: {}", metadata.id);
    println!("Name: {}", metadata.name);
    println!("Author: {}", metadata.author);
    println!("Description: {}", metadata.description);
    println!("Version: {}", metadata.version.unwrap_or_default());
    println!("Hash: {}", metadata.hash.unwrap_or_default());
    for extra in metadata.extra {
        println!("Extra: {}: {}", extra.0, extra.1);
    }
    Ok(())
}

/// Display the contents of the bank including directories.
fn list(args: &ArgMatches) -> Result<()> {
    let bank_file_name = args
        .get_one::<OsString>("BANK_FILE")
        .with_context(|| "Expected a bank file name")?;
    let bank_path = Path::new(bank_file_name);
    let reader = BankReader::open(bank_path)
        .with_context(|| format!("Cannot open bank {}", bank_path.display()))?;

    for item in reader.items() {
        print!("{}", item.file_name_lossy());
        if item.is_directory() {
            // Add a trailing slash that matches what is found in the banks,
            // not what's used by the operating system.
            print!("{}", PATH_SEPARATOR);
        }
        println!();
    }

    Ok(())
}
