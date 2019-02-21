use std::ffi::CString;
use std::io;
use std::mem;
use std::os::unix::io::RawFd;
use std::ptr;

pub mod tcp {
    use std::io;
    use std::mem;
    use std::net::TcpStream;
    use std::os::unix::io::FromRawFd;

    use actor::{
        Pid,
        ProcessContinuation,
        ProcessQueue,
        SpawnParameters,
    };
    use async::{
        EventLoop,
        Mode,
        TcpConnection,
        TcpConnectionNotify,
        manage_connection,
    };
    use self::ffi::ErrNo;
    use self::Msg::*;
    use super::{
        AddrInfoIter,
        close,
        connect,
        ffi,
        getaddrinfo,
        getsockopt,
        socket,
    };

    #[derive(Debug)]
    enum Msg<CONNECTION>
    where CONNECTION: TcpConnectionNotify,
    {
        TryingConnectionToHost(AddrInfoIter, u32, CONNECTION),
    }

    pub fn connect_to_host<CONNECTION>(host: &str, port: &str, process_queue: &ProcessQueue,
        event_loop: &EventLoop, mut connection_notify: CONNECTION) -> io::Result<()>
    where CONNECTION: TcpConnectionNotify + Send + 'static,
    {
        let event_loop = event_loop.clone();
        let handler = move |current: &Pid<_>, msg: Option<Msg<CONNECTION>>| {
            if let Some(msg) = msg {
                match msg {
                    TryingConnectionToHost(mut address_infos, count, mut connection_notify) => {
                        match address_infos.next() {
                            Some(address_info) => {
                                match socket(address_info.ai_family, address_info.ai_socktype | ffi::SOCK_NONBLOCK,
                                                  address_info.ai_protocol)
                                {
                                    Ok(fd) => {
                                        let stream = unsafe { TcpStream::from_raw_fd(fd) };
                                        stream.set_nonblocking(true); // TODO: handle error. FIXME: remove since it's already non-blocking?
                                        let mut connection = TcpConnection::new(stream);
                                        connection_notify.connecting(&mut connection, count); // FIXME: send right count.
                                        match connect(fd, address_info.ai_addr, address_info.ai_addrlen) {
                                            Ok(()) => manage_connection(&event_loop, connection, Box::new(connection_notify)),
                                            Err(ref error) if error.raw_os_error() == Some(ErrNo::InProgress as i32) => {
                                                let current = current.clone();
                                                let eloop = event_loop.clone();
                                                event_loop.add_raw_fd_oneshot(fd, Mode::Write, move |event| {
                                                    if event.events & Mode::Write as u32 != 0 {
                                                        let result = getsockopt(fd, ffi::SOL_SOCKET, ffi::SO_ERROR);
                                                        match result {
                                                            Ok(value) if value != 0 => {
                                                                // TODO: should we close(fd) here?
                                                                current.send_message(TryingConnectionToHost(address_infos, count + 1, connection_notify));
                                                            },
                                                            Ok(_) => {
                                                                eloop.remove_raw_fd(fd).expect("remove raw fd");
                                                                manage_connection(&eloop, connection, Box::new(connection_notify))
                                                            },
                                                            Err(err) => {
                                                                close(fd).expect("close fd"); // TODO: handle error.
                                                                current.send_message(TryingConnectionToHost(address_infos, count + 1, connection_notify));
                                                            },
                                                        }
                                                    }
                                                });
                                            },
                                            Err(error) => {
                                                println!("Error connect: {:?}", error.raw_os_error());
                                                close(fd).expect("close fd"); // TODO: handle error.
                                                // TODO: try next elements in the iterator.
                                            },
                                        }
                                    },
                                    Err(error) => {
                                        println!("Error: {}", error);
                                        current.send_message(TryingConnectionToHost(address_infos, count + 1, connection_notify));
                                    },
                                }
                            },
                            None => connection_notify.connect_failed(),
                        }
                    },
                }
            }
            ProcessContinuation::WaitMessage
        };

        let mut hints: ffi::addrinfo = unsafe { mem::zeroed() };
        hints.ai_socktype = ffi::SOCK_STREAM as i32;
        // TODO: use getaddrinfo_a which is asynchronous. Maybe not: https://medium.com/where-the-flamingcow-roams/asynchronous-name-resolution-in-c-268ff5df3081
        let address_infos = getaddrinfo(Some(host), Some(port), Some(hints))
            .map_err(|()| io::Error::from(io::ErrorKind::NotFound))?;
        let pid = process_queue.blocking_spawn(SpawnParameters {
            handler,
            message_capacity: 2,
            max_message_per_cycle: 1,
        });
        pid.send_message(TryingConnectionToHost(address_infos, 0, connection_notify));

        /*for address_info in address_infos {
            match socket(address_info.ai_family, address_info.ai_socktype | ffi::SOCK_NONBLOCK,
                         address_info.ai_protocol)
            {
                Ok(fd) => {
                    println!("D");
                    match connect(fd, address_info.ai_addr, address_info.ai_addrlen) {
                        Ok(_) => {
                            event_loop.send(ConnectToHost(fd));
                            break;
                        },
                        Err(ref error) if error.raw_os_error() == Some(ErrNo::InProgress as i32) => {
                            // FIXME: should await here.
                            event_loop.add_raw_fd(fd, Mode::Write, ConnectToHost(fd));
                            /*event_loop.add_raw_fd(fd, Mode::Write, |mode| {
                                if mode == Mode::Write as i32 {
                                    /*
                                     * TODO
                                     int result;
                                     socklen_t result_len = sizeof(result);
                                     if (getsockopt(fd, SOL_SOCKET, SO_ERROR, &result, &result_len) < 0) {
                                    // error, fail somehow, close socket
                                    return;
                                    }

                                    if (result != 0) {
                                    // connection failed; error code is in 'result'
                                    return;
                                    }
                                    */
                                    event_loop.send(ConnectToHost(fd));
                                }
                                // TODO: check for EPOLLERR and EPOLLHUP.
                            });*/
                            println!("Register the event");
                        },
                        Err(error) => {
                            println!("Error connect: {:?}", error.raw_os_error());
                            close(fd)?;
                        },
                    }
                },
                Err(error) => {
                    println!("Error: {}", error);
                    continue
                },
            }
        }*/
        Err(io::Error::from(io::ErrorKind::NotFound))
    }
}

#[derive(Debug)]
pub struct AddrInfoIter {
    address_infos: *mut ffi::addrinfo,
}

unsafe impl Send for AddrInfoIter {}

impl AddrInfoIter {
    fn new(address_infos: *mut ffi::addrinfo) -> Self {
        Self {
            address_infos,
        }
    }
}

impl Iterator for AddrInfoIter {
    type Item = ffi::addrinfo;

    fn next(&mut self) -> Option<Self::Item> {
        if self.address_infos.is_null() {
            return None;
        }
        let result = unsafe { ptr::read(self.address_infos) };
        self.address_infos = unsafe { (*self.address_infos).ai_next };
        Some(result)
    }
}

impl Drop for AddrInfoIter {
    fn drop(&mut self) {
        unsafe { ffi::freeaddrinfo(self.address_infos) };
    }
}

pub fn close(fd: RawFd) -> io::Result<()> {
    if unsafe { ffi::close(fd) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub fn connect(socket: RawFd, address: *const ffi::sockaddr, address_len: ffi::socklen_t) -> io::Result<()> {
    if unsafe { ffi::connect(socket, address, address_len) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub fn getaddrinfo(hostname: Option<&str>, service: Option<&str>, hints: Option<ffi::addrinfo>) ->
    Result<AddrInfoIter, ()>
{
    let hints = hints.as_ref().map(|hints| hints as *const _).unwrap_or_else(|| ptr::null());
    let mut address_infos = ptr::null_mut();
    let hostname = to_c_string(hostname);
    let service = to_c_string(service);
    let result = unsafe { ffi::getaddrinfo(hostname, service, hints, &mut address_infos) };
    unsafe {
        // Free memory.
        CString::from_raw(hostname as *mut _);
        CString::from_raw(service as *mut _);
    }
    if result == 0 {
        Ok(AddrInfoIter::new(address_infos))
    }
    else {
        println!("Error: {}", result);
        // FIXME: return error.
        Err(())
    }
}

pub fn socket(domain: i32, typ: i32, protocol: i32) -> io::Result<RawFd> {
    let result = unsafe { ffi::socket(domain, typ, protocol) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(result)
}

fn to_c_string(string: Option<&str>) -> *const i8 {
    match string {
        Some(string) => {
            let string = CString::new(string).expect("nul byte found in hostname argument to getaddrinfo");
            string.into_raw()
        },
        None => {
            ptr::null()
        },
    }
}

pub fn getsockopt(socket: RawFd, level: i32, name: i32) -> io::Result<i32> {
    let mut option_value = 0i32;
    let mut option_len = mem::size_of_val(&option_value) as i32;
    let error = unsafe { ffi::getsockopt(socket, level, name, &mut option_value as *mut i32 as *mut _, &mut option_len as *mut i32) };
    if error == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(option_value)
}

pub mod ffi {
    #![allow(non_camel_case_types)]

    use std::os::raw::c_void;

    #[repr(i32)]
    pub enum ErrNo {
        InProgress = 115,
    }

    pub const SOL_SOCKET: i32 = 1;
    pub const SO_ERROR: i32 = 4;

    pub const SOCK_STREAM: i32 = 1;
    pub const SOCK_DGRAM: i32 = 2;
    pub const SOCK_NONBLOCK: i32 = 0o4000;

    pub enum sockaddr {
    }

    pub type socklen_t = i32;

    #[repr(C)]
    pub struct addrinfo {
        pub ai_flags: i32,
        pub ai_family: i32,
        pub ai_socktype: i32,
        pub ai_protocol: i32,
        pub ai_addrlen: socklen_t,
        pub ai_addr: *mut sockaddr,
        pub ai_canonname: *mut i8,
        pub ai_next: *mut addrinfo,
    }

    extern "C" {
        pub fn connect(socket: i32, address: *const sockaddr, address_len: socklen_t) -> i32;

        pub fn close(fildes: i32) -> i32;

        pub fn freeaddrinfo(res: *mut addrinfo);
        pub fn getaddrinfo(node: *const i8, service: *const i8, hints: *const addrinfo, result: *mut *mut addrinfo)
            -> i32;

        pub fn getsockopt(socket: i32, level: i32, option_name: i32, option_value: *mut c_void, option_len: *mut socklen_t)
            -> i32;
        pub fn socket(domain: i32, typ: i32, protocol: i32) -> i32;
    }
}
