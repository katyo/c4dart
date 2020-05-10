use std::{
    path::PathBuf,
    str::from_utf8,
    process::{Command, Stdio},
};

pub fn system_includes_search_paths() -> Vec<PathBuf> {
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
