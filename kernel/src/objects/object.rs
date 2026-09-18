use core::fmt;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ObjectType {
    Process = 1,
    Thread = 2,
    AddressSpace = 3,
    IpcEndpoint = 4,
    File = 5,
    Device = 6,
    Event = 7,
}

impl ObjectType {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Process => "PROCESS",
            Self::Thread => "THREAD",
            Self::AddressSpace => "ADDRESS SPACE",
            Self::IpcEndpoint => "IPC ENDPOINT",
            Self::File => "FILE",
            Self::Device => "DEVICE",
            Self::Event => "EVENT",
        }
    }
}

impl fmt::Debug for ObjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A kernel object handle.
///
/// Lower 32 bits  = table index
/// Upper 32 bits  = generation
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ObjectHandle(u64);

impl ObjectHandle {
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

impl fmt::Debug for ObjectHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ObjectHandle({:#018x}, index={}, generation={})",
            self.0,
            self.index(),
            self.generation()
        )
    }
}

/// Metadata stored for every kernel object.
///
/// Actual Process/Thread/File/etc. data will be added later.
/// For now this gives us the kernel-wide object lifetime system.
#[derive(Clone, Copy)]
pub struct ObjectRecord {
    pub object_type: ObjectType,
    pub references: u32,
}

impl ObjectRecord {
    pub const fn new(object_type: ObjectType) -> Self {
        Self {
            object_type,
            references: 1,
        }
    }
}

/// One slot in the static object table.
#[derive(Clone, Copy)]
pub struct ObjectSlot {
    pub occupied: bool,
    pub generation: u32,
    pub record: Option<ObjectRecord>,
}

impl ObjectSlot {
    pub const fn empty() -> Self {
        Self {
            occupied: false,
            generation: 1,
            record: None,
        }
    }

    pub const fn is_free(&self) -> bool {
        !self.occupied
    }

    pub fn clear(&mut self) {
        self.occupied = false;
        self.record = None;

        // Never allow generation zero.
        self.generation = self.generation.wrapping_add(1);

        if self.generation == 0 {
            self.generation = 1;
        }
    }
}
