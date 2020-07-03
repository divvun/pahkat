#[cfg(feature = "windows")]
macro_rules! target { () => { "windows" } }

#[cfg(feature = "macos")]
macro_rules! target { () => { "macos" } }

#[cfg(feature = "prefix")]
macro_rules! target { () => { "prefix" } }

macro_rules! title {
    () => {
        concat!("pahkat v", env!("CARGO_PKG_VERSION"), " (", target!(), ") <https://github.com/divvun/pahkat>")
    };
}

pub(crate) const VERSION: &str = concat!(
    "v", env!("CARGO_PKG_VERSION"), " (", target!(), ")",
    " <https://github.com/divvun/pahkat>\n\n",
    "Authors: ",
    structopt::clap::crate_authors!(),
    "\n",
    "License: GPL-3.0 <https://github.com/divvun/pahkat/blob/master/pahkat-cli/LICENSE>",
);

pub(crate) const MAIN_TEMPLATE: &str = concat!(title!(), "

Usage: {usage}

Commands:
{subcommands}

Options:
{unified}
");

pub(crate) const SUB_TEMPLATE: &str = concat!(title!(), "

Usage: {usage}

Arguments:
{positionals}

Options:
{unified}
");

pub(crate) const SUBN_TEMPLATE: &str = concat!(title!(), "

Usage: {usage}

Options:
{unified}
");

pub(crate) const SUBC_TEMPLATE: &str = concat!(title!(), "

Usage: {usage}

Commands:
{subcommands}

Options:
{unified}
");
