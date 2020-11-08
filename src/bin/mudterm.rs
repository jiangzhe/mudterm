use mudterm::error::{Result, Error};
use mudterm::conf::{CmdOpts, Config};
use mudterm::app::standalone::Standalone;
use std::fs::File;
use gag::Redirect;
use std::net::TcpStream;
use std::path::Path;
use std::io::Read;
use structopt::StructOpt;

fn main() -> Result<()> {
    let cmdopts = CmdOpts::from_args();

    if !Path::new(&cmdopts.conf_file).exists() {
        return Err(Error::RuntimeError(format!("config file {} not found", &cmdopts.conf_file)));
    }
    let config: Config = {
        let mut f = File::open(&cmdopts.conf_file)?;
        let mut toml_str = String::new();
        f.read_to_string(&mut toml_str)?;
        toml::from_str(&toml_str)?
    };

    // redirect stderr to file
    let debuglog = File::create(&config.server.debug_file)?;
    let _stderr_redirect = Redirect::stderr(debuglog)
        .map_err(|e| Error::RuntimeError(format!("Redirect stderr error {}", e)))?;

    // connect to mud server 
    let (from_mud, to_mud) = {
        let from_mud = TcpStream::connect("mud.pkuxkx.net:8080")?;
        let to_mud = from_mud.try_clone()?;
        (from_mud, to_mud)  
    };

    // create standalone app
    let mut app = Standalone::with_config(config);
    // load triggers if exists
    app.load_triggers()?;
    // events from server and logging
    app.start_from_mud_handle(from_mud);
    // events to server
    app.start_to_mud_handle(to_mud)?;
    // user input events
    app.start_userinput_handle();
    // signal events
    app.start_signal_handle();
    // ui events and rendering
    app.start_ui_handle()?;
    // main loop to handle all events, blocking on current thread
    app.main_loop()?;

    Ok(())
}