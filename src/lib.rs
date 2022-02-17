#![deny(clippy::all)]
#![forbid(unsafe_code)]

pub mod prelude;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Command {
    Set { pos: [usize; 2], color: [u8; 3] },
    Redraw,
    Nop,
}

impl Default for Command {
    fn default() -> Self {
        Self::Nop
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RendererInitArgs {
    pub size: [u32; 2],
    pub ipc_name: String,
}

impl RendererInitArgs {
    pub fn command_sender(&self) -> CommandSender {
        let sender =
            CommandSender::connect(self.ipc_name.clone()).expect("Connecting to ipc server");
        sender
            .send(Command::Nop)
            .expect("Sending acknowledgment nop");
        sender
    }

    pub fn serialize(&self) -> String {
        base64::encode(bincode::serialize(self).expect("Serializing renderer's init args"))
    }

    pub fn deserialize(v: &str) -> Self {
        let bytes = base64::decode(v).expect("Decoding renderer's init args from base64");
        bincode::deserialize(&bytes).expect("Deserializing renderer's init args")
    }
}

pub type CommandSender = ipc_channel::ipc::IpcSender<Command>;
