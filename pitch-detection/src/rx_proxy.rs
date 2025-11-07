// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! `RxProxy` is a simple class to allow the reuse of a
//! `mpsc::Receiver`.  The receiver to reuse (rx_one` is passed in the
//! constructor `new`.  To reuse `rx_one` create a channel pair using
//! `(tx, rx) = mpsc::Channel<T>()` and pass `tx` to `set_sender`.
//! Any messages received on `rx_one` will be sent to `tx` ad can be
//! received on the new `rx`.  `set_sender` can be called any number
//! of times, and it overwrites the previous setting each time.
use std::error::Error;
use std::fmt::Formatter;
use std::fmt::{self, Debug};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Debug)]
pub enum RxProxyError {
    SetTxOnRunning,
}
impl fmt::Display for RxProxyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            RxProxyError::SetTxOnRunning => write!(f, "{self:?}"),
        }
    }
}
impl Error for RxProxyError {}
pub struct RxProxy<T>
where
    T: Send + 'static,
{
    rx: Arc<Mutex<Receiver<T>>>,
    running: Arc<AtomicBool>,
}

impl<T> RxProxy<T>
where
    T: Send + 'static,
{
    /// Stop the proxy
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Check the proxy
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Start the proxy
    fn run(
        rx: Arc<Mutex<Receiver<T>>>,
        tx: Sender<T>,
        running: Arc<AtomicBool>,
    ) -> Result<JoinHandle<()>, Box<dyn std::error::Error>> {
        if running.swap(true, Ordering::SeqCst) {
            return Err("Already running".into());
        }
        let handle = thread::spawn(move || {
            let rx = rx.lock().unwrap();
            while running.load(Ordering::SeqCst) {
                // Clone the sender.  Multiple senders are allowed

                if !running.load(Ordering::SeqCst) {
                    break;
                }
                let message = {
                    match rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(m) => Some(m),
                        Err(RecvTimeoutError::Timeout) => None,
                        Err(RecvTimeoutError::Disconnected) => {
                            // Channel disconnected, stop the proxy
                            running.store(false, Ordering::SeqCst);
                            break;
                        }
                    }
                };
                // If there is a sender send the message,
                // otherwise drop it
                if let Some(msg) = message {
                    if tx.send(msg).is_err() {
                        // Stop the proxy once `tx` stops working
                        running.store(false, Ordering::SeqCst);
                    }
                }
            }
        });
        Ok(handle)
    }

    /// `rx` is the receiver to proxy
    pub fn new(rx: Receiver<T>) -> Self {
        let rx = Arc::new(Mutex::new(rx));
        let running = Arc::new(AtomicBool::new(false));
        Self { rx, running }
    }

    /// Provide the sending end of a channel to proxy messages on
    /// `rx`.  The proxy must be stopped when this is called.  As this
    /// starts the proxy
    pub fn set_sender(&mut self, tx: Sender<T>) -> Result<JoinHandle<()>, RxProxyError> {
        if self.is_running() {
            return Err(RxProxyError::SetTxOnRunning);
        }
        let rx = self.rx.clone();

        let h = match Self::run(rx.clone(), tx, self.running.clone()) {
            Ok(h) => h,
            Err(err) => panic!("RxProxy::new: Starting thread failed; {err}"),
        };
        Ok(h)
    }
}

// Add the same trait bounds to the Drop implementation
impl<T> Drop for RxProxy<T>
where
    T: Send + 'static,
{
    fn drop(&mut self) {
        self.stop();
    }
}
