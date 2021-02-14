//! "Traversing Mole" server

//! Crate docs

#![forbid(unsafe_code)]
#![deny(
    missing_docs,
    unstable_features,
    missing_debug_implementations,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unused_import_braces,
    unused_qualifications,
    bad_style,
    const_err,
    dead_code,
    improper_ctypes,
    non_shorthand_field_patterns,
    no_mangle_generic_items,
    overflowing_literals,
    path_statements,
    patterns_in_fns_without_body,
    private_in_public,
    unconditional_recursion,
    unused,
    unused_allocation,
    unused_comparisons,
    unused_parens,
    while_true,
    missing_debug_implementations,
    missing_docs,
    trivial_casts,
    trivial_numeric_casts,
    unused_extern_crates,
    unused_import_braces,
    unused_qualifications
)]
// For development:
// #![allow(dead_code, unused_imports, unused_variables, deprecated)]


use actix::prelude::*;
use chrono::*;
use fern::{
    colors::{Color, ColoredLevelConfig},
    Dispatch,
};
use krecik::{
    actors::{
        curl_multi_checker::{Checks, CurlMultiChecker},
        curl_multi_checker_pongo::Checks as ChecksPongo,
        curl_multi_checker_pongo::CurlMultiCheckerPongo,
        history_teacher::{HistoryTeacher, Results},
        results_warden::ResultsWarden,
    },
    api::*,
    *,
};
use lazy_static::lazy_static;
use log::*;
use std::sync::RwLock;

lazy_static! {
    static ref LOG_LEVEL: RwLock<LevelFilter> = RwLock::new(LevelFilter::Info);
}


/// Set log level dynamically at runtime
fn set_log_level() {
    let level = Config::load().get_log_level();
    match LOG_LEVEL.read() {
        Ok(loglevel) => {
            if level != *loglevel {
                drop(loglevel);
                match LOG_LEVEL.write() {
                    Ok(mut log) => {
                        println!("Changing log level to: {}", level);
                        *log = level
                    }
                    Err(err) => {
                        eprintln!("Failed to change log level to: {}, cause: {}", level, err);
                    }
                }
            }
        }
        Err(err) => {
            eprintln!("Couldn't read LOG_LEVEL, cause: {:?}", err);
        }
    }
}


/// Initial setup of the fern logger
fn setup_logger() -> Result<(), SetLoggerError> {
    let log_file = Config::load()
        .log_file
        .unwrap_or_else(|| String::from(DEFAULT_LOG_FILE));
    let colors_line = ColoredLevelConfig::new()
        .error(Color::Red)
        .warn(Color::Yellow)
        .info(Color::White)
        .debug(Color::Magenta)
        .trace(Color::Cyan);
    Dispatch::new()
        .filter(|metadata| {
            match LOG_LEVEL.read() {
                Ok(log) => metadata.level() <= *log,
                Err(_err) => true,
            }
        })
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{color_line}[{date}][{target}][{level}{color_line}] {message}\x1B[0m",
                color_line = format_args!(
                    "\x1B[{}m",
                    colors_line.get_color(&record.level()).to_fg_str()
                ),
                date = Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                target = record.target(),
                level = record.level(),
                message = message
            ))
        })
        .chain(std::io::stdout())
        .chain(fern::DateBased::new(format!("{}.", log_file), "%Y-%m-%d"))
        .apply()
}


#[actix_macros::main]
async fn main() {
    setup_logger().expect("Couldn't initialize logger");
    ctrlc::set_handler(|| {
        println!("\n\nKrecik server was interrupted!");
        std::process::exit(0);
    })
    .expect("Couldn't initialize Ctrl-C handler");

    info!("Starting Krecik-server v{}", env!("CARGO_PKG_VERSION"));

    // Define system actors
    let num = 1;
    let curl_multi_checker = SyncArbiter::start(num, || CurlMultiChecker);
    let curl_multi_checker_pongo = SyncArbiter::start(num, || CurlMultiCheckerPongo);
    let history_teacher = SyncArbiter::start(num, || HistoryTeacher);
    let results_warden = SyncArbiter::start(num, || ResultsWarden);
    let notificator = SyncArbiter::start(num, || Notificator);

    loop {
        set_log_level();
        debug!("New execution iteration…");

        let start = Local::now();

        let pongo_checks = &curl_multi_checker_pongo
            .send(ChecksPongo(all_checks_pongo_merged()))
            .await;

        let regular_checks = curl_multi_checker
            .send(Checks(all_checks_but_remotes()))
            .await;
        let stories = [
            pongo_checks.clone().unwrap().unwrap_or_default(),
            regular_checks.unwrap().unwrap_or_default(),
        ]
        .concat();

        let end = Local::now();
        let diff = end - start;

        warn_for_undefined_notifiers(&stories);

        info!(
            "Remote checks took: {}s. Result stories count: {}.",
            diff.num_seconds(),
            stories.len(),
        );

        debug!("Sending results to HistoryTeacher…");
        history_teacher
            .send(Results(
                stories,
                results_warden.clone(),
                notificator.clone(),
            ))
            .await
            .unwrap_or_default();
    }
}
