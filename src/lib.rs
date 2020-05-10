mod options;
mod result;
mod coder;
mod translator;
mod utils;

use std::{
    path::Path,
    io::Write,
};
use clang::{Clang, Index};

pub use options::*;
pub use result::*;
pub(crate) use coder::*;
pub(crate) use translator::*;
pub(crate) use utils::*;

pub fn translate(options: Options, input: &Path, output: &mut impl Write) -> Result<()> {
    let clang = Clang::new().unwrap();
    
    let index = Index::new(&clang, false, true);
    
    let mut args = Vec::new();
    
    args.push("-xc".into());

    if options.detect_isystem {
        let paths = system_includes_search_paths();
        
        for path in paths {
            args.push(format!("-isystem{}", path.display()));
        }
    }

    for path in &options.include_paths {
        args.push(format!("-I{}", path.display()));
    }

    let tu = index.parser(&input)
        .arguments(&args)
        .parse().unwrap();

    let mut translator = Translator::new(options);

    translator.translate(tu.get_entity());

    writeln!(output,
             "/* This file was generated using {program} v{version} tool and should not be modified manually. */",
             program = env!("CARGO_PKG_NAME"),
             version = env!("CARGO_PKG_VERSION"))?;
    
    writeln!(output, "{}", translator.coder())?;

    Ok(())
}
