# Kibank

Kibank is a command line application for listing, extracting and creating bank
files used by [Kilohearts](https://kilohearts.com) products like
[Phase Plant](https://kilohearts.com/products/phase_plant),
[Snap Heap](https://kilohearts.com/products/multipass) and
[Multipass](https://kilohearts.com/products/multipass) as well as any Snap In.

This application was developed independently by Sheldon Young. Kibank is *not* 
a Kilohearts product, please do not contact them for support.

## Installation

### Binaries

Binaries are available for several platforms from 
[GitHub releases](https://github.com/softdevca/kibank/releases).

### From crates.io

To install this application from [crates.io](https://crates.io/crates/kibank)
ensure [Rust](https://rust-lang.org) is available then run:

```shell
$ cargo install kibank
```

### From Source

To install this application from source ensure `git` and 
[Rust](https://rust-lang.org) are available then run:

```shell
$ git clone https://github.com/softdev.ca/kibank
$ cd kibank 
$ cargo build --release
$ cp target/release/kibank /your/dest/path/
```

## Usage

List the contents of a bank:

```shell
$ kibank list MyBank.bank
```

View bank details:

```shell
$ kibank info MyBank.bank
```

Extract a bank to the current directory:

```shell
$ kibank extract MyBank.bank
```

Extract a bank to a designated directory:

```shell
$ kibank extract -d output_directory MyBank.bank
```

### Creating a new bank

To create a new bank give the names of the files and directories to include as
arguments. Only files that are recognized as compatible are included in the bank.

```shell
$ kibank create MyBank.bank your_files_and_directories
```

Include a file named `background.png` or `background.jpg` to set the background
image used for the bank.

To create a new bank with additional metadata:

```shell
$ kibank create --author "Your Name" --name "My Bank" --description "Weird and wonderful presets" MyNewBank.bank presets samples/*.wav
```

To create a new bank by supplying the metadata directly include a file in the
bank named `index.json` with contents in this format:

```json
{
  "name": "My Bank",
  "author": "Your Name",
  "description": "My weird and wonderful presets"
}
```

### Getting Help

Additional information about how to use `kibank` is available with the `--help` option:

```console
$ kibank --help
kibank 0.1.2
Sheldon Young <sheldon@softdev.ca>
Tool for Kilohearts banks

USAGE:
    kibank [OPTIONS] [SUBCOMMAND]

OPTIONS:
    -h, --help       Print help information
    -v, --verbose
    -V, --version    Print version information

SUBCOMMANDS:
    create     Create a new bank [aliases: c]
    extract    Extract the contents of a bank [aliases: x]
    help       Print this message or the help of the given subcommand(s)
    info       Display the details of a bank [aliases: i]
    list       Display the contents of a bank [aliases: l]
```

Each subcommand like *create*, *extract*, *info* and *list* also have a `--help`
option. For example:

```console
$ kibank create --help
kibank-create
Create a new bank

USAGE:
    kibank create [OPTIONS] <BANK_FILE> <IN_FILES>...

ARGS:
    <BANK_FILE>      File name of new bank
    <IN_FILES>...    Files and directories to add to the bank

OPTIONS:
    -a, --author <author>              Creator of the new bank
    -d, --description <description>    Overview of the new bank
    -h, --help                         Print help information
    -n, --name <name>                  Title of the new bank
```

## Compared with Kilohearts Bank Maker

Bank Maker by Kilohearts is the official application for creating banks. The
differences are that Bank Maker:

* Has a graphical user interface
* Supports project files that can be opened and edited
* Can directly modify the description and author in each preset
* Allows an extra subdirectory to be designated for each file

Kibank is a command line application. It has a more direct workflow for those
comfortable with the command line and is much easier to automate.

## Library

The functionality powering this application is available as a library to reuse
in your own Rust applications. The `kibank` crate on 
[crates.io](https://crates.io/crates/kibank) can be added to your `Cargo.toml`:

```toml
[dependencies]
kibank = { version = "0", default-features = false }
```

## Issues

If you have any problems with or questions about this project, please contact
us through by creating a 
[GitHub issue](https://github.com/softdevca/kibank/issues).

## Contributing

You are invited to contribute to new features, fixes, or updates, large or
small; we are always thrilled to receive pull requests, and do our best to
process them as fast as we can.

Before you start to code, we recommend discussing your plans through a
[GitHub issue](https://github.com/softdevca/kibank/issues), especially for more
ambitious contributions. This gives other
contributors a chance to point you in the right direction, give you feedback on
your design, and help you find out if someone else is working on the same thing.

The copyrights of contributions to this project are retained by their
contributors. No copyright assignment is required to contribute to this
project.

## License

Licensed under the Apache License, Version 2.0 (the "License"); you may not use
this file except in compliance with the License. You may obtain a copy of the 
License at

http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software distributed
under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR 
CONDITIONS OF ANY KIND, either express or implied. See the License for the
specific language governing permissions and limitations under the License.

