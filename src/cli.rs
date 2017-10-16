use std::io;
use std::io::Write;
use std::collections::HashMap;

pub fn prompt_question(prompt: &str, default: bool) -> bool {
    print!("{}? ({}) ", prompt, if default { "yes" } else { "no" });

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
                1 => default,
                _ => parse(input.trim())
            }
        }
        Err(error) => panic!(error)
    }
}

pub fn prompt_line(prompt: &str, default: &str) -> Option<String> {
    if default == "" {
        print!("{}: ", prompt);
    } else {
        print!("{}: ({}) ", prompt, default);
    }
    
    let _ = io::stdout().flush();
    let mut input = String::new();

    match io::stdin().read_line(&mut input) {
        Ok(n) => {
            match n {
                0 => None,
                1 => Some(default.to_owned()),
                _ => Some(input.trim().to_owned())
            }
        }
        Err(error) => panic!(error)
    }
}

pub fn parse_os_list(vec: &[String]) -> HashMap<String, String> {
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
