## Building

1. Install rust: https://www.rust-lang.org/tools/install
2. Install `protobuf`
```
$ brew install protobuf // this will differ on Windows
```
3. Build for your platform
```
cargo build --release --features macos // this will differ on Windows
```


## Pahkat CLI client


### Binaries 

The linux client is the most implemented one. Get it here:

"https://pahkat.uit.no/devtools/download/pahkat-prefix-cli?platform=linux&channel=nightly"

The other ones also exist:
- "https://pahkat.uit.no/devtools/download/pahkat-prefix-cli?platform=macos&channel=nightly"
- "https://pahkat.uit.no/devtools/download/pahkat-prefix-cli?platform=windows&channel=nightly"


### Installation:

```
wget "https://pahkat.uit.no/devtools/download/pahkat-prefix-cli?platform=linux&channel=nightly"
mv "pahkat-prefix-cli?platform=linux&channel=nightly" pahkat-prefix-cli.tar.xz
apt install xz-utils
tar xvf pahkat-prefix-cli.tar.xz
chmod a+x bin/pahkat-prefix
```

### Usage

```
$ bin/pahkat-prefix -h
pahkat v2.3.0 (prefix) <https://github.com/divvun/pahkat>

Usage: pahkat <command>

Commands:
    config       Manage package manager configuration and settings
    download     Download packages into a specified directory
    init         Initialize configuration
    install      Install packages from configured repositories
    status       Query status of given packages
    uninstall    Uninstall previously installed packages

Options:
    -h, --help       Prints help information
    -V, --version    Prints version and license information

$ bin/pahkat-prefix init -c /some_temp_path/pahkat-prefix
$ bin/pahkat-prefix config repo add -c /some_temp_path/pahkat-prefix https://pahkat.uit.no/devtools/ nightly
$ bin/pahkat-prefix install pahkat-uploader divvun-bundler thfst-tools xcnotary -c /some_temp_path/pahkat-prefix
```
