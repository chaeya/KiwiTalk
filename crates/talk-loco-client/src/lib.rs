pub mod client;

use std::{
    collections::HashMap,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use bson::Document;
use futures::{ready, AsyncRead, AsyncReadExt, AsyncWrite, Future, FutureExt};
use loco_protocol::command::codec::CommandCodec;
use nohash_hasher::BuildNoHashHasher;
use parking_lot::Mutex;
use talk_loco_command::{
    command::{
        codec::{BsonCommandCodec, ReadError},
        BsonCommand, ReadBsonCommand,
    },
    response::ResponseData,
};
use tokio::sync::{mpsc, oneshot};

#[derive(Debug, Clone)]
pub struct LocoCommandSession {
    sender: mpsc::Sender<RequestCommand>,
}

impl LocoCommandSession {
    pub fn new<S: AsyncRead + AsyncWrite + Send + 'static>(stream: S) -> Self {
        Self::new_with_handler(stream, |_| {})
    }

    pub fn new_with_handler<
        S: AsyncRead + AsyncWrite + Send + 'static,
        Handler: Send + 'static + FnMut(ReadResult),
    >(
        stream: S,
        handler: Handler,
    ) -> Self {
        let (sender, receiver) = mpsc::channel(128);
        let (read_stream, write_stream) = stream.split();
        let (read_task, write_task) = session_task(handler);

        tokio::spawn(read_task.run(read_stream));
        tokio::spawn(write_task.run(write_stream, receiver));

        Self { sender }
    }

    pub async fn send(&self, command: BsonCommand<Document>) -> CommandRequest {
        let (sender, receiver) = oneshot::channel();

        self.sender
            .send(RequestCommand::new(command, sender))
            .await
            .ok();

        CommandRequest(receiver)
    }
}

#[derive(Debug)]
pub struct CommandRequest(oneshot::Receiver<BsonCommand<ResponseData>>);

impl Future for CommandRequest {
    type Output = Option<BsonCommand<ResponseData>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(ready!(self.0.poll_unpin(cx)).ok())
    }
}

pub type ReadResult = Result<ReadBsonCommand<ResponseData>, ReadError>;

type ResponseMap = HashMap<i32, oneshot::Sender<BsonCommand<ResponseData>>, BuildNoHashHasher<i32>>;

#[derive(Debug)]
struct ReadTask<Handler> {
    response_map: Arc<Mutex<ResponseMap>>,
    handler: Handler,
}

impl<Handler: FnMut(ReadResult)> ReadTask<Handler> {
    #[inline(always)]
    const fn new(response_map: Arc<Mutex<ResponseMap>>, handler: Handler) -> Self {
        ReadTask {
            response_map,
            handler,
        }
    }

    pub async fn run(mut self, read_stream: impl Send + AsyncRead + Unpin + 'static) {
        let mut read_codec = BsonCommandCodec(CommandCodec::new(read_stream));

        loop {
            let read = read_codec.read_async().await;

            match read {
                Ok(read) => {
                    if let Some(sender) = self.response_map.lock().remove(&read.id) {
                        sender.send(read.command).ok();
                    } else {
                        (self.handler)(Ok(read));
                    }
                }

                Err(_) => {
                    (self.handler)(read);
                }
            }
        }
    }
}

#[derive(Debug)]
struct WriteTask {
    response_map: Arc<Mutex<ResponseMap>>,
    next_request_id: i32,
}

impl WriteTask {
    #[inline(always)]
    const fn new(response_map: Arc<Mutex<ResponseMap>>) -> Self {
        WriteTask {
            response_map,
            next_request_id: 0,
        }
    }

    pub async fn run(
        mut self,
        write_stream: impl Send + AsyncWrite + Unpin + 'static,
        mut request_recv: mpsc::Receiver<RequestCommand>,
    ) {
        let mut write_codec = BsonCommandCodec(CommandCodec::new(write_stream));
        while let Some(request) = request_recv.recv().await {
            let request_id = self.next_request_id;

            self.response_map
                .lock()
                .insert(request_id, request.response_sender);

            if write_codec
                .write_async(request_id, &request.command)
                .await
                .is_err()
                || write_codec.flush_async().await.is_err()
            {
                self.response_map.lock().remove(&request_id);
                continue;
            }

            self.next_request_id += 1;
        }
    }
}

fn session_task<Handler: FnMut(ReadResult)>(handler: Handler) -> (ReadTask<Handler>, WriteTask) {
    let map = Arc::new(Mutex::new(HashMap::default()));

    (ReadTask::new(map.clone(), handler), WriteTask::new(map))
}

#[derive(Debug)]
struct RequestCommand {
    pub command: BsonCommand<Document>,
    pub response_sender: oneshot::Sender<BsonCommand<ResponseData>>,
}

impl RequestCommand {
    pub const fn new(
        command: BsonCommand<Document>,
        response_sender: oneshot::Sender<BsonCommand<ResponseData>>,
    ) -> Self {
        Self {
            command,
            response_sender,
        }
    }
}
