// ================================================================
// UNBOUND SCHEDULER
// ================================================================
//
// Phase 3.4:
//   Kernel task contexts + timer-driven preemptive scheduling.
//
// Phase 3.3 cooperative context switching remains intact.
//
// ================================================================

pub mod context;
pub mod queue;
pub mod task;

use queue::RunQueue;

use task::{Task, TaskContext, TaskId, TaskState, allocate_task_id};

use core::sync::atomic::{AtomicU64, Ordering};

// ================================================================
// LIMITS
// ================================================================

pub const MAX_SCHEDULER_TASKS: usize = 8;

// ================================================================
// TIME SLICE
// ================================================================

pub const DEFAULT_TIME_SLICE_TICKS: u64 = 5;

// ================================================================
// TEST STATE
// ================================================================

static mut TEST_KERNEL_CONTEXT: TaskContext = TaskContext::empty();

static mut TEST_TASK_A_ID: TaskId = 0;

static mut TEST_TASK_B_ID: TaskId = 0;

// ================================================================
// SCHEDULER
// ================================================================

pub struct Scheduler {
    run_queue: RunQueue,

    current_task: Option<TaskId>,

    task_count: usize,

    scheduler_ticks: u64,

    tasks: [Option<Task>; MAX_SCHEDULER_TASKS],
}

impl Scheduler {
    // ------------------------------------------------------------
    // CREATE EMPTY SCHEDULER
    // ------------------------------------------------------------

    pub const fn new() -> Self {
        Self {
            run_queue: RunQueue::new(),

            current_task: None,

            task_count: 0,

            scheduler_ticks: 0,

            tasks: [const { None }; MAX_SCHEDULER_TASKS],
        }
    }

    // ------------------------------------------------------------
    // CREATE TASK
    // ------------------------------------------------------------

    pub fn create_task(&mut self, task: Task) -> Option<TaskId> {
        if self.task_count >= MAX_SCHEDULER_TASKS {
            return None;
        }

        let task_id = task.id;

        if !self.run_queue.push(task_id) {
            return None;
        }

        for slot in self.tasks.iter_mut() {
            if slot.is_none() {
                *slot = Some(task);

                self.task_count += 1;

                return Some(task_id);
            }
        }

        self.run_queue.remove(task_id);

        None
    }

    // ------------------------------------------------------------
    // FIND TASK
    // ------------------------------------------------------------

    pub fn task(&self, task_id: TaskId) -> Option<&Task> {
        for slot in self.tasks.iter() {
            if let Some(task) = slot {
                if task.id == task_id {
                    return Some(task);
                }
            }
        }

        None
    }

    // ------------------------------------------------------------
    // FIND TASK MUTABLY
    // ------------------------------------------------------------

    pub fn task_mut(&mut self, task_id: TaskId) -> Option<&mut Task> {
        for slot in self.tasks.iter_mut() {
            if let Some(task) = slot {
                if task.id == task_id {
                    return Some(task);
                }
            }
        }

        None
    }

    // ------------------------------------------------------------
    // SET CURRENT TASK
    // ------------------------------------------------------------

    pub fn set_current(&mut self, task_id: TaskId) -> bool {
        if !self.run_queue.contains(task_id) {
            return false;
        }

        self.current_task = Some(task_id);

        true
    }

    // ------------------------------------------------------------
    // CURRENT TASK
    // ------------------------------------------------------------

    pub fn current(&self) -> Option<TaskId> {
        self.current_task
    }

    // ------------------------------------------------------------
    // TIMER TICK
    // ------------------------------------------------------------

    pub fn tick(&mut self) {
        self.scheduler_ticks = self.scheduler_ticks.wrapping_add(1);

        if let Some(task_id) = self.current_task {
            if let Some(task) = self.task_mut(task_id) {
                task.total_ticks = task.total_ticks.wrapping_add(1);

                task.time_slice_ticks = task.time_slice_ticks.wrapping_add(1);
            }
        }
    }

    // ------------------------------------------------------------
    // TICK COUNT
    // ------------------------------------------------------------

    pub fn ticks(&self) -> u64 {
        self.scheduler_ticks
    }

    // ------------------------------------------------------------
    // TASK COUNT
    // ------------------------------------------------------------

    pub fn task_count(&self) -> usize {
        self.task_count
    }

    // ------------------------------------------------------------
    // SCHEDULE
    // ------------------------------------------------------------

    pub fn schedule(&mut self) {
        if self.run_queue.is_empty() {
            self.current_task = None;

            return;
        }

        self.run_queue.rotate();

        self.current_task = self.run_queue.first();
    }

    // ------------------------------------------------------------
    // READY CHECK
    // ------------------------------------------------------------

    pub fn has_runnable_task(&self) -> bool {
        !self.run_queue.is_empty()
    }

    // ------------------------------------------------------------
    // REMOVE TASK
    // ------------------------------------------------------------

    pub fn remove_task(&mut self, task_id: TaskId) -> bool {
        if !self.run_queue.remove(task_id) {
            return false;
        }

        for slot in self.tasks.iter_mut() {
            if let Some(task) = slot {
                if task.id == task_id {
                    *slot = None;

                    break;
                }
            }
        }

        if self.task_count > 0 {
            self.task_count -= 1;
        }

        if self.current_task == Some(task_id) {
            self.current_task = self.run_queue.first();
        }

        true
    }

    // ------------------------------------------------------------
    // TASK STATE
    // ------------------------------------------------------------

    pub fn task_state(&self, task_id: TaskId) -> Option<TaskState> {
        self.task(task_id).map(|task| task.state)
    }
}

// ================================================================
// GLOBAL SCHEDULER
// ================================================================

static mut SCHEDULER: Scheduler = Scheduler::new();

// ================================================================
// INITIALIZATION
// ================================================================

pub fn init() {
    unsafe {
        let scheduler = core::ptr::addr_of_mut!(SCHEDULER);

        (*scheduler) = Scheduler::new();

        TEST_KERNEL_CONTEXT = TaskContext::empty();

        TEST_TASK_A_ID = 0;

        TEST_TASK_B_ID = 0;
    }
}

// ================================================================
// TIMER TICK
// ================================================================

pub fn timer_tick() {
    unsafe {
        let scheduler = core::ptr::addr_of_mut!(SCHEDULER);

        (*scheduler).tick();
    }
}

// ================================================================
// PREEMPTIVE TIMER INTERRUPT
// ================================================================

static PREEMPT_SWITCH_DEBUG_COUNT: AtomicU64 = AtomicU64::new(0);

#[inline(always)]
fn read_interrupt_frame_word(rsp: u64, offset: usize) -> u64 {
    unsafe { core::ptr::read((rsp as *const u8).add(offset) as *const u64) }
}

#[inline(always)]
fn is_canonical_address(addr: u64) -> bool {
    let upper = addr >> 48;

    upper == 0x0000 || upper == 0xFFFF
}

fn validate_interrupt_frame(rsp: u64) -> bool {
    if rsp == 0 {
        return false;
    }

    let vector = read_interrupt_frame_word(rsp, 15 * 8);

    let rip = read_interrupt_frame_word(rsp, 16 * 8);

    let cs = read_interrupt_frame_word(rsp, 17 * 8);

    let rflags = read_interrupt_frame_word(rsp, 18 * 8);

    if vector != 0x20 {
        return false;
    }

    if cs != 0x08 {
        return false;
    }

    if (rflags & 0x2) == 0 {
        return false;
    }

    if !is_canonical_address(rip) {
        return false;
    }

    if rip < 0x100000 || rip >= 0x200000 {
        return false;
    }

    true
}

fn debug_interrupt_frame(label: &[u8], rsp: u64) {
    super::serial_write_string(label);

    super::serial_write_string(b"RSP: 0x");

    super::serial_write_hex64(rsp);

    super::serial_write_string(b"\r\n");

    if rsp == 0 {
        super::serial_write_string(b"FRAME: NULL\r\n");
        return;
    }

    let vector = read_interrupt_frame_word(rsp, 15 * 8);

    let rip = read_interrupt_frame_word(rsp, 16 * 8);

    let cs = read_interrupt_frame_word(rsp, 17 * 8);

    let rflags = read_interrupt_frame_word(rsp, 18 * 8);

    super::serial_write_string(b"VECTOR: 0x");

    super::serial_write_hex64(vector);

    super::serial_write_string(b"\r\n");

    super::serial_write_string(b"RIP: 0x");

    super::serial_write_hex64(rip);

    super::serial_write_string(b"\r\n");

    super::serial_write_string(b"CS: 0x");

    super::serial_write_hex64(cs);

    super::serial_write_string(b"\r\n");

    super::serial_write_string(b"RFLAGS: 0x");

    super::serial_write_hex64(rflags);

    super::serial_write_string(b"\r\n");

    if validate_interrupt_frame(rsp) {
        super::serial_write_string(b"FRAME VALIDATION: PASS\r\n");
    } else {
        super::serial_write_string(b"FRAME VALIDATION: FAIL\r\n");
    }
}

// ================================================================
// TIMER INTERRUPT
// ================================================================

pub fn timer_interrupt(current_interrupt_rsp: u64) -> u64 {
    unsafe {
        let scheduler = core::ptr::addr_of_mut!(SCHEDULER);

        let scheduler = &mut *scheduler;

        let current_id = match scheduler.current_task {
            Some(id) => id,

            None => {
                return 0;
            }
        };

        // --------------------------------------------------------
        // Save current task's actual IRQ context.
        // --------------------------------------------------------

        if let Some(task) = scheduler.task_mut(current_id) {
            task.interrupt_rsp = current_interrupt_rsp;

            task.state = TaskState::Running;
        } else {
            return 0;
        }

        // --------------------------------------------------------
        // Account tick.
        // --------------------------------------------------------

        scheduler.tick();

        let slice_expired = match scheduler.task(current_id) {
            Some(task) => task.time_slice_ticks >= DEFAULT_TIME_SLICE_TICKS,

            None => false,
        };

        if !slice_expired {
            return 0;
        }

        // --------------------------------------------------------
        // Select next task.
        // --------------------------------------------------------

        scheduler.schedule();

        let next_id = match scheduler.current_task {
            Some(id) => id,

            None => {
                if let Some(task) = scheduler.task_mut(current_id) {
                    task.state = TaskState::Running;

                    task.time_slice_ticks = 0;
                }

                return 0;
            }
        };

        // --------------------------------------------------------
        // Same task → no switch.
        // --------------------------------------------------------

        if next_id == current_id {
            if let Some(task) = scheduler.task_mut(current_id) {
                task.state = TaskState::Running;

                task.time_slice_ticks = 0;
            }

            return 0;
        }

        // --------------------------------------------------------
        // Get next task's saved interrupt context.
        // --------------------------------------------------------

        let next_rsp = match scheduler.task(next_id) {
            Some(task) => task.interrupt_rsp,

            None => {
                scheduler.current_task = Some(current_id);

                if let Some(task) = scheduler.task_mut(current_id) {
                    task.state = TaskState::Running;

                    task.time_slice_ticks = 0;
                }

                return 0;
            }
        };

        // --------------------------------------------------------
        // Debug.
        // --------------------------------------------------------

        let debug_count = PREEMPT_SWITCH_DEBUG_COUNT.fetch_add(1, Ordering::Relaxed) + 1;

        if debug_count <= 4 {
            super::serial_write_string(b"\r\n*** PREEMPTIVE SWITCH ***\r\n");

            super::serial_write_string(b"CURRENT TASK: 0x");

            super::serial_write_hex64(current_id);

            super::serial_write_string(b"\r\n");

            super::serial_write_string(b"NEXT TASK: 0x");

            super::serial_write_hex64(next_id);

            super::serial_write_string(b"\r\n");

            super::serial_write_string(b"CURRENT IRQ RSP: 0x");

            super::serial_write_hex64(current_interrupt_rsp);

            super::serial_write_string(b"\r\n");

            super::serial_write_string(b"NEXT IRQ RSP: 0x");

            super::serial_write_hex64(next_rsp);

            super::serial_write_string(b"\r\n");

            debug_interrupt_frame(b"NEXT INTERRUPT FRAME\r\n", next_rsp);
        }

        // --------------------------------------------------------
        // Validate destination.
        // --------------------------------------------------------

        if !validate_interrupt_frame(next_rsp) {
            super::serial_write_string(b"\r\n*** PREEMPTIVE SWITCH ABORTED ***\r\n");

            super::serial_write_string(b"INVALID NEXT INTERRUPT FRAME\r\n");

            scheduler.current_task = Some(current_id);

            if let Some(task) = scheduler.task_mut(current_id) {
                task.state = TaskState::Running;

                task.time_slice_ticks = 0;
            }

            if let Some(task) = scheduler.task_mut(next_id) {
                task.state = TaskState::Ready;
            }

            return 0;
        }

        // --------------------------------------------------------
        // Update task states.
        // --------------------------------------------------------

        if let Some(task) = scheduler.task_mut(current_id) {
            task.state = TaskState::Ready;
        }

        if let Some(task) = scheduler.task_mut(next_id) {
            task.state = TaskState::Running;

            task.time_slice_ticks = 0;
        }

        // --------------------------------------------------------
        // Return destination RSP.
        // --------------------------------------------------------

        next_rsp
    }
}

// ================================================================
// RUN SCHEDULER
// ================================================================

pub fn schedule() {
    unsafe {
        let scheduler = core::ptr::addr_of_mut!(SCHEDULER);

        (*scheduler).schedule();
    }
}

// ================================================================
// CREATE INITIAL TASK
// ================================================================

pub fn create_initial_task() -> Option<TaskId> {
    let task_id = allocate_task_id();

    let mut task = Task::new(task_id, 1);

    task.initialize_context(task_bootstrap as *const () as usize as u64);

    unsafe {
        let scheduler = core::ptr::addr_of_mut!(SCHEDULER);

        (*scheduler).create_task(task)
    }
}

// ================================================================
// INITIAL TASK BOOTSTRAP
// ================================================================

extern "C" fn task_bootstrap() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

// ================================================================
// KERNEL → TASK
// ================================================================

unsafe fn switch_kernel_to_task(task_id: TaskId) {
    let scheduler = core::ptr::addr_of_mut!(SCHEDULER);

    let new_context = unsafe {
        match (*scheduler).task(task_id) {
            Some(task) => {
                core::ptr::addr_of!(task.context)
            }

            None => {
                return;
            }
        }
    };

    unsafe {
        (*scheduler).current_task = Some(task_id);
    }

    let kernel_context = core::ptr::addr_of_mut!(TEST_KERNEL_CONTEXT);

    unsafe {
        context::switch(kernel_context, new_context);
    }
}

// ================================================================
// TASK → TASK
// ================================================================

unsafe fn switch_task_to_task(old_task_id: TaskId, new_task_id: TaskId) {
    if old_task_id == new_task_id {
        return;
    }

    let scheduler = core::ptr::addr_of_mut!(SCHEDULER);

    let old_context = unsafe {
        match (*scheduler).task_mut(old_task_id) {
            Some(task) => {
                core::ptr::addr_of_mut!(task.context)
            }

            None => {
                return;
            }
        }
    };

    let new_context = unsafe {
        match (*scheduler).task(new_task_id) {
            Some(task) => {
                core::ptr::addr_of!(task.context)
            }

            None => {
                return;
            }
        }
    };

    unsafe {
        (*scheduler).current_task = Some(new_task_id);
    }

    unsafe {
        context::switch(old_context, new_context);
    }
}

// ================================================================
// TASK → KERNEL
// ================================================================

unsafe fn switch_task_to_kernel(task_id: TaskId) -> ! {
    let scheduler = core::ptr::addr_of_mut!(SCHEDULER);

    let old_context = unsafe {
        match (*scheduler).task_mut(task_id) {
            Some(task) => {
                core::ptr::addr_of_mut!(task.context)
            }

            None => loop {
                core::hint::spin_loop();
            },
        }
    };

    unsafe {
        (*scheduler).current_task = None;
    }

    let kernel_context = core::ptr::addr_of!(TEST_KERNEL_CONTEXT);

    unsafe {
        context::switch(old_context, kernel_context);
    }

    loop {
        core::hint::spin_loop();
    }
}

// ================================================================
// TASK A
// ================================================================

extern "C" fn context_test_task_a() -> ! {
    super::serial_write_string(b"TASK A: START\r\n");

    let task_a = unsafe { TEST_TASK_A_ID };

    let task_b = unsafe { TEST_TASK_B_ID };

    super::serial_write_string(b"TASK A: SWITCH -> TASK B\r\n");

    unsafe {
        switch_task_to_task(task_a, task_b);
    }

    super::serial_write_string(b"TASK A: RESUMED\r\n");

    super::serial_write_string(b"TASK A: SWITCH -> KERNEL\r\n");

    unsafe {
        switch_task_to_kernel(task_a);
    }
}

// ================================================================
// TASK B
// ================================================================

extern "C" fn context_test_task_b() -> ! {
    super::serial_write_string(b"TASK B: START\r\n");

    let task_a = unsafe { TEST_TASK_A_ID };

    let task_b = unsafe { TEST_TASK_B_ID };

    super::serial_write_string(b"TASK B: SWITCH -> TASK A\r\n");

    unsafe {
        switch_task_to_task(task_b, task_a);
    }

    loop {
        core::hint::spin_loop();
    }
}

// ================================================================
// CONTROLLED CONTEXT SWITCH TEST
// ================================================================

pub fn run_context_switch_test() {
    super::serial_write_string(b"\r\n");

    super::serial_write_string(b"================================\r\n");

    super::serial_write_string(b"PHASE 3.3 CONTEXT SWITCH TEST\r\n");

    super::serial_write_string(b"================================\r\n");

    let task_a_id = allocate_task_id();

    let task_b_id = allocate_task_id();

    let mut task_a = Task::new(task_a_id, 1);

    let mut task_b = Task::new(task_b_id, 1);

    let created_a = unsafe {
        let scheduler = core::ptr::addr_of_mut!(SCHEDULER);

        (*scheduler).create_task(task_a)
    };

    if created_a.is_none() {
        super::serial_write_string(b"CONTEXT SWITCH: TASK A CREATE FAILED\r\n");

        return;
    }

    let created_b = unsafe {
        let scheduler = core::ptr::addr_of_mut!(SCHEDULER);

        (*scheduler).create_task(task_b)
    };

    if created_b.is_none() {
        super::serial_write_string(b"CONTEXT SWITCH: TASK B CREATE FAILED\r\n");

        return;
    }

    unsafe {
        TEST_TASK_A_ID = task_a_id;

        TEST_TASK_B_ID = task_b_id;
    }

    super::serial_write_string(b"TASK A ID: 0x");

    super::serial_write_hex64(task_a_id);

    super::serial_write_string(b"\r\n");

    super::serial_write_string(b"TASK B ID: 0x");

    super::serial_write_hex64(task_b_id);

    super::serial_write_string(b"\r\n");

    unsafe {
        switch_kernel_to_task(task_a_id);
    }

    super::serial_write_string(b"CONTEXT SWITCH: PASS\r\n");

    unsafe {
        let scheduler = core::ptr::addr_of_mut!(SCHEDULER);

        (*scheduler).remove_task(task_a_id);

        (*scheduler).remove_task(task_b_id);

        TEST_TASK_A_ID = 0;

        TEST_TASK_B_ID = 0;
    }

    super::serial_write_string(b"CONTEXT TEST TASKS: CLEANED UP\r\n");
}

// ================================================================
// CURRENT TASK
// ================================================================

pub fn current_task() -> Option<TaskId> {
    unsafe {
        let scheduler = core::ptr::addr_of!(SCHEDULER);

        (*scheduler).current()
    }
}

// ================================================================
// TASK COUNT
// ================================================================

pub fn task_count() -> usize {
    unsafe {
        let scheduler = core::ptr::addr_of!(SCHEDULER);

        (*scheduler).task_count()
    }
}

// ================================================================
// TICK COUNT
// ================================================================

pub fn tick_count() -> u64 {
    unsafe {
        let scheduler = core::ptr::addr_of!(SCHEDULER);

        (*scheduler).ticks()
    }
}

// ================================================================
// PHASE 3.4 — PREEMPTIVE SCHEDULING TEST
// ================================================================

static mut PREEMPT_TASK_A_ID: TaskId = 0;
static mut PREEMPT_TASK_B_ID: TaskId = 0;

static PREEMPT_TICKS_A: AtomicU64 = AtomicU64::new(0);

static PREEMPT_TICKS_B: AtomicU64 = AtomicU64::new(0);

// ================================================================
// PREEMPTIVE TASK A
// ================================================================

extern "C" fn preemptive_task_a() -> ! {
    super::serial_write_string(b"PREEMPT A: START\r\n");

    loop {
        let ticks = PREEMPT_TICKS_A.fetch_add(1, Ordering::Relaxed) + 1;

        if ticks == 1 {
            super::serial_write_string(b"PREEMPT A: RUNNING\r\n");
        }

        core::hint::spin_loop();
    }
}

// ================================================================
// PREEMPTIVE TASK B
// ================================================================

extern "C" fn preemptive_task_b() -> ! {
    super::serial_write_string(b"PREEMPT B: START\r\n");

    loop {
        let ticks = PREEMPT_TICKS_B.fetch_add(1, Ordering::Relaxed) + 1;

        if ticks == 1 {
            super::serial_write_string(b"PREEMPT B: RUNNING\r\n");
        }

        core::hint::spin_loop();
    }
}

// ================================================================
// RUN PREEMPTIVE TEST
// ================================================================

pub fn run_preemptive_test() {
    super::serial_write_string(b"\r\nPREEMPTIVE TEST: CREATING TASKS\r\n");

    let task_a_id = allocate_task_id();

    let task_b_id = allocate_task_id();

    let task_a = Task::new(task_a_id, 1);

    let task_b = Task::new(task_b_id, 1);

    let entry_a = preemptive_task_a as *const () as usize as u64;

    let entry_b = preemptive_task_b as *const () as usize as u64;

    // ------------------------------------------------------------
    // Insert TASK A first.
    // ------------------------------------------------------------

    let created_a = unsafe {
        let scheduler = core::ptr::addr_of_mut!(SCHEDULER);

        (*scheduler).create_task(task_a)
    };

    if created_a.is_none() {
        super::serial_write_string(b"PREEMPTIVE TEST: TASK A CREATE FAILED\r\n");

        return;
    }

    // ------------------------------------------------------------
    // TASK A is now stored permanently.
    //
    // IMPORTANT:
    //
    // Task::kernel_stack belongs to the Task stored inside
    // SCHEDULER. Therefore initialize the synthetic interrupt
    // frame only AFTER create_task().
    // ------------------------------------------------------------

    unsafe {
        let scheduler = core::ptr::addr_of_mut!(SCHEDULER);

        if let Some(task) = (*scheduler).task_mut(task_a_id) {
            task.initialize_interrupt_context(entry_a);

            task.prepare_preemptive_start();
        } else {
            super::serial_write_string(b"PREEMPTIVE TEST: TASK A CONTEXT INIT FAILED\r\n");

            (*scheduler).remove_task(task_a_id);

            return;
        }
    }

    // ------------------------------------------------------------
    // Insert TASK B.
    // ------------------------------------------------------------

    let created_b = unsafe {
        let scheduler = core::ptr::addr_of_mut!(SCHEDULER);

        (*scheduler).create_task(task_b)
    };

    if created_b.is_none() {
        super::serial_write_string(b"PREEMPTIVE TEST: TASK B CREATE FAILED\r\n");

        unsafe {
            let scheduler = core::ptr::addr_of_mut!(SCHEDULER);

            (*scheduler).remove_task(task_a_id);
        }

        return;
    }

    // ------------------------------------------------------------
    // TASK B is now stored permanently.
    // ------------------------------------------------------------

    unsafe {
        let scheduler = core::ptr::addr_of_mut!(SCHEDULER);

        if let Some(task) = (*scheduler).task_mut(task_b_id) {
            task.initialize_interrupt_context(entry_b);

            task.prepare_preemptive_start();
        } else {
            super::serial_write_string(b"PREEMPTIVE TEST: TASK B CONTEXT INIT FAILED\r\n");

            (*scheduler).remove_task(task_b_id);

            (*scheduler).remove_task(task_a_id);

            return;
        }
    }

    // ------------------------------------------------------------
    // Store test IDs.
    // ------------------------------------------------------------

    unsafe {
        PREEMPT_TASK_A_ID = task_a_id;

        PREEMPT_TASK_B_ID = task_b_id;
    }

    PREEMPT_TICKS_A.store(0, Ordering::Relaxed);

    PREEMPT_TICKS_B.store(0, Ordering::Relaxed);

    // ------------------------------------------------------------
    // Start TASK A.
    // ------------------------------------------------------------

    unsafe {
        let scheduler = core::ptr::addr_of_mut!(SCHEDULER);

        (*scheduler).set_current(task_a_id);

        if let Some(task) = (*scheduler).task_mut(task_a_id) {
            task.state = TaskState::Running;

            task.time_slice_ticks = 0;
        }
    }

    super::serial_write_string(b"PREEMPTIVE TEST: TASK A READY\r\n");

    super::serial_write_string(b"PREEMPTIVE TEST: TASK B READY\r\n");

    super::serial_write_string(b"PREEMPTIVE TEST: STARTING TASK A\r\n");

    // ------------------------------------------------------------
    // Enter TASK A.
    //
    // IMPORTANT:
    //
    // The task's context.rip has been prepared by
    // prepare_preemptive_start().
    //
    // The context switch therefore enters:
    //
    //     unbound_start_interrupt_context
    //
    // which restores the synthetic interrupt context and finally
    // executes IRETQ into preemptive_task_a().
    // ------------------------------------------------------------

    unsafe {
        switch_kernel_to_task(task_a_id);
    }

    super::serial_write_string(b"PREEMPTIVE TEST: RETURNED TO KERNEL\r\n");
}
