pub const CROSSBEAM_PORTS: &str = r"pub struct InPort<T> {
    rx: crossbeam_channel::Receiver<T>,
    buf: VecDeque<T>,
}

impl<T: Clone> InPort<T> {
    pub fn new(rx: crossbeam_channel::Receiver<T>) -> Self {
        Self { rx, buf: VecDeque::new() }
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
    Many(Vec<crossbeam_channel::Sender<T>>, Vec<VecDeque<T>>),
}

impl<T: Clone> OutPort<T> {
    pub fn none() -> Self {
        Self::None
    }
    pub fn one(tx: crossbeam_channel::Sender<T>) -> Self {
        Self::One(tx, VecDeque::new())
    }
    pub fn many(txs: Vec<crossbeam_channel::Sender<T>>) -> Self {
        let pending = txs.iter().map(|_| VecDeque::new()).collect();
        Self::Many(txs, pending)
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
            Self::Many(txs, pending) => {
                for (tx, queue) in txs.iter().zip(pending.iter_mut()) {
                    Self::drain(tx, queue);
                }
            }
        }
    }
    pub fn has_room(&mut self) -> bool {
        self.pump();
        match self {
            Self::None => true,
            Self::One(tx, pending) => pending.is_empty() && !tx.is_full(),
            Self::Many(txs, pending) => {
                pending.iter().all(|q| q.is_empty()) && txs.iter().all(|tx| !tx.is_full())
            }
        }
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
            Self::Many(txs, pending) => {
                for queue in pending.iter_mut() {
                    queue.push_back(value.clone());
                }
                for (tx, queue) in txs.iter().zip(pending.iter_mut()) {
                    Self::drain(tx, queue);
                }
            }
        }
    }
}
";
