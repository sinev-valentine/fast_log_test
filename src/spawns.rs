use prometheus_client::{registry::Registry, encoding::text::encode, metrics::counter::Counter};
use tokio::sync::oneshot;
use std::{sync::Arc, io, pin::Pin, future::Future};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use log::*;
use hyper::{
    service::{make_service_fn, service_fn},
    Body, Request, Response, Server,
};
use std::sync::atomic::AtomicU64;
use std::ops::Deref;
use notify::{EventKind, event::{ModifyKind, CreateKind, DataChange} };

pub async fn start_prometheus (cnt: Arc<Counter<u64, AtomicU64>>, rx_term: oneshot::Receiver<()>) {
    let mut registry = <Registry>::default();
    registry.register("event", "desc event", cnt.deref().clone()   );


    let metrics_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9090);
    start_metrics_server(metrics_addr, registry, rx_term).await
}


async fn start_metrics_server(metrics_addr: SocketAddr, registry: Registry, rx_term: oneshot::Receiver<()>) {

    println!("Starting metrics server on {metrics_addr}");

    let registry = Arc::new(registry);
    Server::bind(&metrics_addr)
        .serve(make_service_fn(move |_conn| {
            let registry = registry.clone();
            async move {
                let handler = make_handler(registry);
                Ok::<_, io::Error>(service_fn(handler))
            }
        }))
        .with_graceful_shutdown(async move {
            rx_term.await.ok();
            info!("Shutdown prometheus task");
        })
        .await
        .expect("Failed to bind hyper server with graceful_shutdown");
}

fn make_handler(
    registry: Arc<Registry>,
) -> impl Fn(Request<Body>) -> Pin<Box<dyn Future<Output = io::Result<Response<Body>>> + Send>> {
    // This closure accepts a request and responds with the OpenMetrics encoding of our metrics.
    move |_req: Request<Body>| {
        let reg = registry.clone();
        Box::pin(async move {
            let mut buf = String::new();
            encode(&mut buf, &reg.clone())
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
                .map(|_| {
                    let body = Body::from(buf);
                    Response::builder()
                        .header(
                            hyper::header::CONTENT_TYPE,
                            "application/openmetrics-text; version=1.0.0; charset=utf-8",
                        )
                        .body(body)
                        .unwrap()
                })
        })
    }
}


pub async fn infinity_loop ( rx: flume::Receiver<String>) {
    info!("spawn infinity_loop");
    loop {
        match rx.recv_async().await {
            Ok(mes) => {
                info!("get message: {mes}");
                if mes == "shutdown".to_string() {
                    break
                }
            },
            _ => info!("get message error")
        }
    }
}

pub async fn file_watch(rx: std::sync::mpsc::Receiver<notify::Result<notify::Event>>) {
    for e in rx {
        match e {
            Ok(event) =>  {
                match event.kind {
                    EventKind::Create(CreateKind::File) => info! ("file_watcher: create file"),
                    EventKind::Other => {
                        info!("file_watcher shutdown");
                        break;
                    },
                    _ => {} // doesn't matter
                }
            },
            _ => info!("file_watcher error")
        };
    }
}