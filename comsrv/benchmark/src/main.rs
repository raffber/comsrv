use comsrv::app::{Request, Response};
use wsrpc::client::Client as WsrpcClient;
use tokio::time::Duration;
use std::time::Instant;
use tokio::task;
use futures::future::join_all;
use tokio::sync::mpsc::UnboundedReceiver;
use uuid::Uuid;
use url::Url;

type Client = WsrpcClient<Request, Response>;

async fn bench() {
    let url: Url = "ws://127.0.0.1:5902".parse().unwrap();

    let mut tasks = Vec::new();
    let now = Instant::now();
    for _ in 0 .. 10 {
        tasks.push(task::spawn(bench_task(url.clone())));
    }
    let _ = join_all(tasks).await;
    let delta = now.elapsed();
    println!("Finished in: {} ms", delta.as_millis());
}

async fn bench_task(url: Url) {
    let client = Client::connect(url, Duration::from_millis(100)).await.unwrap();
    let mut replies = client.replies();
    for _ in 0..10000_u32 {
        query_one(&client, &mut replies).await;
    }
}

async fn query_one(client: &Client, replies: &mut UnboundedReceiver<(Response, Uuid)>) {
    let req = Request::ListInstruments;
    if let Some(id) = client.send(req) {
        while let Some((_, rx_id)) = replies.recv().await {
            if id == rx_id {
                return;
            }
        }
    }
}

fn main() {
    let mut rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(bench());
}
