use std::io;
use std::os::windows::io::{AsRawSocket, RawSocket};
#[cfg(feature = "io_timeout")]
use std::time::Duration;

use super::super::miow::socket_write;
use super::super::{co_io_result, EventData};
use crate::coroutine_impl::{is_coroutine, CoroutineImpl, EventSource};
use crate::scheduler::get_scheduler;
use windows_sys::Win32::Foundation::*;

pub struct SocketWrite<'a> {
    io_data: EventData,
    buf: &'a [u8],
    socket: RawSocket,
    #[cfg(feature = "io_timeout")]
    timeout: Option<Duration>,
    pub(crate) is_coroutine: bool,
}

impl<'a> SocketWrite<'a> {
    pub fn new<T: AsRawSocket>(
        s: &T,
        buf: &'a [u8],
        #[cfg(feature = "io_timeout")] timeout: Option<Duration>,
    ) -> Self {
        let socket = s.as_raw_socket();
        SocketWrite {
            io_data: EventData::new(socket as HANDLE),
            buf,
            socket,
            #[cfg(feature = "io_timeout")]
            timeout,
            is_coroutine: is_coroutine(),
        }
    }

    pub fn done(&mut self) -> io::Result<usize> {
        co_io_result(&self.io_data, self.is_coroutine)
    }
}

impl EventSource for SocketWrite<'_> {
    fn subscribe(&mut self, co: CoroutineImpl) {
        let s = get_scheduler();
        #[cfg(feature = "io_timeout")]
        if let Some(dur) = self.timeout {
            s.get_selector().add_io_timer(&mut self.io_data, dur);
        }

        // prepare the co first
        self.io_data.co = Some(co);
        // call the overlapped write API
        co_try!(s, self.io_data.co.take().expect("can't get co"), unsafe {
            socket_write(self.socket, self.buf, self.io_data.get_overlapped())
        });
    }
}
