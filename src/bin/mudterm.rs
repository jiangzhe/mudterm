use gag::Redirect;
use mudterm::app;
use mudterm::conf::{CmdOpts, Config, Mode};
use mudterm::error::{Error, Result};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use structopt::StructOpt;

fn main() -> Result<()> {
    let cmdopts = CmdOpts::from_args();

    if !Path::new(&cmdopts.conf_file).exists() {
        return Err(Error::RuntimeError(format!(
            "config file {} not found",
            &cmdopts.conf_file
        )));
    }

    let config: Config = {
        let mut f = File::open(&cmdopts.conf_file)?;
        let mut toml_str = String::new();
        f.read_to_string(&mut toml_str)?;
        toml::from_str(&toml_str)?
    };

    // redirect stderr to file
    let debuglog = File::create(&config.server.debug_file)?;
    let _stderr_redirect = Redirect::stderr(debuglog).unwrap();
    let verbosity = match &cmdopts.log_level[..] {
        "error" => 0,
        "warn" => 1,
        "info" => 2,
        "debug" => 3,
        "trace" => 4,
        _ => 2,
    };
    stderrlog::new()
        .module(module_path!())
        .verbosity(verbosity)
        .timestamp(stderrlog::Timestamp::Millisecond)
        .init()
        .unwrap();

    log::info!("starting mudterm in {:?} mode", config.mode);

    match config.mode {
        Mode::Standalone => app::standalone(config),
        Mode::Server => app::server(config),
        Mode::Client => app::client(config),
    }
}
