use std::collections::BTreeSet;
use std::fmt::Write as _;

use super::Channels;

pub const CHAN_MOD: &str = "chan";

pub fn emit_chan_file(imports: &str, cap: usize, ports: &str) -> String {
    let mut out = String::new();
    out.push_str("#![allow(warnings)]\n");
    out.push_str(imports);
    out.push('\n');
    let _ = writeln!(out, "pub const CAP: usize = {cap};\n");
    out.push_str(ports);
    out
}

pub fn chan_use() -> String {
    format!("mod {CHAN_MOD};\npub use {CHAN_MOD}::*;\n\n")
}

pub fn local_chan_imports(cap: usize) -> &'static str {
    if cap == 0 {
        "use std::cell::Cell;\nuse std::cell::RefCell;\nuse std::collections::VecDeque;\nuse std::rc::Rc;\n"
    } else {
        "use std::cell::Cell;\nuse std::collections::VecDeque;\nuse std::rc::Rc;\n"
    }
}

pub fn ring_channels(cap: usize) -> Channels {
    if cap == 0 {
        Channels::Crossbeam
    } else {
        Channels::Spsc
    }
}

pub fn emit_ring_ports(cap: usize, types: &BTreeSet<String>) -> String {
    if cap == 0 {
        CROSSBEAM_PORTS.to_string()
    } else {
        spsc_ports(types)
    }
}

pub fn ring_imports(cap: usize) -> &'static str {
    if cap == 0 {
        "use std::collections::VecDeque;\n"
    } else {
        "use std::collections::VecDeque;\nuse std::sync::Arc;\nuse std::sync::atomic::{AtomicUsize, Ordering};\n"
    }
}

pub fn ring_main_imports(cap: usize) -> &'static str {
    if cap == 0 {
        "use std::collections::VecDeque;\n"
    } else {
        "use std::collections::VecDeque;\nuse std::sync::Arc;\n"
    }
}

pub fn ring_deps(cap: usize) -> &'static str {
    if cap == 0 {
        "crossbeam-channel = \"0.5\"\n"
    } else {
        ""
    }
}

pub fn local_ports(cap: usize, types: &BTreeSet<String>) -> String {
    let chan = if cap == 0 {
        LOCAL_UNBOUNDED_CHAN
    } else {
        LOCAL_RING_CHAN
    };
    let impls: String = types.iter().map(|ty| local_slot_impl(ty)).collect();
    format!(
        "{}{impls}\n{chan}\n{}",
        slot_trait(false),
        port_wrappers("Rc", "Slot")
    )
}

fn local_slot_impl(ty: &str) -> String {
    let get = match ty {
        "i64" | "bool" | "f64" => "        cell.get()".to_string(),
        _ => "        let value = cell.take();\n        cell.set(value.clone());\n        value"
            .to_string(),
    };
    format!(
        r"impl Slot for {ty} {{
    type Cell = Cell<{ty}>;
    #[inline(always)]
    fn cell(value: Self) -> Self::Cell {{
        Cell::new(value)
    }}
    #[inline(always)]
    fn put(cell: &Self::Cell, value: Self) {{
        cell.set(value);
    }}
    #[inline(always)]
    fn get(cell: &Self::Cell) -> Self {{
{get}
    }}
}}

"
    )
}

const LOCAL_RING_CHAN: &str = r"pub struct Chan<T: Slot> {
    buf: Box<[T::Cell]>,
    slots: usize,
    r: Cell<usize>,
    w: Cell<usize>,
}

impl<T: Slot> Chan<T> {
    pub fn new() -> Self {
        let slots = CAP + 1;
        Self {
            buf: (0..slots)
                .map(|_| T::cell(T::default()))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            slots,
            r: Cell::new(0),
            w: Cell::new(0),
        }
    }
    #[inline(always)]
    pub fn len(&self) -> usize {
        let (r, w) = (self.r.get(), self.w.get());
        if w >= r { w - r } else { self.slots + w - r }
    }
    #[inline(always)]
    pub fn room(&self) -> usize {
        CAP - self.len()
    }
    #[inline(always)]
    pub fn try_push(&self, value: T) -> bool {
        let w = self.w.get();
        let next = if w + 1 == self.slots { 0 } else { w + 1 };
        if next == self.r.get() {
            return false;
        }
        T::put(&self.buf[w], value);
        self.w.set(next);
        true
    }
    #[inline(always)]
    pub fn pop(&self) -> T {
        let r = self.r.get();
        let value = T::get(&self.buf[r]);
        self.r.set(if r + 1 == self.slots { 0 } else { r + 1 });
        value
    }
    #[inline(always)]
    pub fn at(&self, index: usize) -> T {
        let mut k = self.r.get() + index;
        if k >= self.slots {
            k -= self.slots;
        }
        T::get(&self.buf[k])
    }
}
";

const LOCAL_UNBOUNDED_CHAN: &str = r"pub struct Chan<T: Slot> {
    buf: RefCell<VecDeque<T>>,
}

impl<T: Slot> Chan<T> {
    pub fn new() -> Self {
        Self { buf: RefCell::new(VecDeque::new()) }
    }
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.buf.borrow().len()
    }
    #[inline(always)]
    pub fn room(&self) -> usize {
        usize::MAX
    }
    #[inline(always)]
    pub fn try_push(&self, value: T) -> bool {
        self.buf.borrow_mut().push_back(value);
        true
    }
    #[inline(always)]
    pub fn pop(&self) -> T {
        self.buf.borrow_mut().pop_front().unwrap()
    }
    #[inline(always)]
    pub fn at(&self, index: usize) -> T {
        self.buf.borrow()[index].clone()
    }
}
";

fn spsc_ports(types: &BTreeSet<String>) -> String {
    let impls: String = types.iter().map(|ty| slot_impl(ty)).collect();
    format!(
        "{}{impls}\n{SPSC_CHAN}\n{}",
        slot_trait(true),
        port_wrappers("Arc", "Slot")
    )
}

fn slot_trait(shared: bool) -> String {
    let cell = if shared {
        "type Cell: Send + Sync;"
    } else {
        "type Cell;"
    };
    format!(
        r"pub trait Slot: Clone + Default {{
    {cell}
    fn cell(value: Self) -> Self::Cell;
    fn put(cell: &Self::Cell, value: Self);
    fn get(cell: &Self::Cell) -> Self;
}}

"
    )
}

fn slot_impl(ty: &str) -> String {
    match ty {
        "i64" => atomic_slot("i64", "AtomicI64", "value", "value"),
        "bool" => atomic_slot("bool", "AtomicBool", "value", "value"),
        "f64" => atomic_slot(
            "f64",
            "AtomicU64",
            "value.to_bits()",
            "f64::from_bits(value)",
        ),
        _ => format!(
            r"impl Slot for {ty} {{
    type Cell = std::sync::Mutex<{ty}>;
    #[inline(always)]
    fn cell(value: Self) -> Self::Cell {{
        std::sync::Mutex::new(value)
    }}
    #[inline(always)]
    fn put(cell: &Self::Cell, value: Self) {{
        *cell.lock().unwrap() = value;
    }}
    #[inline(always)]
    fn get(cell: &Self::Cell) -> Self {{
        cell.lock().unwrap().clone()
    }}
}}

"
        ),
    }
}

fn atomic_slot(ty: &str, cell: &str, encode: &str, decode: &str) -> String {
    format!(
        r"impl Slot for {ty} {{
    type Cell = std::sync::atomic::{cell};
    #[inline(always)]
    fn cell(value: Self) -> Self::Cell {{
        std::sync::atomic::{cell}::new({encode})
    }}
    #[inline(always)]
    fn put(cell: &Self::Cell, value: Self) {{
        cell.store({encode}, Ordering::Relaxed);
    }}
    #[inline(always)]
    fn get(cell: &Self::Cell) -> Self {{
        let value = cell.load(Ordering::Relaxed);
        {decode}
    }}
}}

"
    )
}

const SPSC_CHAN: &str = r"#[repr(align(128))]
pub struct Side {
    index: AtomicUsize,
    cache: AtomicUsize,
}

impl Side {
    fn new() -> Self {
        Self { index: AtomicUsize::new(0), cache: AtomicUsize::new(0) }
    }
}

pub struct Chan<T: Slot> {
    buf: Box<[T::Cell]>,
    slots: usize,
    prod: Side,
    cons: Side,
}

impl<T: Slot> Chan<T> {
    pub fn new() -> Self {
        let slots = CAP + 1;
        Self {
            buf: (0..slots)
                .map(|_| T::cell(T::default()))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            slots,
            prod: Side::new(),
            cons: Side::new(),
        }
    }
    #[inline(always)]
    pub fn len(&self) -> usize {
        let w = self.prod.index.load(Ordering::Acquire);
        let r = self.cons.index.load(Ordering::Acquire);
        if w >= r { w - r } else { self.slots + w - r }
    }
    #[inline(always)]
    pub fn room(&self) -> usize {
        CAP - self.len()
    }
    #[inline(always)]
    pub fn try_push(&self, value: T) -> bool {
        let w = self.prod.index.load(Ordering::Relaxed);
        let next = if w + 1 == self.slots { 0 } else { w + 1 };
        if next == self.prod.cache.load(Ordering::Relaxed) {
            let r = self.cons.index.load(Ordering::Acquire);
            self.prod.cache.store(r, Ordering::Relaxed);
            if next == r {
                return false;
            }
        }
        T::put(&self.buf[w], value);
        self.prod.index.store(next, Ordering::Release);
        true
    }
    #[inline(always)]
    pub fn pop(&self) -> T {
        let r = self.cons.index.load(Ordering::Relaxed);
        let value = T::get(&self.buf[r]);
        self.cons
            .index
            .store(if r + 1 == self.slots { 0 } else { r + 1 }, Ordering::Release);
        value
    }
    #[inline(always)]
    pub fn at(&self, index: usize) -> T {
        let mut k = self.cons.index.load(Ordering::Relaxed) + index;
        if k >= self.slots {
            k -= self.slots;
        }
        T::get(&self.buf[k])
    }
}
";

fn port_wrappers(handle: &str, bound: &str) -> String {
    format!("{}\n{}", in_port(handle, bound), out_port(handle, bound))
}

fn in_port(handle: &str, bound: &str) -> String {
    format!(
        r"pub struct InPort<T: {bound}> {{
    chan: {handle}<Chan<T>>,
}}

impl<T: {bound}> InPort<T> {{
    pub fn new(chan: {handle}<Chan<T>>) -> Self {{
        Self {{ chan }}
    }}
    #[inline(always)]
    pub fn len(&mut self) -> usize {{
        self.chan.len()
    }}
    #[inline(always)]
    pub fn avail(&mut self, n: usize) -> bool {{
        self.chan.len() >= n
    }}
    #[inline(always)]
    pub fn peek(&self, i: usize) -> T {{
        self.chan.at(i)
    }}
    #[inline(always)]
    pub fn recv(&mut self) -> T {{
        self.chan.pop()
    }}
    #[inline(always)]
    pub fn pop_front(&mut self) -> Option<T> {{
        if self.chan.len() == 0 {{
            None
        }} else {{
            Some(self.chan.pop())
        }}
    }}
}}
"
    )
}

fn out_port(handle: &str, bound: &str) -> String {
    format!(
        r"pub enum OutPort<T: {bound}> {{
    None,
    One({handle}<Chan<T>>, VecDeque<T>),
}}

impl<T: {bound}> OutPort<T> {{
    pub fn none() -> Self {{
        Self::None
    }}
    pub fn one(target: {handle}<Chan<T>>) -> Self {{
        Self::One(target, VecDeque::new())
    }}
    fn drain(chan: &Chan<T>, queue: &mut VecDeque<T>) {{
        while let Some(value) = queue.front() {{
            if chan.try_push(value.clone()) {{
                queue.pop_front();
            }} else {{
                break;
            }}
        }}
    }}
    pub fn pump(&mut self) {{
        match self {{
            Self::None => {{}}
            Self::One(chan, pending) => {{
                if !pending.is_empty() {{
                    Self::drain(chan, pending)
                }}
            }}
        }}
    }}
    #[inline(always)]
    pub fn room(&mut self) -> usize {{
        self.pump();
        match self {{
            Self::None => usize::MAX,
            Self::One(chan, pending) => {{
                if pending.is_empty() {{ chan.room() }} else {{ 0 }}
            }}
        }}
    }}
    #[inline(always)]
    pub fn has_room(&mut self) -> bool {{
        self.room() > 0
    }}
    #[inline(always)]
    pub fn push_back(&mut self, value: T) {{
        match self {{
            Self::None => {{}}
            Self::One(chan, pending) => {{
                if pending.is_empty() {{
                    if !chan.try_push(value.clone()) {{
                        pending.push_back(value);
                    }}
                }} else {{
                    pending.push_back(value);
                }}
            }}
        }}
    }}
}}
"
    )
}

pub const CROSSBEAM_PORTS: &str = r"pub struct InPort<T> {
    rx: crossbeam_channel::Receiver<T>,
    buf: VecDeque<T>,
}

impl<T: Clone> InPort<T> {
    pub fn new(rx: crossbeam_channel::Receiver<T>) -> Self {
        Self { rx, buf: VecDeque::new() }
    }
    pub fn len(&mut self) -> usize {
        while let Ok(value) = self.rx.try_recv() {
            self.buf.push_back(value);
        }
        self.buf.len()
    }
    pub fn avail(&mut self, n: usize) -> bool {
        while self.buf.len() < n {
            match self.rx.try_recv() {
                Ok(value) => self.buf.push_back(value),
                Err(_) => break,
            }
        }
        self.buf.len() >= n
    }
    pub fn peek(&self, i: usize) -> T {
        self.buf[i].clone()
    }
    pub fn recv(&mut self) -> T {
        self.buf.pop_front().unwrap()
    }
    pub fn pop_front(&mut self) -> Option<T> {
        self.buf.pop_front()
    }
}

pub enum OutPort<T> {
    None,
    One(crossbeam_channel::Sender<T>, VecDeque<T>),
}

impl<T: Clone> OutPort<T> {
    pub fn none() -> Self {
        Self::None
    }
    pub fn one(tx: crossbeam_channel::Sender<T>) -> Self {
        Self::One(tx, VecDeque::new())
    }
    fn drain(tx: &crossbeam_channel::Sender<T>, queue: &mut VecDeque<T>) {
        while let Some(value) = queue.front() {
            match tx.try_send(value.clone()) {
                Ok(()) => {
                    queue.pop_front();
                }
                Err(crossbeam_channel::TrySendError::Full(_)) => break,
                Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                    queue.clear();
                    break;
                }
            }
        }
    }
    pub fn pump(&mut self) {
        match self {
            Self::None => {}
            Self::One(tx, pending) => Self::drain(tx, pending),
        }
    }
    pub fn room(&mut self) -> usize {
        self.pump();
        match self {
            Self::None => usize::MAX,
            Self::One(tx, pending) => {
                if pending.is_empty() { tx.capacity().map_or(usize::MAX, |cap| cap - tx.len()) } else { 0 }
            }
        }
    }
    pub fn has_room(&mut self) -> bool {
        self.room() > 0
    }
    pub fn push_back(&mut self, value: T) {
        match self {
            Self::None => {}
            Self::One(tx, pending) => {
                if pending.is_empty() {
                    match tx.try_send(value) {
                        Ok(()) => {}
                        Err(crossbeam_channel::TrySendError::Full(value)) => {
                            pending.push_back(value)
                        }
                        Err(crossbeam_channel::TrySendError::Disconnected(_)) => {}
                    }
                } else {
                    pending.push_back(value);
                    Self::drain(tx, pending);
                }
            }
        }
    }
}
";
