use std::fmt::Write as _;

use super::channel_types;
use crate::codegen::{Options, Program};

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

pub const LOCAL_HANDLE: &str = "Rc";
pub const SHARED_HANDLE: &str = "std::sync::Arc";

pub const LOCAL_CHAN_IMPORTS: &str =
    "use std::cell::Cell;\nuse std::collections::VecDeque;\nuse std::rc::Rc;\n";

pub const RING_IMPORTS: &str = "use std::collections::VecDeque;\nuse std::sync::Arc;\nuse std::sync::atomic::{AtomicUsize, Ordering};\n";

pub const RING_MAIN_IMPORTS: &str = "use std::collections::VecDeque;\nuse std::sync::Arc;\n";

pub fn ring_ports(program: &Program<'_>, _options: Options) -> String {
    let types = channel_types(program);
    let impls: String = types.iter().map(|ty| slot_impl(ty)).collect();
    format!(
        "{}{impls}\n{SPSC_CHAN}\n{}",
        slot_trait(true),
        port_wrappers("Arc", "Slot")
    )
}

pub fn local_ports(program: &Program<'_>, _options: Options) -> String {
    let types = channel_types(program);
    let impls: String = types.iter().map(|ty| local_slot_impl(ty)).collect();
    format!(
        "{}{impls}\n{LOCAL_RING_CHAN}\n{}",
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
    #[inline(always)]
    pub fn publish_write(&self) {}
    #[inline(always)]
    pub fn publish_read(&self) {}
}
";

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
    head: AtomicUsize,
    cache: AtomicUsize,
}

impl Side {
    fn new() -> Self {
        Self {
            index: AtomicUsize::new(0),
            head: AtomicUsize::new(0),
            cache: AtomicUsize::new(0),
        }
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
        let r = self.cons.head.load(Ordering::Relaxed);
        if w >= r { w - r } else { self.slots + w - r }
    }
    #[inline(always)]
    pub fn room(&self) -> usize {
        let w = self.prod.head.load(Ordering::Relaxed);
        let r = self.cons.index.load(Ordering::Acquire);
        let used = if w >= r { w - r } else { self.slots + w - r };
        CAP - used
    }
    #[inline(always)]
    pub fn try_push(&self, value: T) -> bool {
        let w = self.prod.head.load(Ordering::Relaxed);
        let next = if w + 1 == self.slots { 0 } else { w + 1 };
        if next == self.prod.cache.load(Ordering::Relaxed) {
            let r = self.cons.index.load(Ordering::Acquire);
            self.prod.cache.store(r, Ordering::Relaxed);
            if next == r {
                return false;
            }
        }
        T::put(&self.buf[w], value);
        self.prod.head.store(next, Ordering::Relaxed);
        true
    }
    #[inline(always)]
    pub fn pop(&self) -> T {
        let r = self.cons.head.load(Ordering::Relaxed);
        let value = T::get(&self.buf[r]);
        self.cons
            .head
            .store(if r + 1 == self.slots { 0 } else { r + 1 }, Ordering::Relaxed);
        value
    }
    #[inline(always)]
    pub fn at(&self, index: usize) -> T {
        let mut k = self.cons.head.load(Ordering::Relaxed) + index;
        if k >= self.slots {
            k -= self.slots;
        }
        T::get(&self.buf[k])
    }
    #[inline(always)]
    pub fn publish_write(&self) {
        let head = self.prod.head.load(Ordering::Relaxed);
        if head != self.prod.index.load(Ordering::Relaxed) {
            self.prod.index.store(head, Ordering::Release);
        }
    }
    #[inline(always)]
    pub fn publish_read(&self) {
        let head = self.cons.head.load(Ordering::Relaxed);
        if head != self.cons.index.load(Ordering::Relaxed) {
            self.cons.index.store(head, Ordering::Release);
        }
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
    #[inline(always)]
    pub fn commit(&mut self) {{
        self.chan.publish_read();
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
    pub fn commit(&mut self) {{
        self.pump();
        match self {{
            Self::None => {{}}
            Self::One(chan, _) => chan.publish_write(),
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
