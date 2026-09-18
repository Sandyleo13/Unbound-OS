use crate::objects::ObjectHandle;

pub const IPC_MESSAGE_SIZE: usize = 256;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct IpcMessage {
    pub sender: ObjectHandle,
    pub len: u16,
    pub _reserved: u16,
    pub data: [u8; IPC_MESSAGE_SIZE],
}

impl IpcMessage {
    pub const fn empty() -> Self {
        Self {
            sender: ObjectHandle::INVALID,
            len: 0,
            _reserved: 0,
            data: [0; IPC_MESSAGE_SIZE],
        }
    }

    pub fn new(sender: ObjectHandle, data: &[u8]) -> Option<Self> {
        if data.len() > IPC_MESSAGE_SIZE {
            return None;
        }

        let mut message = Self::empty();

        message.sender = sender;
        message.len = data.len() as u16;

        let mut index = 0;

        while index < data.len() {
            message.data[index] = data[index];
            index += 1;
        }

        Some(message)
    }

    pub fn payload(&self) -> &[u8] {
        &self.data[..self.len as usize]
    }

    pub fn payload_mut(&mut self) -> &mut [u8] {
        &mut self.data[..self.len as usize]
    }
}
