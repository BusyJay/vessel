#[macro_use]
extern crate clap;
extern crate regex;
#[macro_use]
extern crate vessel;

use std::path::Path;
use std::process;

use regex::Regex;

struct ExecContex {
    met_error: bool,
}

fn find(pattern: &Option<Regex>, path: &Path, ctx: &mut ExecContex) {
    let name = path.file_name().and_then(|n| n.to_str());
    if pattern.as_ref().map_or(true, |p| name.map_or(false, |n| p.is_match(n))) {
        outputln!("{}", path.display());
    }
    if path.is_file() {
        return;
    }
    let f_list = match path.read_dir() {
        Err(e) => {
            fatal!("failed to list {}: {}", path.display(), e);
            ctx.met_error = true;
            return;
        }
        Ok(f_list) => f_list,
    };
    for p in f_list {
        match p {
            Err(e) => {
                fatal!("failed to list {}: {}", path.display(), e);
                return;
            }
            Ok(path) => find(pattern, path.path().as_path(), ctx)
        }
    }
}

fn main() {
    let matches = clap_app!(find => 
        (version: "0.1.0")
        (@arg regex: --regex +takes_value "pattern to use for matching")
        (@arg path: +required "directory to search")
    ).get_matches();

    let path = matches.value_of("path").unwrap();
    let pattern = matches.value_of("regex").map(|p| {
        let mut s = p.to_owned();
        if !s.starts_with('^') {
            s.insert(0, '^');
        }
        if !s.ends_with('$') {
            s.push('$');
        }
        match Regex::new(&s) {
            Ok(p) => p,
            Err(e) => {
                fatal!("failed to parse pattern {:?}: {}", p, e);
                process::exit(-1);
            }
        }
    });

    let mut ctx = ExecContex { met_error: false };
    let p = Path::new(path);
    if p.is_file() || p.is_dir() {
        find(&pattern, p, &mut ctx);
    } else {
        fatal!("{} is not a file or directory.", p.display());
        ctx.met_error = true;
    }
    if ctx.met_error {
        process::exit(-1);
    }
}
