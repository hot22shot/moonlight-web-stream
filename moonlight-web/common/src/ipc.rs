use std::{marker::PhantomData, pin::Pin};

use log::warn;
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader, Stdin, Stdout},
    process::{ChildStderr, ChildStdin, ChildStdout},
    spawn,
    sync::mpsc::{Receiver, Sender, channel},
};

use bincode::{Decode, Encode, config::Configuration as BincodeConfig, decode_from_slice};

use crate::{
    StreamSettings,
    api_bindings::{StreamClientMessage, StreamServerMessage},
    config::Config,
};

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Encode, Decode)]
pub enum ServerIpcMessage {
    Init {
        server_config: Config,
        stream_settings: StreamSettings,
        host_address: String,
        host_http_port: u16,
        host_unique_id: Option<String>,
        client_private_key_pem: String,
        client_certificate_pem: String,
        server_certificate_pem: String,
        app_id: u32,
    },
    WebSocket(StreamClientMessage),
    Stop,
}

#[derive(Debug, Encode, Decode)]
pub enum StreamerIpcMessage {
    WebSocket(StreamServerMessage),
}

// We're using the:
// Stdin: message passing
// Stdout: message passing
// Stderr: logging

const BINCODE_CONFIG: BincodeConfig = bincode::config::standard();

pub async fn create_child_ipc<Message, ChildMessage>(
    log_prefix: String,
    stdin: ChildStdin,
    stdout: ChildStdout,
    stderr: Option<ChildStderr>,
) -> (IpcSender<Message>, IpcReceiver<ChildMessage>)
where
    Message: Send + Encode + 'static,
    ChildMessage: Decode<()>,
{
    if let Some(stderr) = stderr {
        spawn(async move {
            let buf_reader = BufReader::new(stderr);
            let mut lines = buf_reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                println!("[{log_prefix}]: {line}");
            }
        });
    }

    let (sender, receiver) = channel::<Message>(10);

    spawn(async move {
        ipc_sender(stdin, receiver).await;
    });

    (
        IpcSender { sender },
        IpcReceiver {
            errored: false,
            vec: Vec::new(),
            read: Box::pin(stdout),
            phantom: Default::default(),
        },
    )
}

pub async fn create_process_ipc<ParentMessage, Message>(
    stdin: Stdin,
    stdout: Stdout,
) -> (IpcSender<Message>, IpcReceiver<ParentMessage>)
where
    ParentMessage: Decode<()>,
    Message: Send + Encode + 'static,
{
    let (sender, receiver) = channel::<Message>(10);

    spawn(async move {
        ipc_sender(stdout, receiver).await;
    });

    (
        IpcSender { sender },
        IpcReceiver {
            errored: false,
            vec: Vec::new(),
            read: Box::pin(stdin),
            phantom: Default::default(),
        },
    )
}

async fn ipc_sender<Message>(mut write: impl AsyncWriteExt + Unpin, mut receiver: Receiver<Message>)
where
    Message: Encode,
{
    while let Some(value) = receiver.recv().await {
        let vec = match bincode::encode_to_vec(&value, BINCODE_CONFIG) {
            Ok(value) => value,
            Err(err) => {
                warn!("[Ipc]: failed to encode message: {err:?}");
                continue;
            }
        };

        if let Err(err) = write.write_u32(vec.len() as u32).await {
            warn!("[Ipc]: failed to write message length: {err:?}");
            return;
        };
        if let Err(err) = write.write(&vec).await {
            warn!("[Ipc]: failed to write message length: {err:?}");
            return;
        };
    }
}

#[derive(Debug)]
pub struct IpcSender<Message> {
    sender: Sender<Message>,
}

impl<Message> Clone for IpcSender<Message> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

impl<Message> IpcSender<Message>
where
    Message: Encode + Send + 'static,
{
    pub async fn send(&mut self, message: Message) {
        let _ = self.sender.send(message).await;
    }
}

pub struct IpcReceiver<Message> {
    errored: bool,
    vec: Vec<u8>,
    read: Pin<Box<dyn AsyncRead + Send + 'static>>,
    phantom: PhantomData<Message>,
}

impl<Message> IpcReceiver<Message>
where
    Message: Decode<()>,
{
    pub async fn recv(&mut self) -> Option<Message> {
        if self.errored {
            return None;
        }

        let len = match self.read.read_u32().await {
            Ok(value) => value,
            Err(err) => {
                self.errored = true;
                warn!("[Ipc]: failed to read u32: {err:?}");

                return None;
            }
        };

        self.vec.resize(len as usize, 0);
        if let Err(err) = self.read.read_exact(&mut self.vec).await {
            self.errored = true;
            warn!("[Ipc]: failed to read message: {err:?}");

            return None;
        }

        match decode_from_slice(&self.vec, BINCODE_CONFIG) {
            Ok((value, _)) => Some(value),
            Err(err) => {
                warn!("[Ipc]: failed to decode message: {err:?}");

                None
            }
        }
    }
}
