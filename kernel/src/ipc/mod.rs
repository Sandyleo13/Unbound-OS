pub mod endpoint;
pub mod message;

pub use message::{IPC_MESSAGE_SIZE, IpcMessage};

pub use endpoint::{IPC_QUEUE_CAPACITY, IpcHandle, IpcManager, MAX_IPC_ENDPOINTS};
