use rustc_lint;
use rustc_driver::{self, driver, target_features, abort_on_err, RustcDefaultCalls, Compilation, get_trans};
use rustc::session::{self, config, Session};
use rustc::hir::def_id::{DefId, CrateNum};
use rustc::hir::def::Def;
use rustc::middle::cstore::CrateStore;
use rustc::middle::privacy::AccessLevels;
use rustc::ty::{self, TyCtxt, AllArenas};
use rustc::hir::map as hir_map;
use rustc::lint;
use rustc::util::nodemap::{FxHashMap, FxHashSet};
use rustc_resolve as resolve;
use rustc_metadata::creader::CrateLoader;
use rustc_metadata::cstore::CStore;
use rustc_trans_utils;
use std::default::Default;
use rustc::session::config::Input;
use rustc;

use rustc::session::search_paths::SearchPaths;
use rustc::session::config::{ErrorOutputType, RustcOptGroup, nightly_options, Externs};

use rustc_errors::{self, Handler};
use rustc_errors::emitter::Emitter;
use rustc_errors::{Diagnostic, DiagnosticBuilder, HandlerFlags};

use rustc_trans_utils::trans_crate::TransCrate;
use rustc_driver::CompilerCalls;

use syntax;
use syntax::ast::NodeId;
use syntax::ast;
use syntax::codemap;
use syntax::feature_gate::UnstableFeatures;

use getopts;

use std;
use std::cell::{RefCell, Cell};
use std::mem;
use rustc_data_structures::sync::Lrc;
use std::rc::Rc;
use std::path::PathBuf;
use std::ops::Deref;
use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

use run_compile;

struct CargoStubCallbacks {
    default: RustcDefaultCalls,
}

impl CargoStubCallbacks {
    fn new() -> CargoStubCallbacks {
        CargoStubCallbacks { default: RustcDefaultCalls }
    }
}

impl<'a> CompilerCalls<'a> for CargoStubCallbacks {
    fn early_callback(
        &mut self,
        matches: &getopts::Matches,
        sopts: &config::Options,
        cfg: &ast::CrateConfig,
        descriptions: &rustc_errors::registry::Registry,
        output: ErrorOutputType,
    ) -> Compilation {
        self.default
            .early_callback(matches, sopts, cfg, descriptions, output)
    }
    fn no_input(
        &mut self,
        matches: &getopts::Matches,
        sopts: &config::Options,
        cfg: &ast::CrateConfig,
        odir: &Option<PathBuf>,
        ofile: &Option<PathBuf>,
        descriptions: &rustc_errors::registry::Registry,
    ) -> Option<(Input, Option<PathBuf>)> {
        self.default
            .no_input(matches, sopts, cfg, odir, ofile, descriptions)
    }
    fn late_callback(
        &mut self,
        trans_crate: &TransCrate,
        matches: &getopts::Matches,
        sess: &Session,
        crate_stores: &rustc::middle::cstore::CrateStore,
        input: &Input,
        odir: &Option<PathBuf>,
        ofile: &Option<PathBuf>,
    ) -> Compilation {
        eprintln!("Late callback!");
        self.default
            .late_callback(trans_crate, matches, sess, crate_stores, input, odir, ofile)
    }
    fn build_controller(&mut self, sess: &Session, matches: &getopts::Matches) -> driver::CompileController<'a> {
        let mut control = self.default.build_controller(sess, matches);
        control
    }
}


#[derive(Debug)]
struct ErrorCollector {
    errors: Rc<RefCell<Vec<Diagnostic>>>
}

impl ErrorCollector {
    fn new() -> ErrorCollector {
        ErrorCollector { errors: Default::default() }
    }

    fn dup(&self) -> ErrorCollector {
        ErrorCollector {
            errors: self.errors.clone()
        }
    }
}

impl Emitter for ErrorCollector {
    fn emit(&mut self, db: &DiagnosticBuilder) {
        println!("Emitting: {:?}", db);
        self.errors.borrow_mut().push(db.deref().clone());
    }
}


pub fn get_all_errors() {

    let mut args: Vec<_> = std::env::args().skip(1).collect();
    
    let mut callbacks = CargoStubCallbacks::new();
    let collector = ErrorCollector::new();

    let sysroot = Command::new("rustc")
                .arg("--print")
                .arg("sysroot")
                .output()
                .ok()
                .and_then(|out| String::from_utf8(out.stdout).ok())
                .map(|s| s.trim().to_owned()).unwrap();

    args.push("--sysroot".to_owned());
    args.push(sysroot);


    run_compile::run_compiler(&args, &mut callbacks, None, Box::new(collector.dup()));

/*    let handler = Handler::with_emitter_and_flags(Box::new(collector.dup()), HandlerFlags {
        can_emit_warnings: false,
        treat_err_as_bug: false,
        external_macro_backtrace: false
    });*/





    /*let sessopts = config::Options {
        maybe_sysroot: None,
        search_paths: SearchPaths::new(),
        crate_types: vec![config::CrateTypeRlib],
        lint_opts: Vec::new(),
        lint_cap: Some(lint::Allow),
        externs: Externs::new(BTreeMap::new()), // TODO
        target_triple: config::host_triple().to_string(),
        // Ensure that rustdoc works even if rustc is feature-staged
        unstable_features: UnstableFeatures::Allow,
        actually_rustdoc: true,
        debugging_opts: config::basic_debugging_options(),
        ..config::basic_options().clone()
    };*/

    /*
    let codemap = Lrc::new(codemap::CodeMap::new(sessopts.file_path_mapping()));

    let mut sess = session::build_session_(
        sessopts, None, handler, codemap,
    );


    let trans = rustc_driver::get_trans(&sess);
    let cstore = Rc::new(CStore::new(trans.metadata_loader()));

    let (odir, ofile) = make_output(&matches);

    match RustcDefaultCalls::print_crate_info(trans, sess, Some(input), odir, ofile)
         .and_then(|| RustcDefaultCalls::list_metadata(sess, cstore, matches, input)){
    }

    rustc_lint::register_builtins(&mut sess.lint_store.borrow_mut(), Some(&sess));


    /*let mut cfg = config::build_configuration(&sess, Default::default());*/
    target_features::add_configuration(&mut cfg, &sess, &*trans);
    sess.parse_sess.config = cfg;


    let mut control = driver::CompileController::basic();
    control.continue_parse_after_error = true;


    let krate = driver::phase_1_parse_input(&control, &sess, &input).expect("Wtf");

    let name = rustc_trans_utils::link::find_crate_name(Some(&sess), &krate.attrs, &input);

    let mut crate_loader = CrateLoader::new(&sess, &cstore, &name);

    let resolver_arenas = resolve::Resolver::arenas();
    let result = driver::phase_2_configure_and_expand_inner(&sess,
                                                      &cstore,
                                                      krate,
                                                      None,
                                                      &name,
                                                      None,
                                                      resolve::MakeGlobMap::No,
                                                      &resolver_arenas,
                                                      &mut crate_loader,
                                                      |_| Ok(()));
    let driver::InnerExpansionResult {
        mut hir_forest,
        resolver,
        ..
    } = result.unwrap();

       // We need to hold on to the complete resolver, so we clone everything
    // for the analysis passes to use. Suboptimal, but necessary in the
    // current architecture.
    let defs = resolver.definitions.clone();
    let resolutions = ty::Resolutions {
        freevars: resolver.freevars.clone(),
        export_map: resolver.export_map.clone(),
        trait_map: resolver.trait_map.clone(),
        maybe_unused_trait_imports: resolver.maybe_unused_trait_imports.clone(),
        maybe_unused_extern_crates: resolver.maybe_unused_extern_crates.clone(),
    };
    let analysis = ty::CrateAnalysis {
        access_levels: Lrc::new(AccessLevels::default()),
        name: name.to_string(),
        glob_map: if resolver.make_glob_map { Some(resolver.glob_map.clone()) } else { None },
    };

    let arenas = AllArenas::new();
    let hir_map = hir_map::map_crate(&sess, &*cstore, &mut hir_forest, &defs);
    let output_filenames = driver::build_output_filenames(&input,
                                                          &None,
                                                          &None,
                                                          &[],
                                                          &sess);

    let resolver = RefCell::new(resolver);


    driver::phase_3_run_analysis_passes(&*trans,
                                                     &control,
                                                     &sess,
                                                     &*cstore,
                                                     hir_map,
                                                     analysis,
                                                     resolutions,
                                                     &arenas,
                                                     &name,
                                                     &output_filenames,
                                                     |tcx, analysis, _, result| {

        println!("Compilation done! Collector: {:?}", collector);
    }).unwrap();*/

    eprintln!("All done for real");
}
