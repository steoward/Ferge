use std::sync::Arc;
use crossbeam::channel::{unbounded, Sender};
use parking_lot::Mutex;

pub struct ThreadPool {
    _workers: Vec<std::thread::JoinHandle<()>>,
    sender: Sender<Box<dyn FnOnce() + Send + 'static>>,
}

impl ThreadPool {
    pub fn new(num_threads: usize) -> Self {
        let (sender, receiver): (Sender<Box<dyn FnOnce() + Send + 'static>>, _) = unbounded();
        let receiver = Arc::new(Mutex::new(receiver));
        let mut workers = Vec::with_capacity(num_threads);

        for _ in 0..num_threads {
            let receiver = Arc::clone(&receiver);
            let handle = std::thread::spawn(move || loop {
                let task = {
                    let rx = receiver.lock();
                    rx.recv()
                };
                match task {
                    Ok(job) => job(),
                    Err(_) => break,
                }
            });
            workers.push(handle);
        }

        Self {
            _workers: workers,
            sender,
        }
    }

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.sender.send(Box::new(f)).unwrap();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        // Workers will exit when sender is dropped
    }
}
