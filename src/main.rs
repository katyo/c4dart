mod options;
mod coder;
mod translator;

use std::{
    path::PathBuf,
    io::Write,
    fs::File,
};
use regex::Regex;
use log::LevelFilter;
use clang::{Clang, Index};

pub use options::*;
pub use coder::*;
pub use translator::*;

/// Command-line arguments
#[derive(Debug, structopt::StructOpt)]
#[structopt(about)]
struct Args {
    /// Print version number
    #[structopt(short = "V", long)]
    version: bool,
    
    /// C headers to parse
    #[structopt(parse(from_os_str))]
    input: Option<PathBuf>,

    /// Dart source output
    #[structopt(short, long, parse(from_os_str))]
    output: Option<PathBuf>,

    /// Dart class name
    #[structopt(short, long)]
    class: Option<String>,

    /// Log level
    #[structopt(short, long, env, parse(try_from_str), default_value = "off")]
    log: LevelFilter,

    /// Name match pattern
    #[structopt(short, long = "match", env, parse(try_from_str = Regex::new), default_value = ".*")]
    match_: Regex,

    /// Name replace pattern
    #[structopt(short, long, env, default_value = "$0")]
    replace: String,
}

#[paw::main]
fn main(args: Args) {
    if args.version {
        println!("Version: {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    {
        std::env::set_var("__LOG_LEVEL_FILTER__", args.log.to_string());
        pretty_env_logger::init_custom_env("__LOG_LEVEL_FILTER__");
    }

    let input = args.input.expect("Missing input C header");
    let output = args.output.expect("Missing output Dart source");

    let class = args.class.or_else(|| {
        input.file_stem().or_else(|| output.file_stem())
            .and_then(|name| name.to_str()).map(|name| name.into())
    }).expect("Missing library class name");

    let options = Options {
        match_: args.match_,
        replace: args.replace,
    };
    
    let clang = Clang::new().unwrap();

    let index = Index::new(&clang, false, true);

    let mut args = Vec::new();

    args.push("-xc".into());

    {
        let paths = system_headers_search_paths();

        for path in paths {
            args.push(format!("-isystem{}", path.display()));
        }
    }

    let tu = index.parser(&input)
        .arguments(&args)
        .parse().unwrap();

    let mut translator = Translator::new(options);

    translator.translate(tu.get_entity());
    translator.make_class(&class);

    let mut file = File::create(&output).expect("Unable to create output file");
    writeln!(file, "/* This file was generated using {program} v{version} tool and should not be modified manually. */", program = env!("CARGO_PKG_NAME"), version = env!("CARGO_PKG_VERSION")).expect("Unable to write output file");
    writeln!(file, "{}", translator.coder()).expect("Unable to write output file");
}

fn system_headers_search_paths() -> Vec<PathBuf> {
    use std::{
        str::from_utf8,
        process::{Command, Stdio},
    };

    let out = Command::new("clang")
        .arg("-E").arg("-xc").arg("-v").arg("-")
        .stdin(Stdio::null())
        .output().unwrap().stderr;
    
    let out = from_utf8(out.as_ref()).unwrap();

    let mut lines = out.lines();

    for line in &mut lines {
        if line == "#include <...> search starts here:" {
            break;
        }
    }

    let mut paths = Vec::new();

    for line in &mut lines {
        if line == "End of search list." {
            break;
        }
        paths.push(PathBuf::from(line.trim()));
    }

    paths
}
