use std::{num::NonZeroUsize, panic::resume_unwind};

use compio::{
    buf::{arrayvec::ArrayVec, IntoInner},
    dispatcher::Dispatcher,
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    runtime::{spawn, Unattached},
};
use futures_util::{stream::FuturesUnordered, StreamExt};

#[compio_macros::test]
async fn listener_dispatch() {
    const THREAD_NUM: usize = 5;
    const CLIENT_NUM: usize = 10;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let dispatcher = Dispatcher::builder()
        .worker_threads(NonZeroUsize::new(THREAD_NUM).unwrap())
        .build()
        .unwrap();
    let task = spawn(async move {
        let mut futures = FuturesUnordered::from_iter((0..CLIENT_NUM).map(|_| async {
            let mut cli = TcpStream::connect(&addr).await.unwrap();
            cli.write_all("Hello world!").await.unwrap();
        }));
        while let Some(()) = futures.next().await {}
    });
    let mut handles = FuturesUnordered::new();
    for _i in 0..CLIENT_NUM {
        let (srv, _) = listener.accept().await.unwrap();
        let srv = Unattached::new(srv).unwrap();
        let handle = dispatcher
            .dispatch(move || {
                let mut srv = srv.into_inner();
                async move {
                    let (_, buf) = srv.read_exact(ArrayVec::<u8, 12>::new()).await.unwrap();
                    assert_eq!(buf.as_slice(), b"Hello world!");
                }
            })
            .unwrap();
        handles.push(handle.join());
    }
    while let Some(res) = handles.next().await {
        res.unwrap().unwrap_or_else(|e| resume_unwind(e));
    }
    let (_, results) = futures_util::join!(task, dispatcher.join());
    results.unwrap();
}
