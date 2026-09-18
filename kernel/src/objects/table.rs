use crate::sync::SpinLock;

use super::object::{ObjectHandle, ObjectRecord, ObjectSlot, ObjectType};

pub const MAX_KERNEL_OBJECTS: usize = 64;

/// Global kernel object table.
///
/// This intentionally uses a fixed array for now.
/// Later this can become a dynamically growing object manager
/// once the kernel allocator is fully operational.
pub struct ObjectTable {
    slots: SpinLock<[ObjectSlot; MAX_KERNEL_OBJECTS]>,
}

impl ObjectTable {
    pub const fn new() -> Self {
        Self {
            slots: SpinLock::new([const { ObjectSlot::empty() }; MAX_KERNEL_OBJECTS]),
        }
    }

    /// Create a new kernel object and return its handle.
    pub fn create(&self, object_type: ObjectType) -> Option<ObjectHandle> {
        let mut slots = self.slots.lock();

        for index in 0..MAX_KERNEL_OBJECTS {
            let slot = &mut slots[index];

            if slot.is_free() {
                slot.occupied = true;
                slot.record = Some(ObjectRecord::new(object_type));

                return Some(ObjectHandle::new(index, slot.generation));
            }
        }

        None
    }

    /// Check whether a handle refers to a currently valid object.
    pub fn is_valid(&self, handle: ObjectHandle) -> bool {
        if !handle.is_valid() {
            return false;
        }

        let index = handle.index();

        if index >= MAX_KERNEL_OBJECTS {
            return false;
        }

        let slots = self.slots.lock();
        let slot = &slots[index];

        slot.occupied && slot.generation == handle.generation()
    }

    /// Get the object type associated with a handle.
    pub fn get_type(&self, handle: ObjectHandle) -> Option<ObjectType> {
        if !handle.is_valid() {
            return None;
        }

        let index = handle.index();

        if index >= MAX_KERNEL_OBJECTS {
            return None;
        }

        let slots = self.slots.lock();
        let slot = &slots[index];

        if !slot.occupied || slot.generation != handle.generation() {
            return None;
        }

        slot.record.map(|record| record.object_type)
    }

    /// Increase the reference count.
    pub fn retain(&self, handle: ObjectHandle) -> bool {
        if !handle.is_valid() {
            return false;
        }

        let index = handle.index();

        if index >= MAX_KERNEL_OBJECTS {
            return false;
        }

        let mut slots = self.slots.lock();
        let slot = &mut slots[index];

        if !slot.occupied || slot.generation != handle.generation() {
            return false;
        }

        if let Some(record) = slot.record.as_mut() {
            if record.references == u32::MAX {
                return false;
            }

            record.references += 1;
            return true;
        }

        false
    }

    /// Decrease the reference count.
    ///
    /// When the reference count reaches zero,
    /// the object slot is released.
    pub fn release(&self, handle: ObjectHandle) -> bool {
        if !handle.is_valid() {
            return false;
        }

        let index = handle.index();

        if index >= MAX_KERNEL_OBJECTS {
            return false;
        }

        let mut slots = self.slots.lock();
        let slot = &mut slots[index];

        if !slot.occupied || slot.generation != handle.generation() {
            return false;
        }

        let should_destroy = match slot.record.as_mut() {
            Some(record) => {
                if record.references == 0 {
                    return false;
                }

                record.references -= 1;
                record.references == 0
            }

            None => false,
        };

        if should_destroy {
            slot.clear();
        }

        true
    }

    /// Explicitly destroy an object.
    ///
    /// This is primarily useful for kernel-owned objects where
    /// the caller knows the object should no longer exist.
    pub fn destroy(&self, handle: ObjectHandle) -> bool {
        if !handle.is_valid() {
            return false;
        }

        let index = handle.index();

        if index >= MAX_KERNEL_OBJECTS {
            return false;
        }

        let mut slots = self.slots.lock();
        let slot = &mut slots[index];

        if !slot.occupied || slot.generation != handle.generation() {
            return false;
        }

        slot.clear();
        true
    }

    /// Number of currently occupied object slots.
    pub fn count(&self) -> usize {
        let slots = self.slots.lock();

        slots.iter().filter(|slot| slot.occupied).count()
    }
}
