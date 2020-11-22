pub mod client;
pub mod server;
pub mod standalone;

use crate::auth;
use crate::conf::Config;
use crate::error::Result;
use crate::event::EventLoop;
use crate::runtime::Runtime;
use client::{Client, QuitClient};
use crossbeam_channel::unbounded;
use server::{QuitServer, Server};
use standalone::{QuitStandalone, Standalone};
use std::fs::File;
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

/// standalone app
pub fn standalone(config: Config) -> Result<()> {
    let (evttx, evtrx) = unbounded();
    // let world_addr = config.world.addr.clone();
    let serverlog = File::create(&config.server.log_file)?;

    // 1. init runtime
    log::info!("initilizing runtime with config");
    let mut rt = Runtime::new(evttx.clone(), &config);
    rt.set_logger(serverlog);
    rt.init()?;

    // 2. connect to mud
    log::info!("connecting to world {}", &config.world.addr);
    let (from_mud, to_mud) = {
        let from_mud = server::connect_world(&config.world.addr, Duration::from_secs(3))?;
        let to_mud = from_mud.try_clone()?;
        (from_mud, to_mud)
    };

    // 3. start io threads
    log::info!("starting thread handling message to mud server");
    let worldtx = server::start_to_mud_handle(evttx.clone(), to_mud);
    log::info!("starting thread handling message from mud server");
    server::start_from_mud_handle(evttx.clone(), from_mud);

    // 4. start userinput thread
    log::info!("starting thread handling keyboard and mouse events");
    let _ = client::start_userinput_handle(evttx.clone());

    // 5. start signal thread
    log::info!("starting thread handling window resize");
    let _ = client::start_signal_handle(evttx.clone());

    // 6. start ui thread
    log::info!("starting thread handling user interface");
    let (uitx, uihandle) = client::start_ui_handle(config.term, evttx)?;

    // 7. run event loop on main thread
    let standalone_handler = Standalone::new(uitx, worldtx);
    let quit_handler = QuitStandalone::new(uihandle);
    let eventloop = EventLoop::new(rt, evtrx, standalone_handler, quit_handler);
    eventloop.run()?;

    Ok(())
}

/// client app
pub fn client(config: Config) -> Result<()> {
    let (evttx, evtrx) = unbounded();
    let server_addr = config.client.server_addr.clone();
    let server_pass = config.client.server_pass.clone();
    let clientlog = File::create(&config.client.log_file)?;

    // 1. init runtime
    log::info!("initilizing runtime with config");
    let mut rt = Runtime::new(evttx.clone(), &config);
    rt.set_logger(clientlog);
    rt.init()?;

    // 2. connect to server
    log::info!("connecting to server {}", server_addr);
    let (from_server, to_server) = {
        let from_server = TcpStream::connect(&server_addr)?;
        let from_server = auth::client_auth(from_server, &server_pass)?;
        let to_server = from_server.try_clone()?;
        (from_server, to_server)
    };

    // 3. start io threads
    log::info!("starting thread handling message to mudterm server");
    let srvtx = client::start_to_server_handle(to_server);
    log::info!("starting thread handling message from mudterm server");
    client::start_from_server_handle(evttx.clone(), from_server);

    // 4. start userinput thread
    log::info!("starting thread handling keyboard and mouse events");
    let _ = client::start_userinput_handle(evttx.clone());

    // 5. start signal thread
    log::info!("starting thread handling window resize");
    let _ = client::start_signal_handle(evttx.clone());

    // 6. start ui thread
    log::info!("starting thread handling user interface");
    let (uitx, uihandle) = client::start_ui_handle(config.term, evttx)?;

    // 7. run event loop on main thread
    let client_handler = Client::new(uitx, srvtx);
    let quit_handler = QuitClient::new(uihandle);
    let eventloop = EventLoop::new(rt, evtrx, client_handler, quit_handler);
    eventloop.run()?;

    Ok(())
}

/// server app
pub fn server(config: Config) -> Result<()> {
    let (evttx, evtrx) = unbounded();
    let world_addr = config.world.addr.clone();
    let server_port = config.server.port;
    let pass = config.server.pass.clone();
    let init_max_lines = config.server.client_init_max_lines;
    let serverlog = File::create(&config.server.log_file)?;

    // 1. init runtime
    log::info!("initilizing runtime with config");
    let mut rt = Runtime::new(evttx.clone(), &config);
    rt.set_logger(serverlog);
    rt.init()?;

    // 2. connect to mud
    log::info!("connecting to mud server {:?}", world_addr);
    let (from_mud, to_mud) = {
        let from_mud = TcpStream::connect(world_addr)?;
        let to_mud = from_mud.try_clone()?;
        (from_mud, to_mud)
    };

    // 3. start server thread
    log::info!("start thread to bind local port {}", server_port);
    let listener = TcpListener::bind(format!("0.0.0.0:{}", config.server.port))?;
    server::start_server_listener_handle(listener, evttx.clone());

    // 4. start io threads for mud communication
    log::info!("starting thread handling message to mud server");
    let worldtx = server::start_to_mud_handle(evttx.clone(), to_mud);
    log::info!("starting thread handling message from mud server");
    server::start_from_mud_handle(evttx, from_mud);

    // 7. run event loop on main thread
    let client_handler = Server::new(worldtx, pass, init_max_lines);
    let eventloop = EventLoop::new(rt, evtrx, client_handler, QuitServer);
    eventloop.run()?;

    Ok(())
}
