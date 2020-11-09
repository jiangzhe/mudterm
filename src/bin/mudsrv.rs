use gag::Redirect;
use mudterm::app::server::Server;
use mudterm::conf::{CmdOpts, Config};
use mudterm::error::{Error, Result};
use std::fs::File;
use std::io::Read;
use std::net::{TcpListener, TcpStream};
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
    let _stderr_redirect = Redirect::stderr(debuglog)
        .map_err(|e| Error::RuntimeError(format!("Redirect stderr error {}", e)))?;

    // connect to server
    let (from_mud, to_mud) = {
        let from_mud = TcpStream::connect("mud.pkuxkx.net:8080")?;
        let to_mud = from_mud.try_clone()?;
        (from_mud, to_mud)
    };

    // listen to local port
    let listener = TcpListener::bind(format!("0.0.0.0:{}", config.server.port))?;

    // create server app
    let mut app = Server::with_config(config);

    app.start_from_mud_handle(from_mud);

    app.start_to_mud_handle(to_mud)?;

    app.start_server_listener_handle(listener);

    app.main_loop()?;

    Ok(())
}
