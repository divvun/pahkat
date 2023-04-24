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