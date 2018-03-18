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

use std::process::Command;
use std::io::Write;
use std::fs::File;


fn main() {

    let mut out = File::create("my_args.txt").unwrap();
    out.write_all(std::env::args().collect::<Vec<String>>().join(" ").as_bytes()).unwrap();

    let sysroot = Command::new("rustc")
                .arg("--print")
                .arg("sysroot")
                .output()
                .ok()
                .and_then(|out| String::from_utf8(out.stdout).ok())
                .map(|s| s.trim().to_owned()).unwrap();


    let mut args: Vec<_> = std::env::args().skip(1).collect();

    args.push("--sysroot".to_owned());
    args.push(sysroot);


    //eprintln!("Starting");
    if let Some(fns) = get_errors::get_erroring_functions(&args) {
        stub_fn::compile_stubbed(fns, args.clone())
    }

}
