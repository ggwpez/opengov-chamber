#[global_allocator]
static mut ALLOC: picoalloc::Mutex<picoalloc::Allocator<picoalloc::ArrayPointer<16384>>> = {
    static mut ARRAY: picoalloc::Array<16384> = picoalloc::Array([0u8; 16384]);

    picoalloc::Mutex::new(picoalloc::Allocator::new(unsafe {
        picoalloc::ArrayPointer::new(&raw mut ARRAY)
    }))
};

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // Safety: The unimp instruction is guaranteed to trap
    unsafe {
        core::arch::asm!("unimp");
        core::hint::unreachable_unchecked();
    }
}
