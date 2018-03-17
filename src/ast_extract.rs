use syntax::visit::{Visitor, FnKind, walk_crate};
use syntax::ast::{self, FnDecl, NodeId, Crate};
use syntax_pos::{Span, FileName};
use syntax::codemap::CodeMap;

use std::collections::HashMap;
use std::cmp::Ordering;
use std::path::PathBuf;

pub type FnMap = HashMap<PathBuf, Vec<FnInfo>>;

struct FunctionExtractor<'a> {
	pub fns: FnMap,
	pub code_map: &'a CodeMap
}

#[derive(Debug, Clone)]
pub struct FnInfo {
	pub name: Option<String>,
    pub file: PathBuf,
	pub lo_line: usize,
	pub hi_line: usize
}

impl PartialEq for FnInfo {
	fn eq(&self, other: &FnInfo) -> bool {
		self.lo_line == other.hi_line
	}
}

impl Eq for FnInfo {}

impl Ord for FnInfo {
	fn cmp(&self, other: &FnInfo) -> Ordering {
		self.lo_line.cmp(&other.lo_line)
	}
}

impl PartialOrd for FnInfo {
	fn partial_cmp(&self, other: &FnInfo) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a, 'ast> Visitor<'ast> for FunctionExtractor<'a> {

	fn visit_fn(&mut self, fk: FnKind<'ast>, fd: &'ast FnDecl, sp: Span, _: NodeId) {
		let lo = self.code_map.lookup_char_pos(sp.lo());
        let hi = self.code_map.lookup_char_pos(sp.hi());

		let name = match fk {
			FnKind::ItemFn(name, _, _, _, _, _)  | FnKind::Method(name, _, _, _) => {
				Some(format!("{}", name))
			},
			_ => None
		};

		let file = match self.code_map.span_to_filename(sp) {
			FileName::Real(f) => f,
			_ => panic!("Unexpected path for {:?}", sp)
		};

		self.fns.entry(file.clone()).or_insert_with(|| Vec::new()).push(FnInfo { name, file, lo_line: lo.line, hi_line: hi.line });
	}
}

pub fn get_function_info(code_map: &CodeMap, krate: &Crate) -> FnMap {
	let mut extractor = FunctionExtractor { fns: HashMap::new(), code_map };
	walk_crate(&mut extractor, krate);

	extractor.fns
}
