pub mod object;
pub mod table;

pub use object::{ObjectHandle, ObjectRecord, ObjectSlot, ObjectType};

pub use table::{MAX_KERNEL_OBJECTS, ObjectTable};
