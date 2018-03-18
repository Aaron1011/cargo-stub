#![feature(rustc_private)]
#![allow(warnings)]

extern crate rustc;
extern crate rustc_lint;
extern crate rustc_resolve;
extern crate rustc_metadata;
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_data_structures;
extern crate rustc_trans_utils;
extern crate syntax;
extern crate syntax_pos;

extern crate getopts;

mod run_compile;
mod get_errors;
mod ast_extract;
mod stub_fn;


fn main() {
    eprintln!("Starting");
    let fns = get_errors::get_erroring_functions();
    stub_fn::compile_stubbed(fns);
}
