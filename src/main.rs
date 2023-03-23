mod spawns;

use std::sync::Arc;
use tokio::runtime;
use fast_log::{
    consts::LogSize,
    plugin::{file_split::RollingType, packer::LogPacker},
    Config, Logger,
};
use log::{info, Log};
use prometheus_client::{metrics::counter::Counter};
use std::sync::atomic::AtomicU64;
use spawns::{start_prometheus, infinity_loop, file_watch};
use std::path::Path;
use notify::*;


fn main() {

    let runtime = Arc::new(runtime::Builder::new_multi_thread().enable_all().build().expect("runtime error"));
    let logger: &'static Logger = fast_log::init(Config::new().console().file_split(
       "/var/log/neon/geyser.log",
            LogSize::KB(10),
        RollingType::All,
        LogPacker{}
    )).expect("log init error");

    logger.set_level(log::LevelFilter::Trace);
    info!("app start");

    let (prometheus_term_tx, prometheus_term_rx) = tokio::sync::oneshot::channel::<()>();

    let cnt : Counter<u64, AtomicU64>  = Default::default();
    let cnt  = Arc::new(cnt);
    let prometheus_handle = runtime.spawn(start_prometheus(
        cnt.clone(),
        prometheus_term_rx,
    ));


    let (infinity_loop_tx, infinity_loop_rx) = flume::bounded(100);
    let infinity_loop_handle = runtime.spawn(infinity_loop(infinity_loop_rx));


    let (file_watcher_tx, file_watcher_rx) = std::sync::mpsc::channel();
    let mut file_watcher: Box<dyn Watcher> =
        Box::new(RecommendedWatcher::new(file_watcher_tx.clone(), notify::Config::default()).unwrap());
    file_watcher
        .watch(Path::new("/var/log/neon"), RecursiveMode::Recursive)
        .unwrap();
    let file_watcher_handle = runtime.spawn(file_watch(file_watcher_rx));



    runtime.block_on(async {
        tokio::signal::ctrl_c().await.expect("failed to listen for event");

        prometheus_term_tx.send(()).unwrap();
        prometheus_handle.await;

        infinity_loop_tx.send("shutdown".to_string()).unwrap();
        infinity_loop_handle.await;

        file_watcher_tx.send(Ok(Event::new(EventKind::Other)));
        file_watcher_handle.await;



        // let send_message_timeout = std::time::Duration::from_secs(1);
        // let mut send_message = tokio::time::interval(send_message_timeout);
        // send_message.tick().await;
        // loop {
        //     tokio::select! {
        //     _ = send_message.tick() => { infinity_loop_tx.send("message".into()); }
        //     }
        // }

    });

    logger.flush();
    info!("app stop");
}
