# pahkat-uploader

`pahkat-uploader` is a command line utility that has two responsibilities:
1. Creating a `metadata.toml` for a release to be uploaded to a pahkat index using [`pahkat-reposrv`](https://github.com/divvun/pahkat-reposrv).
2. Uploading the `metadata.toml` generated in step 1, along with optional other metadata, via `pahkat-reposrv`.

It is used by CI to do both of the above.

An example (taken from CI) of generating a metadata file looks like this. Note that the command prints its output to stdout; the user is responsible for piping that output to a file.
```bash
pahkat-uploader release --channel nightly -p macos --version 0.0.1 macos-package -i 1 -s 14144 -p no.uit.giella.keyboards.fit.keyboardlayout.fit -u https://pahkat.uit.no/artifacts/keyboard-fit_0.0.1_macos.pkg -t system,user -r install,uninstall
```

Uploading a `metadata.toml` file looks like this (also taken from CI):
```bash
pahkat-uploader upload -u https://pahkat.thetc.se/main/packages/keyboard-fit -P ./metadata.toml
```

### Adding additional metadata to the release

As of version 2.3 it's possible to update a package's `name` and `description` fields.

There are 2 ways to do this:
1. Using the `--metadata-json` option to supply a json file, used for keyboards
2. Using the `--manifest-toml` option to supply a toml file and the `--package-type` option to supply package type (at the time of this writing, only `speller` is supported. It will be expanded to include grammar checkers and other types in the future.) 

#### Supplying additional metadata for keyboard releases
The json file you provide with the `--metadata-json` option should be formatted like this:
```json
{
  "fit": {
    "name": "Meänkieli-näppäimistöt",
    "description": "Näppäimistöt Meänkielille UiT:n Divvun-ryhmästä ja Giellatekno-ryhmästä."
  },
  "en": {
    "name": "Meänkieli Keyboards",
    "description": "Keyboards for the Meänkieli language from the Divvun and Giellatekno groups at UiT."
  },
  "nb": {
    "name": "Meänkieli tastatur",
    "description": "Tastatur for Meänkieli fra Divvun-gruppa ved UiT."
  }
  ...
}
```
`pahkat-uploader` will convert it to the necessary format. This format is used because it's the format used by [`xxx.kbdgen/project.yaml`](https://github.com/giellalt/keyboard-fit/blob/main/fit.kbdgen/project.yaml) files.

A full command that supplies a `metadata.json` might look like this:

```bash
pahkat-uploader upload -u https://pahkat.thetc.se/main/packages/keyboard-fit --release-meta ./metadata.toml --metadata-json ./metadata.json
```

#### Supplying additional metadata when for `lang-xxx` releases (spellers, grammar checkers, etc) via `manifest.toml`
This is currently used when uploading spellers, and will later be expanded to include grammar checkers and other types. The toml file you provide with the `--manifest-toml` option should be formatted like this:
```toml
[speller.name]
en = "name"
sv = "namn"
...

[speller.description]
en = "description"
sv = "beskrivning"
...
```

A full command that supplies a `manifest.toml` might look like:

```bash
pahkat-uploader upload --url http://test.com  --release-meta release-meta.toml --manifest-toml manifest.toml --package-type speller
```