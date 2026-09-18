use crate::objects::ObjectHandle;
use crate::sync::SpinLock;

use super::message::IpcMessage;

pub const MAX_IPC_ENDPOINTS: usize = 16;
pub const IPC_QUEUE_CAPACITY: usize = 16;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct IpcHandle(u64);

impl IpcHandle {
    pub const INVALID: Self = Self(0);

    pub const fn new(index: usize, generation: u32) -> Self {
        Self(((generation as u64) << 32) | index as u64)
    }

    pub const fn index(self) -> usize {
        self.0 as u32 as usize
    }

    pub const fn generation(self) -> u32 {
        (self.0 >> 32) as u32
    }

    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy)]
struct IpcQueue {
    messages: [IpcMessage; IPC_QUEUE_CAPACITY],
    head: usize,
    tail: usize,
    count: usize,
}

impl IpcQueue {
    const fn new() -> Self {
        Self {
            messages: [IpcMessage::empty(); IPC_QUEUE_CAPACITY],
            head: 0,
            tail: 0,
            count: 0,
        }
    }

    fn is_empty(&self) -> bool {
        self.count == 0
    }

    fn is_full(&self) -> bool {
        self.count == IPC_QUEUE_CAPACITY
    }

    fn len(&self) -> usize {
        self.count
    }

    fn send(&mut self, message: IpcMessage) -> bool {
        if self.is_full() {
            return false;
        }

        self.messages[self.tail] = message;

        self.tail += 1;

        if self.tail == IPC_QUEUE_CAPACITY {
            self.tail = 0;
        }

        self.count += 1;

        true
    }

    fn receive(&mut self) -> Option<IpcMessage> {
        if self.is_empty() {
            return None;
        }

        let message = self.messages[self.head];

        self.messages[self.head] = IpcMessage::empty();

        self.head += 1;

        if self.head == IPC_QUEUE_CAPACITY {
            self.head = 0;
        }

        self.count -= 1;

        Some(message)
    }
}

struct IpcEndpoint {
    queue: SpinLock<IpcQueue>,
}

impl IpcEndpoint {
    const fn new() -> Self {
        Self {
            queue: SpinLock::new(IpcQueue::new()),
        }
    }
}

struct IpcEndpointSlot {
    occupied: bool,
    generation: u32,
    endpoint: IpcEndpoint,
}

impl IpcEndpointSlot {
    const fn empty() -> Self {
        Self {
            occupied: false,
            generation: 1,
            endpoint: IpcEndpoint::new(),
        }
    }

    fn clear(&mut self) {
        self.occupied = false;

        self.generation = self.generation.wrapping_add(1);

        if self.generation == 0 {
            self.generation = 1;
        }
    }
}

pub struct IpcManager {
    endpoints: SpinLock<[IpcEndpointSlot; MAX_IPC_ENDPOINTS]>,
}

impl IpcManager {
    pub const fn new() -> Self {
        Self {
            endpoints: SpinLock::new([const { IpcEndpointSlot::empty() }; MAX_IPC_ENDPOINTS]),
        }
    }

    pub fn create(&self) -> Option<IpcHandle> {
        let mut endpoints = self.endpoints.lock();

        for index in 0..MAX_IPC_ENDPOINTS {
            let slot = &mut endpoints[index];

            if !slot.occupied {
                slot.occupied = true;

                return Some(IpcHandle::new(index, slot.generation));
            }
        }

        None
    }

    pub fn is_valid(&self, handle: IpcHandle) -> bool {
        if !handle.is_valid() {
            return false;
        }

        let index = handle.index();

        if index >= MAX_IPC_ENDPOINTS {
            return false;
        }

        let endpoints = self.endpoints.lock();
        let slot = &endpoints[index];

        slot.occupied && slot.generation == handle.generation()
    }

    pub fn send(&self, handle: IpcHandle, sender: ObjectHandle, data: &[u8]) -> bool {
        let message = match IpcMessage::new(sender, data) {
            Some(message) => message,
            None => return false,
        };

        if !handle.is_valid() {
            return false;
        }

        let index = handle.index();

        if index >= MAX_IPC_ENDPOINTS {
            return false;
        }

        let endpoints = self.endpoints.lock();

        let slot = &endpoints[index];

        if !slot.occupied || slot.generation != handle.generation() {
            return false;
        }

        let mut queue = slot.endpoint.queue.lock();

        queue.send(message)
    }

    pub fn receive(&self, handle: IpcHandle) -> Option<IpcMessage> {
        if !handle.is_valid() {
            return None;
        }

        let index = handle.index();

        if index >= MAX_IPC_ENDPOINTS {
            return None;
        }

        let endpoints = self.endpoints.lock();

        let slot = &endpoints[index];

        if !slot.occupied || slot.generation != handle.generation() {
            return None;
        }

        let mut queue = slot.endpoint.queue.lock();

        queue.receive()
    }

    pub fn queue_len(&self, handle: IpcHandle) -> Option<usize> {
        if !handle.is_valid() {
            return None;
        }

        let index = handle.index();

        if index >= MAX_IPC_ENDPOINTS {
            return None;
        }

        let endpoints = self.endpoints.lock();

        let slot = &endpoints[index];

        if !slot.occupied || slot.generation != handle.generation() {
            return None;
        }

        let queue = slot.endpoint.queue.lock();

        Some(queue.len())
    }

    pub fn destroy(&self, handle: IpcHandle) -> bool {
        if !handle.is_valid() {
            return false;
        }

        let index = handle.index();

        if index >= MAX_IPC_ENDPOINTS {
            return false;
        }

        let mut endpoints = self.endpoints.lock();

        let slot = &mut endpoints[index];

        if !slot.occupied || slot.generation != handle.generation() {
            return false;
        }

        {
            let queue = slot.endpoint.queue.lock();

            if !queue.is_empty() {
                return false;
            }
        }

        slot.clear();

        true
    }

    pub fn count(&self) -> usize {
        let endpoints = self.endpoints.lock();

        endpoints.iter().filter(|slot| slot.occupied).count()
    }
}
