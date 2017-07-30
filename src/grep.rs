#[macro_use]
extern crate clap;
extern crate regex;
#[macro_use]
extern crate vessel;

use std::collections::VecDeque;
use std::fs::File;
use std::io::{self, Read, BufRead, BufReader, Lines};
use std::iter::Peekable;
use std::path::Path;
use std::{process, str};

use regex::{Regex, Matches};
use vessel::*;

struct Opt {
    invert: bool,
    recursive: bool,
    before_num: usize,
    after_num: usize,
}

struct ExecContex {
    opt: Opt,
    met_error: bool,
    collected: bool,
}

fn output_name(name: Option<&Path>, line_num: i64) {
    if let Some(p) = name {
        output!("{}:{}:", p.display(), line_num);
    }
}

fn report_err(name: Option<&Path>, e: io::Error, ctx: &mut ExecContex) {
    ctx.met_error = true;
    match name {
        Some(p) => fatal!("unable to grep file {}: {}", p.display(), e),
        None => fatal!("unable to grep stdin: {}", e),
    }
}

struct BufferedLines<'a, R> {
    name: Option<&'a Path>,
    reader: Option<Lines<BufReader<R>>>,
    lines: VecDeque<Option<String>>,
    idx: i64,
    first_idx: usize,
}

impl<'a, R: Read> BufferedLines<'a, R> {
    fn new(name: Option<&'a Path>, r: R, opt: &Opt) -> Result<BufferedLines<'a, R>, io::Error> {
        let mut reader = Some(BufReader::new(r).lines());
        let cap = opt.before_num + opt.after_num + 1;
        let mut lines = VecDeque::with_capacity(cap);
        for _ in 0..opt.before_num + 1 {
            lines.push_back(None);
        }
        for _ in 0..opt.after_num {
            if reader.is_none() {
                lines.push_back(None);
                continue;
            }

            match reader.as_mut().unwrap().next() {
                Some(l) => lines.push_back(Some(l?)),
                None => {
                    lines.push_back(None);
                    reader.take();
                }
            }
        }
        Ok(BufferedLines {
            name: name,
            reader: reader,
            lines: lines,
            idx: -(opt.before_num as i64),
            first_idx: opt.before_num,
        })
    }

    fn next(&mut self) -> Result<(), io::Error> {
        self.lines.pop_front();
        self.idx += 1;
        if self.reader.is_none() {
            self.lines.push_back(None);
            return Ok(());
        }

        // TODO: what if line is too long?
        match self.reader.as_mut().unwrap().next() {
            None => {
                self.reader.take();
                self.lines.push_back(None);
            }
            Some(s) => self.lines.push_back(Some(s?)),
        }
        Ok(())
    }

    fn cur(&self) -> Option<&str> {
        self.lines.get(self.first_idx).unwrap().as_ref().map(|s| s.as_str())
    }
}

fn output_line(line: &str, matches: Option<Peekable<Matches>>) {
    if matches.is_none() {
        outputln!("{}", line);
        return;
    }

    let mut last_index = 0;
    for m in matches.unwrap() {
        if m.start() > last_index {
            let s = unsafe {
                str::from_utf8_unchecked(&line.as_bytes()[last_index..m.start()])
            };
            output!("{}", s);
        }
        output!("{}", m.as_str());
        last_index = m.end();
    }
    if last_index != line.len() {
        let s = unsafe {
            str::from_utf8_unchecked(&line.as_bytes()[last_index..])
        };
        outputln!("{}", s);
    } else {
        outputln!("");
    }
}

fn report_output<R>(lines: &BufferedLines<R>, mut matches: Option<Peekable<Matches>>, ctx: &mut ExecContex) {
    ctx.collected = true;
    output_name(lines.name, lines.idx + lines.first_idx as i64);
    if lines.lines.len() == 1 {
        output_line(lines.lines.front().unwrap().as_ref().unwrap().as_str(), matches);
        return;
    }
    outputln!("");
    for (i, l) in lines.lines.iter().enumerate() {
        if l.is_none() {
            continue;
        }
        if i == lines.first_idx {
            output_line(l.as_ref().unwrap().as_str(), matches.take())
        } else {
            output_line(l.as_ref().unwrap().as_str(), None)
        }
    }
    outputln!("");
}

fn grep_reader<R: Read>(pattern: &Regex, name: Option<&Path>, r: R, ctx: &mut ExecContex) {
    let mut lines = match BufferedLines::new(name, r, &ctx.opt) {
        Err(e) => {
            report_err(name, e, ctx);
            return;
        }
        Ok(lines) => lines,
    };
    loop {
        if let Err(e) = lines.next() {
            report_err(name, e, ctx);
            return;
        }
        match lines.cur() {
            None => return,
            Some(l) => {
                let mut matches = pattern.find_iter(l).peekable();
                if matches.peek().is_none() {
                    if ctx.opt.invert {
                        report_output(&lines, None, ctx);
                    }
                    continue;
                }
                if ctx.opt.invert {
                    continue;
                }
                report_output(&lines, Some(matches), ctx);
            }
        }
    }
}

fn grep_dir(pattern: &Regex, dir: &Path, ctx: &mut ExecContex) -> Result<(), io::Error> {
    for f in dir.read_dir()? {
        grep(pattern, Some(f?.path().as_path()), ctx);
    }
    Ok(())
}

fn grep(pattern: &Regex, file: Option<&Path>, ctx: &mut ExecContex) {
    match file {
        None => grep_reader(pattern, file, io::stdin(), ctx),
        Some(p) if p.is_dir() => {
            if ctx.opt.recursive {
                if let Err(e) = grep_dir(pattern, p, ctx) {
                    report_err(file, e, ctx);
                }
            } else {
                outputln!("{} is a directory.", p.display());
            }
        }
        Some(p) => {
            match File::open(p) {
                Ok(f) => grep_reader(pattern, file, f, ctx),
                Err(e) => report_err(file, e, ctx),
            }
        }
    }
}

fn main() {
    let matches = clap_app!(grep => 
        (version: "0.1.0")
        (about: "Search for PATTERN in each FILE or standard input.")
        (@arg silent: -s "suppress error messages")
        (@arg invert: -v "select non-matching lines")
        (@arg quiet: -q --quiet "suppress all normal output")
        (@arg recursive: -R "same as -r, but follow links")
        (@arg BEFORE_NUM: -B +takes_value "print BEFORE_NUM lines of leading context")
        (@arg AFTER_NUM: -A +takes_value "print AFTER_NUM lines of trailing context")
        (@arg PATTERN: +required "PATTERN to used for search")
        (@arg FILES: ... "files to search, if not specified, will search from stdin")
    ).get_matches();
    if matches.is_present("silent") {
        vessel::suppress_stderr();
    }
    if matches.is_present("quiet") {
        vessel::suppress_stdout();
    }
    let grep_opt = Opt {
        invert: matches.is_present("invert"),
        recursive: matches.is_present("recursive"),
        before_num: parse(matches.value_of("BEFORE_NUM").unwrap_or("0")),
        after_num: parse(matches.value_of("AFTER_NUM").unwrap_or("0")),
    };
    let pattern = matches.value_of("PATTERN").unwrap();
    let p = match Regex::new(pattern) {
        Err(e) => {
            fatal!("unrecognize pattern {:?}: {}", pattern, e);
            process::exit(-1);
        },
        Ok(p) => p,
    };
    let files = match matches.values_of("FILES") {
        None => vec![None],
        Some(files) => files.map(|f| Some(Path::new(f))).collect(),
    };
    let mut ctx = ExecContex { opt: grep_opt, met_error: false, collected: false };
    for f in files {
        grep(&p, f, &mut ctx)
    }
    if ctx.met_error || !ctx.collected {
        process::exit(-1);
    }
}
