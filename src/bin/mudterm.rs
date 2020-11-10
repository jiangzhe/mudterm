use gag::Redirect;
use mudterm::app::{App, client, server, standalone::{Standalone, StandaloneCallback}};
use mudterm::conf::{CmdOpts, Config};
use mudterm::error::{Error, Result};
use std::fs::File;
use std::io::Read;
use std::net::TcpStream;
use std::path::Path;
use structopt::StructOpt;
use crossbeam_channel::unbounded;

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
    let _stderr_redirect = Redirect::stderr(debuglog)
        .map_err(|e| Error::RuntimeError(format!("Redirect stderr error {}", e)))?;
    eprintln!("starting mudterm in {:?} mode", config.mode);

    // connect to mud server
    let world_addr = &config.world.addr;
    let (from_mud, to_mud) = {
        let from_mud = TcpStream::connect(world_addr)?;
        let to_mud = from_mud.try_clone()?;
        (from_mud, to_mud)
    };
    eprintln!("connecting to world {}", world_addr);

    // create standalone app
    let mut app = Standalone::with_config(config);
    // load triggers if exists
    // app.load_triggers()?;
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

/// standalone app
pub fn standalone(config: Config) -> Result<()> {

    let world_addr = config.world.addr.clone();
    let (evttx, evtrx) = unbounded();
    // 1. init app
    eprintln!("initilizing app with config");
    let mut app = App::with_config(config, evttx.clone());

    // 2. connect to mud
    eprintln!("connecting to world {}", world_addr);
    let (from_mud, to_mud) = {
        let from_mud = TcpStream::connect(world_addr)?;
        let to_mud = from_mud.try_clone()?;
        (from_mud, to_mud)
    };
    
    // 3. start io threads
    eprintln!("starting thread handling message to mud server");
    let worldtx = server::start_to_mud_handle(to_mud)?;
    eprintln!("starting thread handling message from mud server");
    let worldrx = server::start_from_mud_handle(evttx.clone(), from_mud);

    // 4. start userinput thread
    eprintln!("starting thread handling keyboard and mouse events");
    let _ = client::start_userinput_handle(evttx.clone());

    // 5. start signal thread
    eprintln!("starting thread handling window resize");
    let _ = client::start_signal_handle(evttx.clone());

    // 6. start ui thread
    eprintln!("starting thread handling user intergface");
    let cb = StandaloneCallback::new(evttx.clone());
    let (uitx, uihandle) = client::start_ui_handle(app.termconf(), cb)?;

    // 7. run event loop on main thread

    Ok(())
}
