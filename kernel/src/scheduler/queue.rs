// ================================================================
// UNBOUND RUN QUEUE
// ================================================================
//
// Phase 3.2:
//
// A simple fixed-size scheduler queue.
//
// We deliberately avoid dynamic allocation at this stage.
// Heap allocation will be introduced later.
//
// ================================================================

use super::task::{TaskId, TaskState};

// ================================================================
// QUEUE CAPACITY
// ================================================================

pub const MAX_TASKS: usize = 64;

// ================================================================
// RUN QUEUE
// ================================================================

pub struct RunQueue {
    tasks: [Option<TaskId>; MAX_TASKS],
    count: usize,
}

impl RunQueue {
    pub const fn new() -> Self {
        Self {
            tasks: [None; MAX_TASKS],
            count: 0,
        }
    }

    // ------------------------------------------------------------
    // ADD
    // ------------------------------------------------------------

    pub fn push(&mut self, task_id: TaskId) -> bool {
        if self.count >= MAX_TASKS {
            return false;
        }

        for slot in self.tasks.iter() {
            if *slot == Some(task_id) {
                return false;
            }
        }

        for slot in self.tasks.iter_mut() {
            if slot.is_none() {
                *slot = Some(task_id);
                self.count += 1;
                return true;
            }
        }

        false
    }

    // ------------------------------------------------------------
    // REMOVE
    // ------------------------------------------------------------

    pub fn remove(&mut self, task_id: TaskId) -> bool {
        for slot in self.tasks.iter_mut() {
            if *slot == Some(task_id) {
                *slot = None;
                self.count -= 1;
                return true;
            }
        }

        false
    }

    // ------------------------------------------------------------
    // CONTAINS
    // ------------------------------------------------------------

    pub fn contains(&self, task_id: TaskId) -> bool {
        for slot in self.tasks.iter() {
            if *slot == Some(task_id) {
                return true;
            }
        }

        false
    }

    // ------------------------------------------------------------
    // COUNT
    // ------------------------------------------------------------

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    // ------------------------------------------------------------
    // FIRST RUNNABLE TASK
    // ------------------------------------------------------------

    pub fn first(&self) -> Option<TaskId> {
        for slot in self.tasks.iter() {
            if let Some(task_id) = *slot {
                return Some(task_id);
            }
        }

        None
    }

    // ------------------------------------------------------------
    // ROTATE
    // ------------------------------------------------------------
    //
    // Simple round-robin rotation.
    //
    // Actual scheduling policy will be expanded later.
    //

    pub fn rotate(&mut self) {
        if self.count <= 1 {
            return;
        }

        let first = self.tasks[0];

        for index in 0..(MAX_TASKS - 1) {
            self.tasks[index] = self.tasks[index + 1];
        }

        self.tasks[MAX_TASKS - 1] = first;
    }

    // ------------------------------------------------------------
    // DEBUG STATE
    // ------------------------------------------------------------

    pub fn state_for(&self, task_id: TaskId) -> Option<TaskState> {
        if self.contains(task_id) {
            Some(TaskState::Ready)
        } else {
            None
        }
    }
}
