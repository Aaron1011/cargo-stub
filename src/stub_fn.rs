use syntax_pos::Span;
use syntax::codemap::{FileLoader, RealFileLoader};
use rustc_driver::{self, RustcDefaultCalls};

use std;
use std::io::{self, Read};
use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashSet;

use ast_extract::FnMap;

pub fn compile_stubbed(fns: FnMap, args: Vec<String>) {
    let loader = StubbingLoader::new(fns);

    rustc_driver::run(move || rustc_driver::run_compiler(&args, &mut RustcDefaultCalls, Some(Box::new(loader)), None));
}

struct StubbingLoader {
    real: RealFileLoader,
    fns: FnMap
}

impl StubbingLoader {
    fn new(fns: FnMap) -> StubbingLoader {
        StubbingLoader { real: RealFileLoader, fns }
    }
}

impl FileLoader for StubbingLoader {
    fn file_exists(&self, path: &Path) -> bool {
        self.real.file_exists(path)
    }

    fn abs_path(&self, path: &Path) -> Option<PathBuf> {
        self.real.abs_path(path)
    }

    fn read_file(&self, path: &Path) -> io::Result<String> {
        let mut src = String::new();
        fs::File::open(path)?.read_to_string(&mut src)?;

        eprintln!("Testing {:?}", path);
        if let Some(fns) = self.fns.get(path) {

            eprintln!("File has stubbed functions!");

            let mut lines: Vec<String> = Vec::new();
            for line in src.lines() {
                lines.push(line.to_string());
            }

            for f in fns {
                let name = f.name.as_ref().unwrap();

                eprintln!("Stubbing: {:?}", f);
                {
                    // Line numbers from the code map start at 1, so we need
                    // to subtract 1 from lo_line to get the actual line.
                    //
                    // However, the
                    // start of the function span is on the declaration line (e.g. "fn foo()"),
                    // so we need to add 1 from lo_line to get the proper line.
                    //
                    // These two correctiosn cancel out, so we just use lo_lien as-is
                    let start = &mut lines[f.lo_line];
                    start.insert_str(0, &format!("panic!(\"Function {} is stubbed!\")/*", name));
                }

                {
                    // Like with lo_line, we need to adjust for the line numbers starting at 1,
                    // and for the span ending on the closing bracket line. However, these
                    // corrections don't cancel out - both are subtractions - so we subtract 2
                    let end = &mut lines[f.hi_line - 2];
                    end.push_str("*/");
                }
            }

            let out = lines.join("\n");
            eprintln!("Modified file: \n");
            eprint!("{}", out);
            eprintln!("");

            return Ok(out)
        }
        return Ok(src)
    }
}
