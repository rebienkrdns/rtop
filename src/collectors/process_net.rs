use std::collections::HashMap;
use std::time::Instant;

#[derive(Clone, Default)]
pub struct ProcessNetSnapshot {
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

#[derive(Clone)]
pub struct ProcessNetRate {
    pub rx_bytes_per_sec: f64,
    pub tx_bytes_per_sec: f64,
    pub rx_total: u64,
    pub tx_total: u64,
}

pub struct ProcessNetCollector {
    prev: HashMap<u32, ProcessNetSnapshot>,
    prev_time: Instant,
}

impl ProcessNetCollector {
    pub fn new() -> Self {
        Self {
            prev: HashMap::new(),
            prev_time: Instant::now(),
        }
    }

    pub fn collect(&mut self, pids: &[u32]) -> HashMap<u32, ProcessNetRate> {
        let now = Instant::now();
        let elapsed = now.duration_since(self.prev_time).as_secs_f64().max(0.01);
        self.prev_time = now;

        let current = Self::collect_all(pids);

        let mut rates = HashMap::new();
        for (&pid, snap) in &current {
            let (rx_rate, tx_rate) = match self.prev.get(&pid) {
                Some(prev) => (
                    snap.rx_bytes.saturating_sub(prev.rx_bytes) as f64 / elapsed,
                    snap.tx_bytes.saturating_sub(prev.tx_bytes) as f64 / elapsed,
                ),
                None => (0.0, 0.0),
            };
            rates.insert(
                pid,
                ProcessNetRate {
                    rx_bytes_per_sec: rx_rate,
                    tx_bytes_per_sec: tx_rate,
                    rx_total: snap.rx_bytes,
                    tx_total: snap.tx_bytes,
                },
            );
        }

        self.prev = current;
        rates
    }

    #[cfg(target_os = "linux")]
    fn collect_all(pids: &[u32]) -> HashMap<u32, ProcessNetSnapshot> {
        // Step 1: map socket inode -> pid
        let inode_pid = build_inode_pid_map(pids);
        if inode_pid.is_empty() {
            return HashMap::new();
        }

        // Step 2: query TCP socket byte counters via Netlink sock_diag
        let socket_bytes = query_tcp_bytes().unwrap_or_default();

        // Step 3: aggregate per pid
        let mut result: HashMap<u32, ProcessNetSnapshot> = HashMap::new();
        for (inode, (rx, tx)) in socket_bytes {
            if let Some(&pid) = inode_pid.get(&inode) {
                let e = result.entry(pid).or_default();
                e.rx_bytes += rx;
                e.tx_bytes += tx;
            }
        }

        // Ensure every queried pid has an entry so show_net triggers in the UI
        for &pid in pids {
            result.entry(pid).or_default();
        }

        result
    }

    #[cfg(not(target_os = "linux"))]
    fn collect_all(_pids: &[u32]) -> HashMap<u32, ProcessNetSnapshot> {
        HashMap::new()
    }
}

/// Build a map of socket inode -> pid by scanning /proc/<pid>/fd/.
#[cfg(target_os = "linux")]
fn build_inode_pid_map(pids: &[u32]) -> HashMap<u64, u32> {
    let mut map: HashMap<u64, u32> = HashMap::new();
    for &pid in pids {
        let Ok(dir) = std::fs::read_dir(format!("/proc/{}/fd", pid)) else {
            continue;
        };
        for entry in dir.flatten() {
            let Ok(target) = std::fs::read_link(entry.path()) else {
                continue;
            };
            let s = target.to_string_lossy();
            if let Some(inner) = s
                .strip_prefix("socket:[")
                .and_then(|s| s.strip_suffix("]"))
            {
                if let Ok(inode) = inner.parse::<u64>() {
                    map.entry(inode).or_insert(pid);
                }
            }
        }
    }
    map
}

/// Query per-socket cumulative byte counters via Linux Netlink SOCK_DIAG_BY_FAMILY.
///
/// Uses INET_DIAG_INFO extension which returns a tcp_info struct per socket.
/// Fields used:
///   tcpi_bytes_received (offset 128, kernel ≥ 4.2) — cumulative bytes received
///   tcpi_bytes_acked    (offset 120, kernel ≥ 4.2) — cumulative bytes sent (acked)
///
/// Returns map: socket_inode -> (rx_bytes, tx_bytes).
#[cfg(target_os = "linux")]
fn query_tcp_bytes() -> std::io::Result<HashMap<u64, (u64, u64)>> {
    use libc::{
        c_int, c_void, recv, send, AF_INET, AF_INET6, AF_NETLINK, IPPROTO_TCP, SOCK_CLOEXEC,
        SOCK_DGRAM,
    };
    use std::{io, mem, ptr};

    const NETLINK_INET_DIAG: c_int = 4;
    const SOCK_DIAG_BY_FAMILY: u16 = 20;
    const NLMSG_DONE: u16 = 3;
    const NLM_F_REQUEST: u16 = 0x01;
    const NLM_F_DUMP: u16 = 0x300;
    const INET_DIAG_INFO: u16 = 2;
    // Offsets within tcp_info for byte counters (kernel >= 4.2)
    const TCPI_BYTES_ACKED_OFF: usize = 120;
    const TCPI_BYTES_RECEIVED_OFF: usize = 128;

    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct NlMsgHdr {
        len: u32,
        msg_type: u16,
        flags: u16,
        seq: u32,
        pid: u32,
    }

    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct InetDiagSockId {
        sport: u16,
        dport: u16,
        src: [u32; 4],
        dst: [u32; 4],
        iface: u32,
        cookie: [u32; 2],
    }

    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct InetDiagReqV2 {
        family: u8,
        protocol: u8,
        ext: u8,
        pad: u8,
        states: u32,
        id: InetDiagSockId,
    }

    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct InetDiagMsg {
        family: u8,
        state: u8,
        timer: u8,
        retrans: u8,
        id: InetDiagSockId,
        expires: u32,
        rqueue: u32,
        wqueue: u32,
        uid: u32,
        inode: u32,
    }

    #[repr(C)]
    struct NlAttr {
        len: u16,
        attr_type: u16,
    }

    // Open Netlink socket
    let fd = unsafe { libc::socket(AF_NETLINK, SOCK_DGRAM | SOCK_CLOEXEC, NETLINK_INET_DIAG) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    struct FdGuard(c_int);
    impl Drop for FdGuard {
        fn drop(&mut self) {
            unsafe { libc::close(self.0) };
        }
    }
    let _guard = FdGuard(fd);

    let mut result: HashMap<u64, (u64, u64)> = HashMap::new();

    for &family in &[AF_INET as u8, AF_INET6 as u8] {
        #[repr(C)]
        struct Request {
            hdr: NlMsgHdr,
            body: InetDiagReqV2,
        }

        let req = Request {
            hdr: NlMsgHdr {
                len: mem::size_of::<Request>() as u32,
                msg_type: SOCK_DIAG_BY_FAMILY,
                flags: NLM_F_REQUEST | NLM_F_DUMP,
                seq: 1,
                pid: 0,
            },
            body: InetDiagReqV2 {
                family,
                protocol: IPPROTO_TCP as u8,
                ext: (1u8 << (INET_DIAG_INFO - 1)),
                states: 0xffff_ffff,
                ..Default::default()
            },
        };

        let sent = unsafe {
            send(
                fd,
                &req as *const _ as *const c_void,
                mem::size_of::<Request>(),
                0,
            )
        };
        if sent < 0 {
            continue;
        }

        let mut buf = vec![0u8; 131_072]; // 128 KiB receive buffer
        'recv: loop {
            let n = unsafe { recv(fd, buf.as_mut_ptr() as *mut c_void, buf.len(), 0) };
            if n <= 0 {
                break;
            }
            let n = n as usize;
            let mut pos = 0usize;

            while pos + mem::size_of::<NlMsgHdr>() <= n {
                let hdr: NlMsgHdr =
                    unsafe { ptr::read_unaligned(buf.as_ptr().add(pos) as *const NlMsgHdr) };
                let msg_len = hdr.len as usize;
                if msg_len < mem::size_of::<NlMsgHdr>() || pos + msg_len > n {
                    break;
                }

                match hdr.msg_type {
                    NLMSG_DONE => break 'recv,
                    SOCK_DIAG_BY_FAMILY => {
                        let payload_start = pos + mem::size_of::<NlMsgHdr>();
                        if payload_start + mem::size_of::<InetDiagMsg>() <= pos + msg_len {
                            let diag: InetDiagMsg = unsafe {
                                ptr::read_unaligned(
                                    buf.as_ptr().add(payload_start) as *const InetDiagMsg,
                                )
                            };
                            let inode = diag.inode as u64;

                            // Walk NLA attributes for INET_DIAG_INFO
                            let mut apos =
                                payload_start + mem::size_of::<InetDiagMsg>();
                            let msg_end = pos + msg_len;
                            while apos + mem::size_of::<NlAttr>() <= msg_end {
                                let attr: NlAttr = unsafe {
                                    ptr::read_unaligned(
                                        buf.as_ptr().add(apos) as *const NlAttr,
                                    )
                                };
                                let alen = attr.len as usize;
                                if alen < mem::size_of::<NlAttr>() {
                                    break;
                                }
                                if attr.attr_type == INET_DIAG_INFO {
                                    let data = apos + mem::size_of::<NlAttr>();
                                    let data_len = alen - mem::size_of::<NlAttr>();
                                    let rx = read_u64(&buf, data, TCPI_BYTES_RECEIVED_OFF, data_len);
                                    let tx = read_u64(&buf, data, TCPI_BYTES_ACKED_OFF, data_len);
                                    if rx.is_some() || tx.is_some() {
                                        let e = result.entry(inode).or_insert((0, 0));
                                        e.0 += rx.unwrap_or(0);
                                        e.1 += tx.unwrap_or(0);
                                    }
                                }
                                // NLA padding: align to 4 bytes
                                apos += (alen + 3) & !3;
                            }
                        }
                    }
                    _ => {}
                }

                // NLMSG padding: align to 4 bytes
                pos += (msg_len + 3) & !3;
            }
        }
    }

    Ok(result)
}

#[cfg(target_os = "linux")]
#[inline]
fn read_u64(buf: &[u8], base: usize, offset: usize, data_len: usize) -> Option<u64> {
    let start = base + offset;
    if offset + 8 > data_len || start + 8 > buf.len() {
        return None;
    }
    Some(u64::from_ne_bytes(buf[start..start + 8].try_into().ok()?))
}
