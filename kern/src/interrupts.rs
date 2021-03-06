use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode, HandlerFunc};
use crate::gdt::DOUBLE_FAULT_IST_INDEX;
use lazy_static::lazy_static;
use crate::hardware::pic::ChainedPics;
use spin::MutexGuard;
use x86_64::instructions::hlt;
use crate::{FRAME_ALLOC, PAGE_TABLE};
use x86_64::structures::paging::{PageTable, Mapper, FrameAllocator, Page, PageTableFlags};
use core::borrow::BorrowMut;
use crate::memory::frame_allocator::FrameAllocWrapper;
use crate::hardware::apic::end_of_interrupt;
use crate::interrupts::context_switch::save_context;

pub mod context_switch;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.general_protection_fault.set_handler_fn(gp_fault_handler);
        unsafe {
            idt.double_fault.set_handler_fn(double_fault_handler)
                .set_stack_index(DOUBLE_FAULT_IST_INDEX as u16);
        }
        idt.page_fault.set_handler_fn(page_fault_handler);
        idt[InterruptIndex::Timer.as_usize()].set_handler_fn(timer_interrupt_handler);
        idt[InterruptIndex::Keyboard.as_usize()].set_handler_fn(keyboard_interrupt_handler);
        idt[InterruptIndex::ApicTimer.as_usize()].set_handler_addr(save_context as u64);
        idt
    };
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC1_OFFSET + 0,
    Keyboard = PIC1_OFFSET + 1,
    ApicTimer = 0x30,
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }

    fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

pub const PIC1_OFFSET: u8 = 0x20;
pub const PIC2_OFFSET: u8 = PIC1_OFFSET + 8;

pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC1_OFFSET, PIC2_OFFSET) });

pub fn init_idt() {
    IDT.load();
}

extern "x86-interrupt" fn page_fault_handler(stack_frame: &mut InterruptStackFrame, ec: PageFaultErrorCode) {
    use x86_64::registers::control::Cr2;

    let faulting_addr = Cr2::read();

    println!("PAGE FAULT");
    println!("Faulting ADDR: {:?}", faulting_addr);
    println!("Error Code {:?}", ec);
    println!("{:#?}", stack_frame);

    let phys_frame = FRAME_ALLOC.lock().allocate_frame().expect("No Space");
    let mut lmao = FrameAllocWrapper{};

    unsafe {
        PAGE_TABLE.lock().map_to(
            Page::containing_address(faulting_addr),
            phys_frame,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
            &mut lmao
        )
    }.expect("Can't map").flush();
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: &mut InterruptStackFrame) {
    use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
    use spin::Mutex;
    use x86_64::instructions::port::Port;

    lazy_static! {
        static ref KEYBOARD: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> =
            Mutex::new(Keyboard::new(layouts::Us104Key, ScancodeSet1,
                HandleControl::MapLettersToUnicode)
            );
    }


    let mut ps2_port = Port::new(0x60); // PS2 Port
    let mut keyboard: MutexGuard<Keyboard<layouts::Us104Key, ScancodeSet1>> = KEYBOARD.lock();
    let scancode: u8 = unsafe { ps2_port.read() };

    if let Ok(Some(key_evnt)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_evnt) {
            match key {
                DecodedKey::Unicode(c) => crate::hardware::keyboard::put_char(c),
                _ => {}
                // DecodedKey::RawKey(key) => println!("raw: {:?}", key),
            }
        }
    }
    unsafe {
        PICS.lock().notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

extern "x86-interrupt" fn apic_timer(_stack_frame: &mut InterruptStackFrame) {
    println!("Timer APIC");
    end_of_interrupt();
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: &mut InterruptStackFrame) {
    unsafe {
        PICS.lock().notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}

extern "x86-interrupt" fn breakpoint_handler(tf: &mut InterruptStackFrame) {
    println!("TRAP: break\n{:#?}", tf);
}

extern "x86-interrupt" fn gp_fault_handler(
    stack_frame: &mut InterruptStackFrame, _error_code: u64)
{
    panic!("GPFault {}\n{:#?}", _error_code, stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: &mut InterruptStackFrame, _error_code: u64) -> !
{
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}