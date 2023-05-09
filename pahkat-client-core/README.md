# Páhkat Client Core

[![Build Status](https://dev.azure.com/divvun/divvun-installer/_apis/build/status/divvun.pahkat-client-core?branchName=master)](https://dev.azure.com/divvun/divvun-installer/_build/latest?definitionId=6&branchName=master)

The base client for deriving further clients without reimplementing the wheel each time.

Includes a command line tool.

## Tips

If you want `xz2-rs` to statically link, add `LZMA_API_STATIC=1` to your environment before building.

## Notes

### MacOS
The MacOS service is using `pkgutil` to determine the status of installed packages and their dependencies. If files get manually deleted, `pkgutil` will not realize that and potentially report a package to be up to date even though it got deleted.
As a workaround, dependencies always get installed on MacOS. Packages that appear up to date but don't work need to be reinstalled.

## License

ISC license - see LICENSE file.
