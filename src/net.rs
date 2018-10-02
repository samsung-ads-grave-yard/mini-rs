use std::ffi::CString;
use std::io;
use std::mem;
use std::net;
use std::net::TcpStream;
use std::os::unix::io::RawFd;
use std::ptr;

pub mod tcp {
    use std::io;
    use std::mem;
    use std::os::unix::io::RawFd;
    use std::sync::Arc;

    use actor::{
        Pid,
        ProcessContinuation,
        ProcessQueue,
        SpawnParameters,
    };
    use async;
    use async::Mode;
    use self::Msg::*;
    use super::{AddrInfoIter, close, connect, ffi, getaddrinfo, socket};
    use super::ffi::ErrNo;

    #[derive(Debug)]
    enum Msg {
        ConnectToHost(RawFd),
        ProgressConnectToHost(RawFd),
        TryingConnectionToHost(AddrInfoIter),
    }

    pub fn connect_to_host<M, MSG>(host: &str, port: &str, process_queue: &Arc<ProcessQueue>,
        event_loop: &Pid<async::Msg>, receiver: &Pid<MSG>, message: M) -> io::Result<()>
    where M: Fn(RawFd) -> MSG + Send + 'static,
          MSG: Send + 'static,
    {
        let receiver = receiver.clone();
        let event_loop = event_loop.clone();
        let handler = move |current: &Pid<_>, msg: Option<Msg>| {
            if let Some(msg) = msg {
                /*match msg {
                    ConnectToHost(_) => println!("Connected to host"),
                    TryingConnectionToHost(mut address_infos) => {
                        println!("Trying connection to host");
                        let address_info = address_infos.next().expect("address_infos");
                        match socket(address_info.ai_family, address_info.ai_socktype | ffi::SOCK_NONBLOCK,
                                          address_info.ai_protocol)
                        {
                            Ok(fd) => {
                                println!("D");
                                match connect(fd, address_info.ai_addr, address_info.ai_addrlen) {
                                    Ok(_) => {
                                        ProcessQueue::send_message(&receiver, message(fd));
                                    },
                                    Err(ref error) if error.raw_os_error() == Some(ErrNo::InProgress as i32) => {
                                        /*ProcessQueue::send_message(&event_loop,
                                            AddFd(fd, Mode::Write, ProgressConnectToHost(fd)));*/
                                        println!("Register the event");
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
                                // TODO: try next elements in the iterator.
                            },
                        }
                    },
                }*/
            }
            ProcessContinuation::WaitMessage
        };

        let mut hints: ffi::addrinfo = unsafe { mem::zeroed() };
        hints.ai_socktype = ffi::SOCK_STREAM as i32;
        // TODO: use getaddrinfo_a which is asynchronous.
        let address_infos = getaddrinfo(Some(host), Some(port), Some(hints))
            .map_err(|()| io::Error::from(io::ErrorKind::NotFound))?;
        let pid = process_queue.blocking_spawn(SpawnParameters {
            handler,
            message_capacity: 1,
            max_message_per_cycle: 1,
        });
        pid.send_message(TryingConnectionToHost(address_infos));

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
    let option_len = mem::size_of_val(&option_value);
    let error = unsafe { ffi::getsockopt(socket, level, name, &mut option_value as *mut i32 as *mut _, option_len as i32) };
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

        pub fn getsockopt(socket: i32, level: i32, option_name: i32, option_value: *mut c_void, option_len: socklen_t)
            -> i32;
        pub fn socket(domain: i32, typ: i32, protocol: i32) -> i32;
    }
}
