//! An actor: one thread that owns a stateful object, with commands in and events out. The object
//! is built on the actor's own thread, so it may be `!Sync` or even `!Send` (the media player).

use super::RuntimeError;
use super::mailbox::Outbox;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;

/// Whether the actor goes on after a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Flow {
    /// Wait for the next command or wake.
    Continue,
    /// End the thread.
    Quit,
}

/// What an actor owns and how it reacts. Called only on the actor's thread.
pub trait ActorBody: 'static {
    /// What the UI sends.
    type Command: Send + 'static;
    /// What the actor reports back.
    type Event: Send + 'static;

    /// Applies one command, posting any events it causes.
    fn command(&mut self, command: Self::Command, events: &Outbox<Self::Event>) -> Flow;

    /// The object asked for attention through its `ActorWake` (the player's notify callback):
    /// poll it and post what changed.
    fn woken(&mut self, events: &Outbox<Self::Event>);
}

enum Message<C> {
    Command(C),
    Wake,
    Quit,
}

/// Wakes an actor from any thread, including a callback running on a library's own thread.
/// Wakes coalesce: one is queued until the actor handles it.
pub struct ActorWake<C> {
    sender: Sender<Message<C>>,
    pending: Arc<AtomicBool>,
}

impl<C> Clone for ActorWake<C> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            pending: Arc::clone(&self.pending),
        }
    }
}

impl<C> std::fmt::Debug for ActorWake<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActorWake").finish_non_exhaustive()
    }
}

impl<C> ActorWake<C> {
    /// Asks the actor to call `woken`. Does nothing if the actor has ended.
    pub fn wake(&self) {
        if !self.pending.swap(true, Ordering::SeqCst) {
            let _ = self.sender.send(Message::Wake);
        }
    }
}

/// The UI's end of an actor. Dropping it ends the thread and joins it.
pub struct Actor<C> {
    wake: ActorWake<C>,
    thread: Option<JoinHandle<()>>,
}

impl<C> std::fmt::Debug for Actor<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Actor").finish_non_exhaustive()
    }
}

impl<C: Send + 'static> Actor<C> {
    /// Starts the thread `name`. `make` runs on it and builds the body, given the wake to hand to
    /// whatever must call back; `events` is where the body posts.
    pub fn spawn<B>(
        name: &str,
        events: Outbox<B::Event>,
        make: impl FnOnce(ActorWake<C>) -> B + Send + 'static,
    ) -> Result<Actor<C>, RuntimeError>
    where
        B: ActorBody<Command = C>,
    {
        let (sender, receiver) = channel();
        let wake = ActorWake {
            sender,
            pending: Arc::new(AtomicBool::new(false)),
        };
        let for_body = wake.clone();
        let thread = std::thread::Builder::new()
            .name(name.to_owned())
            .spawn(move || {
                let body = make(for_body.clone());
                run(body, &receiver, &for_body.pending, &events);
            })
            .map_err(|source| RuntimeError::Spawn {
                name: name.to_owned(),
                source,
            })?;
        Ok(Actor {
            wake,
            thread: Some(thread),
        })
    }

    /// Sends a command; fails if the actor's thread has ended.
    pub fn send(&self, command: C) -> Result<(), RuntimeError> {
        self.wake
            .sender
            .send(Message::Command(command))
            .map_err(|_| RuntimeError::ActorEnded)
    }

    /// A wake for the actor, to give to a callback.
    pub fn waker(&self) -> ActorWake<C> {
        self.wake.clone()
    }
}

impl<C> Drop for Actor<C> {
    fn drop(&mut self) {
        let _ = self.wake.sender.send(Message::Quit);
        if let Some(thread) = self.thread.take() {
            // A panicking body already ended the thread and `send` reported it.
            let _ = thread.join();
        }
    }
}

fn run<B: ActorBody>(
    mut body: B,
    receiver: &Receiver<Message<B::Command>>,
    pending: &AtomicBool,
    events: &Outbox<B::Event>,
) {
    while let Ok(message) = receiver.recv() {
        match message {
            Message::Command(command) => match body.command(command, events) {
                Flow::Continue => {}
                Flow::Quit => return,
            },
            Message::Wake => {
                pending.store(false, Ordering::SeqCst);
                body.woken(events);
            }
            Message::Quit => return,
        }
    }
}
