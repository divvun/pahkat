# pahkat-uploader

`pahkat-uploader` is a command line utility that has two responsibilities:
1. Creating a `metadata.toml` for a release to be uploaded to a pahkat index using [`pahkat-reposrv`](https://github.com/divvun/pahkat-reposrv).
2. Uploading the `metadata.toml` generated in step 1 via `pahkat-reposrv`.

It is used by CI to do both of the above.

An example (taken from CI) of generating a metadata file looks like this. Note that the command prints its output to stdout; the user is responsible for piping that output to a file.
```bash
pahkat-uploader release --channel nightly -p macos --version 0.0.1 macos-package -i 1 -s 14144 -p no.uit.giella.keyboards.fit.keyboardlayout.fit -u https://pahkat.uit.no/artifacts/keyboard-fit_0.0.1_macos.pkg -t system,user -r install,uninstall
```

Uploading a `metadata.toml` file looks like this (also taken from CI):
```bash
pahkat-uploader upload -u https://pahkat.thetc.se/main/packages/keyboard-fit -P ./metadata.toml
```