use rustc_lint;
use rustc_driver::{self, driver, target_features, abort_on_err, RustcDefaultCalls};
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
use rustc_driver::driver::CompileController;

use std::default::Default;

use rustc::session::search_paths::SearchPaths;
use rustc::session::config::{ErrorOutputType, RustcOptGroup, nightly_options, Externs, Input};

use rustc_driver::diagnostics_registry;
use rustc_driver::handle_options;
use rustc::session::CompileResult;
use rustc_driver::Compilation;
use syntax::codemap::FileLoader;
use rustc_driver::CompilerCalls;

use rustc_errors::Handler;
use rustc_errors::emitter::Emitter;
use rustc_errors::{Diagnostic, DiagnosticBuilder, HandlerFlags};

use syntax;
use syntax::ast::NodeId;
use syntax::codemap::{self, CodeMap};
use rustc::session::CompileIncomplete;

use getopts;

use std;
use std::cell::{RefCell, Cell};
use std::mem;
use rustc_data_structures::sync::Lrc;
use std::rc::Rc;
use std::ops::Deref;
use std::collections::{BTreeMap, BTreeSet};

use syntax::codemap::RealFileLoader;
use rustc_driver::get_trans;
use rustc_trans_utils::trans_crate::TransCrate;
use std::path::PathBuf;
use std::process::Command;
use syntax::ast;

use syntax::feature_gate::{GatedCfg, UnstableFeatures};
use syntax::parse::{self, PResult};
use syntax_pos::{DUMMY_SP, MultiSpan, FileName};

use std::io;
use std::io::Read;

use ast_extract::{self, FnInfo, FnMap};

pub struct ExtractionResult {
    pub fns: FnMap
}

pub fn run_compiler<'a>(args: &[String],
                        callbacks: &mut CompilerCalls<'a>,
                        file_loader: Option<Box<FileLoader + Send + Sync + 'static>>,
                        emitter: Box<Emitter>) -> Option<ExtractionResult>
{
    syntax::with_globals(|| {
        run_compiler_impl(args, callbacks, file_loader, emitter)
    })
}

fn run_compiler_impl<'a>(args: &[String],
                         callbacks: &mut CompilerCalls<'a>,
                         file_loader: Option<Box<FileLoader + Send + Sync + 'static>>,
                         emitter: Box<Emitter>) -> Option<ExtractionResult>
{
    macro_rules! do_or_return {($expr: expr) => {
        match $expr {
            Compilation::Stop => return None,
            Compilation::Continue => {}
        }
    }}


    let matches = match handle_options(args) {
        Some(matches) => matches,
        None => return None
    };

    let (sopts, cfg) = config::build_session_options_and_crate_config(&matches);

    let descriptions = diagnostics_registry();

    do_or_return!(callbacks.early_callback(&matches,
                                           &sopts,
                                           &cfg,
                                           &descriptions,
                                           sopts.error_format));

    let (odir, ofile) = make_output(&matches);
    let (input, input_file_path, input_err) = match make_input(&matches.free) {
        Some((input, input_file_path, input_err)) => {
            let (input, input_file_path) = callbacks.some_input(input, input_file_path);
            (input, input_file_path, input_err)
        },
        None => match callbacks.no_input(&matches, &sopts, &cfg, &odir, &ofile, &descriptions) {
            Some((input, input_file_path)) => (input, input_file_path, None),
            None => unreachable!()
        },
    };

    let loader = file_loader.unwrap_or(Box::new(RealFileLoader));
    let codemap = Lrc::new(CodeMap::with_file_loader(loader, sopts.file_path_mapping()));
    let mut sess = session::build_session_with_codemap(
        sopts, input_file_path.clone(), descriptions, codemap, None,
    );

    sess.parse_sess.span_diagnostic = Handler::with_emitter(false, false, emitter);

    if let Some(err) = input_err {
        // Immediately stop compilation if there was an issue reading
        // the input (for example if the input stream is not UTF-8).
        sess.err(&format!("{}", err));
        panic!("{}", err);
    }

    let trans = get_trans(&sess);

    rustc_lint::register_builtins(&mut sess.lint_store.borrow_mut(), Some(&sess));

    let mut cfg = config::build_configuration(&sess, cfg);
    target_features::add_configuration(&mut cfg, &sess, &*trans);
    sess.parse_sess.config = cfg;

    let plugins = sess.opts.debugging_opts.extra_plugins.clone();

    let cstore = CStore::new(trans.metadata_loader());

    do_or_return!(callbacks.late_callback(&*trans,
                                          &matches,
                                          &sess,
                                          &cstore,
                                          &input,
                                          &odir,
                                          &ofile));

    let control = callbacks.build_controller(&sess, &matches);

    Some(extract_fns(trans,
                           &sess,
                           &cstore,
                           &input_file_path,
                           &input,
                           &odir,
                           &ofile,
                           Some(plugins),
                           &control))
}

fn extract_fns(trans: Box<TransCrate>,
                     sess: &Session,
                     cstore: &CStore,
                     input_path: &Option<PathBuf>,
                     input: &Input,
                     outdir: &Option<PathBuf>,
                     output: &Option<PathBuf>,
                     addl_plugins: Option<Vec<String>>,
                     control: &CompileController) -> ExtractionResult {

    let code_map = sess.codemap();

    let krate = driver::phase_1_parse_input(control, &sess, &input).unwrap();

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
        expanded_crate,
        mut hir_forest,
        resolver,
        ..
    } = abort_on_err(result, &sess);

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
                                                     control,
                                                     &sess,
                                                     &*cstore,
                                                     hir_map,
                                                     analysis,
                                                     resolutions,
                                                     &arenas,
                                                     &name,
                                                     &output_filenames,
                                                     |tcx, analysis, _, result| {
	});

    let mut fn_map = ast_extract::get_function_info(code_map, &expanded_crate);
    for fns in fn_map.values_mut() {
        fns.sort_unstable();
    }

    ExtractionResult { fns: fn_map }

}
 

fn make_output(matches: &getopts::Matches) -> (Option<PathBuf>, Option<PathBuf>) {
    let odir = matches.opt_str("out-dir").map(|o| PathBuf::from(&o));
    let ofile = matches.opt_str("o").map(|o| PathBuf::from(&o));
    (odir, ofile)
}

// Extract input (string or file and optional path) from matches.
fn make_input(free_matches: &[String]) -> Option<(Input, Option<PathBuf>, Option<io::Error>)> {
    if free_matches.len() == 1 {
        let ifile = &free_matches[0];
        if ifile == "-" {
            let mut src = String::new();
            let err = if io::stdin().read_to_string(&mut src).is_err() {
                Some(io::Error::new(io::ErrorKind::InvalidData,
                                    "couldn't read from stdin, as it did not contain valid UTF-8"))
            } else {
                None
            };
            Some((Input::Str { name: FileName::Anon, input: src },
                  None, err))
        } else {
            Some((Input::File(PathBuf::from(ifile)),
                  Some(PathBuf::from(ifile)), None))
        }
    } else {
        None
    }
}
