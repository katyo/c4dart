use std::borrow::Cow;
use std::fmt::{Display, Formatter, Result as FmtResult};

#[derive(Debug, Clone, Default)]
pub struct Coder {
    units: Chunks,
}

impl Coder {
    /// Append code line
    pub fn line(&mut self, src: impl Into<String>) {
        self.units.push(Chunk::Line(src.into()));
    }

    /// Append code block
    pub fn block(&mut self, src: impl Into<String>, blk: impl FnOnce(&mut Coder)) {
        let mut cg = Coder::default();
        blk(&mut cg);
        self.units.push(Chunk::Block(src.into(), cg.units));
    }

    /// Append comment
    pub fn comment(&mut self, src: impl AsRef<str>) {
        self.units.push(Chunk::Comment(unroll_comment(src.as_ref()).into()));
    }

    /// Format output
    pub fn format(&self, f: &mut Formatter, l: usize) -> FmtResult {
        for src in &self.units {
            src.format(f, l)?;
        }
        Ok(())
    }
}

impl Display for Coder {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        self.format(f, 0)
    }
}

#[derive(Debug, Clone)]
enum Chunk {
    Line(String),
    Block(String, Chunks),
    Comment(String),
}

impl Chunk {
    pub fn format(&self, f: &mut Formatter, l: usize) -> FmtResult {
        use Chunk::*;
        
        let indent = l * 4;
        match self {
            Line(src) => writeln!(f, "{:indent$}{}", "", src, indent = indent),
            Block(src, units) => if units.is_empty() {
                writeln!(f, "{:indent$}{} {{}}", "", src, indent = indent)
            } else {
                writeln!(f, "{:indent$}{} {{", "", src, indent = indent)?;
                for src in units {
                    src.format(f, l + 1)?;
                }
                writeln!(f, "{:indent$}}}", "", indent = indent)
            },
            Comment(src) => {
                write!(f, "{:indent$}/*", "", indent = indent)?;
                let mut lines = src.lines();
                if let Some(line) = lines.next() {
                    writeln!(f, "{}", line)?;
                    for line in lines {
                        writeln!(f, "{:indent$} {}", "", line, indent = indent)?;
                    }
                }
                writeln!(f, "{:indent$} */", "", indent = indent)
            },
        }
    }
}

type Chunks = Vec<Chunk>;

impl Display for Chunk {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        self.format(f, 0)
    }
}

fn unroll_comment(src: &str) -> Cow<'_, str> {
    let src = src.trim();

    let src = if src.starts_with("//") {
        &src[2..]
    } else if src.starts_with("/*") && src.ends_with("*/") && src.len() > 3 {
        &src[2..src.len()-2]
    } else {
        src
    };

    let src = src.trim();

    if src.find('\n').is_some() {
        let initial_spaces = src.lines().skip(1)
            .map(|line| line.chars().take_while(|c| c.is_whitespace()).count())
            .min().unwrap_or(0);
        
        src.lines().enumerate().map(|(n, line)| if n > 0 {
            &line[initial_spaces..]
        } else {
            line
        }).collect::<Vec<_>>().join("\n").into()
    } else {
        src.into()
    }
}
