use std::{
    path::PathBuf,
    fs::File,
};
use regex::Regex;
use log::LevelFilter;

pub use c4dart::{Options, translate};

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
    
    /// Library class name
    #[structopt(short, long)]
    class_name: Option<String>,

    /// Extra include paths
    #[structopt(short = "I", long, parse(from_os_str))]
    include_paths: Vec<PathBuf>,
    
    /// Skip system include paths detection
    #[structopt(short = "D", long)]
    no_system_includes: bool,

    /// Name match pattern
    #[structopt(short = "m", long = "match", env, parse(try_from_str = Regex::new), default_value = ".*")]
    names_match: Regex,

    /// Name replace pattern
    #[structopt(short = "r", long = "replace", env, default_value = "$0")]
    names_replace: String,

    /// Log level
    #[structopt(short, long, env, parse(try_from_str), default_value = "off")]
    log_level: LevelFilter,
}

#[paw::main]
fn main(args: Args) {
    if args.version {
        println!("Version: {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    {
        std::env::set_var("__LOG_LEVEL_FILTER__", args.log_level.to_string());
        pretty_env_logger::init_custom_env("__LOG_LEVEL_FILTER__");
    }

    let input = args.input.expect("Missing input C header");
    let output = args.output.expect("Missing output Dart source");

    let class_name = args.class_name.or_else(|| {
        input.file_stem().or_else(|| output.file_stem())
            .and_then(|name| name.to_str()).map(|name| name.into())
    }).expect("Missing library class name");

    let options = Options {
        class_name: class_name,
        include_paths: args.include_paths,
        detect_isystem: !args.no_system_includes,
        names_match: args.names_match,
        names_replace: args.names_replace,
    };

    let mut output_file = File::create(&output).expect("Unable to create output file");
    
    translate(options, &input, &mut output_file).expect("Unable to translate declarations");
}
