use std::collections::{BTreeMap, VecDeque};
use std::future::Future;
use std::marker::PhantomData;
use std::mem;
use std::panic::resume_unwind;
use std::pin::Pin;
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, Waker};
use std::thread::{self, JoinHandle};

pub type OpId = u64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event<S, A, SR, AR> {
    CreateSync { id: OpId, op: S },
    ReturnSync { id: OpId, result: SR },
    CreateAsync { id: OpId, op: A },
    ResolveAsync { id: OpId, result: AR },
    CancelAsync { id: OpId },
    AbortAsync { id: OpId },
}

pub trait SimBundle: Sized + Send + 'static {
    type SyncOp: Clone + Send + 'static;
    type AsyncOp: Clone + Send + 'static;
    type SyncResult: Clone + Send + 'static;
    type AsyncResult: Clone + Send + 'static;
    type RunFuture: Future + Send + 'static;

    fn build(
        self,
        driver: SimDriver<Self::SyncOp, Self::AsyncOp, Self::SyncResult, Self::AsyncResult>,
    ) -> Self::RunFuture;

    fn sync_result_matches(op: &Self::SyncOp, result: &Self::SyncResult) -> bool;
    fn async_result_matches(op: &Self::AsyncOp, result: &Self::AsyncResult) -> bool;
}

pub struct NewRunWrapper<B: SimBundle> {
    bundle: B,
}

pub struct RunWrapper<B: SimBundle> {
    shared: SharedHandle<B::SyncOp, B::AsyncOp, B::SyncResult, B::AsyncResult>,
    thread: Option<JoinHandle<()>>,
    _bundle: PhantomData<B>,
}

pub struct SimDriver<S, A, SR, AR> {
    shared: SharedHandle<S, A, SR, AR>,
}

pub enum AsyncCompletion<R> {
    Resolved(R),
    Aborted,
}

pub struct InboundAsync<A> {
    pub id: OpId,
    pub op: A,
}

#[derive(Debug)]
struct ShutdownPanic;

fn shutdown_panic() -> ! {
    resume_unwind(Box::new(ShutdownPanic))
}

impl<B: SimBundle> NewRunWrapper<B> {
    pub fn new(bundle: B) -> Self {
        Self { bundle }
    }

    pub fn start(
        self,
    ) -> (
        RunWrapper<B>,
        Vec<Event<B::SyncOp, B::AsyncOp, B::SyncResult, B::AsyncResult>>,
    ) {
        let shared = SharedHandle::new();
        let run_shared = shared.clone();
        let thread = thread::spawn(move || {
            let exec_shared = run_shared.clone();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                let driver = SimDriver {
                    shared: exec_shared.clone(),
                };
                let future = self.bundle.build(driver);
                executor_loop::<B, _>(exec_shared, future)
            }));
            if result.is_err() {
                let mut state = run_shared.lock();
                state.terminated = true;
                state.idle = true;
                run_shared.notify_all();
            }
        });

        shared.wait_for_idle();
        let events = shared.take_outbound();

        (
            RunWrapper {
                shared,
                thread: Some(thread),
                _bundle: PhantomData,
            },
            events,
        )
    }
}

impl<B: SimBundle> RunWrapper<B> {
    pub fn push(
        &mut self,
        event: Event<B::SyncOp, B::AsyncOp, B::SyncResult, B::AsyncResult>,
    ) -> Vec<Event<B::SyncOp, B::AsyncOp, B::SyncResult, B::AsyncResult>> {
        self.shared.apply_inbound::<B>(event);
        self.shared.wait_for_idle();
        self.shared.take_outbound()
    }

    pub fn is_terminated(&self) -> bool {
        self.shared.lock().terminated
    }
}

impl<B: SimBundle> Drop for RunWrapper<B> {
    fn drop(&mut self) {
        {
            let mut state = self.shared.lock();
            state.shutdown = true;
            state.idle = true;
            self.shared.notify_all();
        }

        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl<S, A, SR, AR> Clone for SimDriver<S, A, SR, AR> {
    fn clone(&self) -> Self {
        Self {
            shared: self.shared.clone(),
        }
    }
}

impl<S, A, SR, AR> SimDriver<S, A, SR, AR>
where
    S: Clone + Send + 'static,
    A: Clone + Send + 'static,
    SR: Send + 'static,
    AR: Send + 'static,
{
    pub fn create_sync(&self, op: S) -> SR {
        let mut state = self.shared.lock();
        if state.shutdown {
            shutdown_panic();
        }
        assert!(state.pending_sync.is_none(), "multiple sync ops are not allowed");

        let id = state.next_op_id;
        state.next_op_id += 1;
        state.outbound.push(Event::CreateSync {
            id,
            op: op.clone(),
        });
        state.pending_sync = Some(PendingSync {
            id,
            op,
            result: None,
        });
        state.idle = true;
        self.shared.notify_all();

        loop {
            if state.shutdown {
                shutdown_panic();
            }

            if let Some(pending) = state.pending_sync.as_mut() {
                if pending.id == id {
                    if let Some(result) = pending.result.take() {
                        state.pending_sync = None;
                        state.idle = false;
                        self.shared.notify_all();
                        return result;
                    }
                } else {
                    panic!("pending sync id changed unexpectedly");
                }
            } else {
                panic!("pending sync op disappeared unexpectedly");
            }

            state = self.shared.wait(state);
        }
    }

    pub fn create_async(&self, op: A) -> AsyncHandle<S, A, SR, AR> {
        let mut state = self.shared.lock();
        if state.shutdown {
            shutdown_panic();
        }

        let id = state.next_op_id;
        state.next_op_id += 1;
        state.outbound.push(Event::CreateAsync {
            id,
            op: op.clone(),
        });
        state.outbound_async.insert(id, OutboundAsyncState::Pending { op });
        self.shared.bump_and_notify(&mut state);

        AsyncHandle {
            shared: self.shared.clone(),
            id,
            completed: false,
        }
    }

    pub fn spawn<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let mut state = self.shared.lock();
        state.spawned_tasks.push(Box::pin(future));
        self.shared.bump_and_notify(&mut state);
    }

    pub fn next_inbound_async(&self) -> NextInboundAsync<S, A, SR, AR> {
        NextInboundAsync {
            shared: self.shared.clone(),
        }
    }

    pub fn resolve_inbound_async(&self, id: OpId, result: AR) {
        let mut state = self.shared.lock();
        match state.inbound_async.get(&id) {
            Some(InboundAsyncState::Active) => {
                state.inbound_async.remove(&id);
                state.outbound.push(Event::ResolveAsync { id, result });
                self.shared.bump_and_notify(&mut state);
            }
            Some(InboundAsyncState::Canceled) => {
                state.inbound_async.remove(&id);
                self.shared.bump_and_notify(&mut state);
            }
            _ => panic!("cannot resolve unknown inbound async id {id}"),
        }
    }

    pub fn abort_inbound_async(&self, id: OpId) {
        let mut state = self.shared.lock();
        match state.inbound_async.get(&id) {
            Some(InboundAsyncState::Queued(_)) | Some(InboundAsyncState::Active) => {
                state.inbound_async.remove(&id);
                state.inbound_queue.retain(|queued| *queued != id);
                state.outbound.push(Event::AbortAsync { id });
                self.shared.bump_and_notify(&mut state);
            }
            Some(InboundAsyncState::Canceled) => {
                state.inbound_async.remove(&id);
                self.shared.bump_and_notify(&mut state);
            }
            None => panic!("cannot abort unknown inbound async id {id}"),
        }
    }
}

pub struct AsyncHandle<S, A, SR, AR> {
    shared: SharedHandle<S, A, SR, AR>,
    id: OpId,
    completed: bool,
}

impl<S, A, SR, AR> Future for AsyncHandle<S, A, SR, AR>
where
    S: Clone + Send + 'static,
    A: Clone + Send + 'static,
    SR: Send + 'static,
    AR: Send + 'static,
{
    type Output = AsyncCompletion<AR>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let mut state = this.shared.lock();
        match state.outbound_async.remove(&this.id) {
            Some(OutboundAsyncState::Pending { op }) => {
                state.outbound_async.insert(this.id, OutboundAsyncState::Pending { op });
                Poll::Pending
            }
            Some(OutboundAsyncState::Resolved(result)) => {
                this.completed = true;
                Poll::Ready(AsyncCompletion::Resolved(result))
            }
            Some(OutboundAsyncState::Aborted) => {
                this.completed = true;
                Poll::Ready(AsyncCompletion::Aborted)
            }
            None => panic!("unknown async op id {}", this.id),
        }
    }
}

impl<S, A, SR, AR> Drop for AsyncHandle<S, A, SR, AR> {
    fn drop(&mut self) {
        if self.completed {
            return;
        }

        let mut state = self.shared.lock();
        if let Some(OutboundAsyncState::Pending { .. }) = state.outbound_async.remove(&self.id) {
            state.outbound.push(Event::CancelAsync { id: self.id });
            self.shared.bump_and_notify(&mut state);
        }
    }
}

pub struct NextInboundAsync<S, A, SR, AR> {
    shared: SharedHandle<S, A, SR, AR>,
}

impl<S, A, SR, AR> Future for NextInboundAsync<S, A, SR, AR>
where
    S: Clone + Send + 'static,
    A: Clone + Send + 'static,
    SR: Send + 'static,
    AR: Send + 'static,
{
    type Output = InboundAsync<A>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.shared.lock();

        while let Some(id) = state.inbound_queue.pop_front() {
            match state.inbound_async.remove(&id) {
                Some(InboundAsyncState::Queued(op)) => {
                    state.inbound_async.insert(id, InboundAsyncState::Active);
                    return Poll::Ready(InboundAsync { id, op });
                }
                Some(InboundAsyncState::Canceled) => {
                    state.inbound_async.remove(&id);
                }
                Some(InboundAsyncState::Active) | None => {}
            }
        }

        Poll::Pending
    }
}

type BoxTask = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

struct SharedHandle<S, A, SR, AR> {
    inner: Arc<(Mutex<State<S, A, SR, AR>>, Condvar)>,
}

struct State<S, A, SR, AR> {
    next_op_id: OpId,
    outbound: Vec<Event<S, A, SR, AR>>,
    pending_sync: Option<PendingSync<S, SR>>,
    outbound_async: BTreeMap<OpId, OutboundAsyncState<A, AR>>,
    inbound_async: BTreeMap<OpId, InboundAsyncState<A>>,
    inbound_queue: VecDeque<OpId>,
    spawned_tasks: Vec<BoxTask>,
    idle: bool,
    terminated: bool,
    shutdown: bool,
    version: u64,
}

struct PendingSync<S, SR> {
    id: OpId,
    op: S,
    result: Option<SR>,
}

enum OutboundAsyncState<A, AR> {
    Pending { op: A },
    Resolved(AR),
    Aborted,
}

enum InboundAsyncState<A> {
    Queued(A),
    Active,
    Canceled,
}

impl<S, A, SR, AR> Clone for SharedHandle<S, A, SR, AR> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<S, A, SR, AR> SharedHandle<S, A, SR, AR> {
    fn new() -> Self {
        Self {
            inner: Arc::new((
                Mutex::new(State {
                    next_op_id: 0,
                    outbound: Vec::new(),
                    pending_sync: None,
                    outbound_async: BTreeMap::new(),
                    inbound_async: BTreeMap::new(),
                    inbound_queue: VecDeque::new(),
                    spawned_tasks: Vec::new(),
                    idle: false,
                    terminated: false,
                    shutdown: false,
                    version: 0,
                }),
                Condvar::new(),
            )),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State<S, A, SR, AR>> {
        self.inner.0.lock().unwrap_or_else(|err| err.into_inner())
    }

    fn wait<'a>(
        &self,
        guard: std::sync::MutexGuard<'a, State<S, A, SR, AR>>,
    ) -> std::sync::MutexGuard<'a, State<S, A, SR, AR>> {
        self.inner.1.wait(guard).unwrap_or_else(|err| err.into_inner())
    }

    fn notify_all(&self) {
        self.inner.1.notify_all();
    }

    fn bump_and_notify(&self, state: &mut State<S, A, SR, AR>) {
        state.version += 1;
        state.idle = false;
        self.notify_all();
    }

    fn take_outbound(&self) -> Vec<Event<S, A, SR, AR>> {
        let mut state = self.lock();
        mem::take(&mut state.outbound)
    }

    fn wait_for_idle(&self) {
        let mut state = self.lock();
        while !state.idle && !state.terminated {
            state = self.wait(state);
        }
    }

    fn apply_inbound<B: SimBundle<SyncOp = S, AsyncOp = A, SyncResult = SR, AsyncResult = AR>>(
        &self,
        event: Event<S, A, SR, AR>,
    ) {
        let mut state = self.lock();
        assert!(!state.terminated, "cannot push after termination");

        if let Some(pending) = state.pending_sync.as_ref() {
            match event {
                Event::ReturnSync { id, result } => {
                    assert_eq!(id, pending.id, "sync response id mismatch");
                    let op = &pending.op;
                    assert!(B::sync_result_matches(op, &result), "sync result does not match op");
                    state.pending_sync.as_mut().unwrap().result = Some(result);
                    self.bump_and_notify(&mut state);
                    return;
                }
                _ => panic!("while blocked on sync, the next event must be its ReturnSync"),
            }
        }

        match event {
            Event::ReturnSync { id, .. } => panic!("unknown sync id {id}"),
            Event::ResolveAsync { id, result } => {
                let op = match state.outbound_async.get(&id) {
                    Some(OutboundAsyncState::Pending { op }) => op,
                    Some(_) => panic!("async id {id} is already completed"),
                    None => panic!("unknown async id {id}"),
                };
                assert!(B::async_result_matches(op, &result), "async result does not match op");
                state.outbound_async.insert(id, OutboundAsyncState::Resolved(result));
                self.bump_and_notify(&mut state);
            }
            Event::AbortAsync { id } => match state.outbound_async.get(&id) {
                Some(OutboundAsyncState::Pending { .. }) => {
                    state.outbound_async.insert(id, OutboundAsyncState::Aborted);
                    self.bump_and_notify(&mut state);
                }
                Some(_) => panic!("async id {id} is already completed"),
                None => panic!("unknown async id {id}"),
            },
            Event::CreateAsync { id, op } => {
                assert!(
                    !state.inbound_async.contains_key(&id) && !state.outbound_async.contains_key(&id),
                    "duplicate async id {id}"
                );
                state.inbound_async.insert(id, InboundAsyncState::Queued(op));
                state.inbound_queue.push_back(id);
                self.bump_and_notify(&mut state);
            }
            Event::CancelAsync { id } => match state.inbound_async.get_mut(&id) {
                Some(InboundAsyncState::Queued(_)) => {
                    state.inbound_async.insert(id, InboundAsyncState::Canceled);
                    state.inbound_queue.retain(|queued| *queued != id);
                    self.bump_and_notify(&mut state);
                }
                Some(InboundAsyncState::Active) => {
                    state.inbound_async.insert(id, InboundAsyncState::Canceled);
                    self.bump_and_notify(&mut state);
                }
                Some(InboundAsyncState::Canceled) => panic!("async id {id} is already canceled"),
                None => panic!("unknown inbound async id {id}"),
            },
            Event::CreateSync { id: _, .. } => panic!("inbound CreateSync is not supported"),
        }
    }
}

fn executor_loop<B, F>(shared: SharedHandle<B::SyncOp, B::AsyncOp, B::SyncResult, B::AsyncResult>, future: F)
where
    B: SimBundle,
    F: Future,
{
    let mut main = Box::pin(future);
    let mut tasks: Vec<BoxTask> = Vec::new();
    let waker = noop_waker();

    loop {
        {
            let mut state = shared.lock();
            if state.shutdown {
                return;
            }
            tasks.append(&mut state.spawned_tasks);
            state.idle = false;
        }

        let mut progress = false;
        let mut cx = Context::from_waker(&waker);

        match main.as_mut().poll(&mut cx) {
            Poll::Ready(_) => {
                let mut state = shared.lock();
                state.terminated = true;
                state.idle = true;
                shared.notify_all();
                return;
            }
            Poll::Pending => {}
        }

        let mut idx = 0;
        while idx < tasks.len() {
            match tasks[idx].as_mut().poll(&mut cx) {
                Poll::Ready(()) => {
                    drop(tasks.remove(idx));
                    progress = true;
                }
                Poll::Pending => idx += 1,
            }
        }

        let mut state = shared.lock();
        if state.shutdown {
            return;
        }

        tasks.append(&mut state.spawned_tasks);

        if !progress {
            let current_version = state.version;
            state.idle = true;
            shared.notify_all();
            while !state.shutdown && !state.terminated && state.version == current_version {
                state = shared.wait(state);
            }
            if state.shutdown {
                return;
            }
        }
    }
}

fn noop_waker() -> Waker {
    Waker::noop().clone()
}

#[cfg(test)]
mod tests;
