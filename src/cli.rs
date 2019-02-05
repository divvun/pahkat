use std::io;
use std::io::Write;
use std::collections::BTreeMap;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
use dialoguer::{Confirmation, Input, Checkboxes, Select, theme::ColorfulTheme};

pub fn progress(color: Color, first: &str, rest: &str) -> Result<(), io::Error> {
    let mut stderr = StandardStream::stderr(ColorChoice::Always);
    stderr.set_color(ColorSpec::new().set_fg(Some(color))
        .set_intense(true)
        .set_bold(true))?;
    write!(&mut stderr, "{:>12}", first)?;
    stderr.reset()?;
    writeln!(&mut stderr, " {}", rest)?;
    Ok(())
}

pub fn prompt_question(prompt: &str, default: bool) -> bool {
    Confirmation::with_theme(&ColorfulTheme::default())
        .with_text(prompt)
        .default(default)
        .interact()
        .unwrap_or(default)
}

pub fn prompt_line(prompt: &str, default: &str) -> Option<String> {
    Some(Input::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .default(default.to_string())
        .interact()
        .unwrap_or(default.to_string())
        .to_string())
}

pub fn prompt_multi_select(prompt: &str, options: &[&str]) -> Vec<String> {
    Checkboxes::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .items(options)
        .interact()
        .unwrap_or(vec![])
        .into_iter()
        .map(|i| options[i].to_string())
        .collect()
}

pub fn prompt_select(prompt: &str, options: &[String], default: usize) -> String {
    Select::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .items(options)
        .default(default)
        .interact()
        .map(|i| options[i].to_string())
        .unwrap_or_else(|_| options[default].to_string())
}

pub fn parse_platform_list(vec: &[String]) -> BTreeMap<String, String> {
    let mut map: BTreeMap<String, String> = BTreeMap::new();

    for item in vec {
        let chunks: Vec<&str> = item.splitn(2, " ").collect();

        if chunks.len() == 1 {
            map.insert(chunks[0].to_owned(), "*".to_owned());
        } else {
            map.insert(chunks[0].to_owned(), chunks[1].trim().to_owned());
        }
    }

    map
}
