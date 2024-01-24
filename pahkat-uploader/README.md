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

### Adding additional metadata to the release

As of version 2.XXXXXX it's possible to update a package's `name` and `description` fields.

There are 2 ways to do this:
1. Using the `--metadata-json` option to supply a json file (see format below)
2. Using the `--additional-meta` option to supply a toml file (see format below)

#### Supplying `name` and `description` data via json
This is currently used when uploading keyboard releases. The json file you provide with the `--metadata-json` option should be formatted like this:
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
`pahkat-uploader` will convert it to the necessary format. This format is used because it's what is used in [`xxx.kbdgen/project.yaml`](https://github.com/giellalt/keyboard-fit/blob/main/fit.kbdgen/project.yaml)

#### Supplying `name` and `description` data via toml
This is currently used when uploaded spellers, and will later be expanded to include grammar checkers and speech synthesis. The toml file you provide with the `--additional-meta` should be formatted like this:
```toml
[name]
en = "name"
sv = "namn"
...

[description]
en = "description"
sv = "beskrivning"
...
```