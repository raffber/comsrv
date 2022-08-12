use std::{
    mem,
    os::{raw::c_schar, unix::prelude::AsRawFd},
};

use anyhow::anyhow;
use libc::{c_char, c_int, c_short, c_uint, c_ulong};

#[repr(C)]
struct SerialStruct {
    typ: c_int,
    line: c_int,
    port: c_uint,
    irq: c_int,
    flags: c_int,
    xmit_fifo_size: c_int,
    custom_divisor: c_int,
    baud_rate: c_int,
    close_delay: c_short,
    io_type: c_schar,
    reserved_char: c_schar,
    hub6: c_int,
    closing_wait: c_short,
    closing_wait2: c_short,
    iomem_base: *mut c_char,
    iomem_reg_shift: c_short,
    port_high: c_int,
    iomap_base: c_ulong,
}

pub(crate) fn apply_low_latency<T: AsRawFd>(serial_stream: &T) -> crate::Result<()> {
    let fd = serial_stream.as_raw_fd();

    unsafe {
        let mut serial_struct: SerialStruct = mem::zeroed();
        let ss_ref = &mut serial_struct as *mut SerialStruct;
        let failed = libc::ioctl(fd, libc::TIOCGSERIAL, ss_ref);
        if failed != 0 {
            return Err(crate::Error::transport(anyhow!("Cannot get serial info struct")));
        }

        serial_struct.flags |= 1 << 13;
        const TIOCSSERIAL: c_ulong = 0x541F;
        let failed = libc::ioctl(fd, TIOCSSERIAL, ss_ref);
        if failed != 0 {
            return Err(crate::Error::transport(anyhow!("Cannot set low latency")));
        }
    }

    Ok(())
}
