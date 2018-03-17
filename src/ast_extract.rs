use syntax::visit::{Visitor, FnKind, walk_crate};
use syntax::ast::{self, FnDecl, NodeId, Crate};
use syntax_pos::Span;
use syntax::codemap::CodeMap;

use std::cmp::Ordering;

struct FunctionExtractor<'a> {
	fns: Vec<FnInfo>,
	code_map: &'a CodeMap
}

#[derive(Debug)]
pub struct FnInfo {
	name: Option<String>,
	lo_line: usize,
	hi_line: usize
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

		self.fns.push(FnInfo { name, lo_line: lo.line, hi_line: hi.line });
	}
}

pub fn get_function_info(code_map: &CodeMap, krate: &Crate) -> Vec<FnInfo> {
	let mut extractor = FunctionExtractor { fns: Vec::new(), code_map };
	walk_crate(&mut extractor, krate);

	extractor.fns
}
