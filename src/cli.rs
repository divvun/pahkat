use std::io;
use std::io::Write;
use std::collections::HashMap;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

#[cfg(target_os = "windows")]
pub const INPUT_DEFAULT_LEN: usize = 2;
#[cfg(not(target_os = "windows"))] 
pub const INPUT_DEFAULT_LEN: usize = 1;

pub fn progress(color: Color, first: &str, rest: &str) -> Result<(), io::Error> {
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    try!(stdout.set_color(ColorSpec::new().set_fg(Some(color))
        .set_intense(true)
        .set_bold(true)));
    try!(write!(&mut stdout, "{:>12}", first));
    stdout.reset()?;
    writeln!(&mut stdout, " {}", rest)?;
    Ok(())
}

pub fn prompt_question(prompt: &str, default: bool) -> bool {
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    stdout.set_color(ColorSpec::new().set_fg(Some(Color::Cyan))).unwrap();
    write!(&mut stdout, "{}: ", prompt).unwrap();
    stdout.reset().unwrap();

    print!("({}) ", if default { "yes" } else { "no" });

    let _ = io::stdout().flush();
    let mut input = String::new();

    fn parse(it: &str) -> bool {
        let lower = it.to_lowercase();

        if lower == "y" || lower == "yes" {
            return true;
        }

        false
    }

    match io::stdin().read_line(&mut input) {
        Ok(n) => {
            match n {
                0 => false,
                INPUT_DEFAULT_LEN => default,
                _ => parse(input.trim())
            }
        }
        Err(error) => panic!(error)
    }
}

pub fn prompt_line(prompt: &str, default: &str) -> Option<String> {
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    stdout.set_color(ColorSpec::new().set_fg(Some(Color::Cyan))).unwrap();
    write!(&mut stdout, "{}: ", prompt).unwrap();
    stdout.reset().unwrap();
    
    if default != "" {
        print!("({}) ", default);
    }
    
    let _ = io::stdout().flush();
    let mut input = String::new();

    match io::stdin().read_line(&mut input) {
        Ok(n) => {
            match n {
                0 => None,
                INPUT_DEFAULT_LEN => Some(default.to_owned()),
                _ => Some(input.trim().to_owned())
            }
        }
        Err(error) => panic!(error)
    }
}

pub fn parse_platform_list(vec: &[String]) -> HashMap<String, String> {
    let mut map: HashMap<String, String> = HashMap::new();

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
