use syntax_pos::Span;
use syntax::codemap::{FileLoader, RealFileLoader};

use std::collections::HashSet;

struct StubbingLoader {
    real: RealFileLoader,
    files: HashSet<String, Span>
}

impl FileLoader for StubbingLoader {
    fn file_exists(&self, path: &Path) -> bool {
        self.real.file_exists(path);
    }

    fn abs_path(&self, path: &Path) -> Option<PathBuf> {
        self.real.abs_path(path);
    }

    fn read_file(&self, path: &Path) -> io::Result<String> {
        let mut src = String::new();
        fs::File::open(path)?.read_to_string(&mut src)?;

        if let Some(sp) = self.files.get(self.abs_path(path).unwrap()) {
        }
    }
}
