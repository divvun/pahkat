pub(crate) const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "  <https://github.com/divvun/pahkat-cli>\n\n",
    "Authors: ",
    structopt::clap::crate_authors!(),
    "\n",
    "License: GPL-3.0 <https://github.com/divvun/pahkat-cli/LICENSE>",
);

pub(crate) const MAIN_TEMPLATE: &str = "pahkat v{version}  <https://github.com/divvun/pahkat-cli>

Usage: {usage}

Commands:
{subcommands}

Options:
{unified}
";

pub(crate) const SUB_TEMPLATE: &str = "pahkat v{version}  <https://github.com/divvun/pahkat-cli>

Usage: {usage}

Arguments:
{positionals}

Options:
{unified}
";

pub(crate) const SUBN_TEMPLATE: &str = "pahkat v{version}  <https://github.com/divvun/pahkat-cli>

Usage: {usage}

Options:
{unified}
";

pub(crate) const SUBC_TEMPLATE: &str = "pahkat v{version}  <https://github.com/divvun/pahkat-cli>

Usage: {usage}

Commands:
{subcommands}

Options:
{unified}
";
