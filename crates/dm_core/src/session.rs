use std::collections::VecDeque;
use std::path::{PathBuf};
use std::time::{Duration, Instant};

use crate::fs_adapter::FsAdapter;
use crate::scanner::{scan_one_dir, ScanEvent, ScanJob};
use crate::tree::Tree;
use crate::model::{NodeId, ResourceName};

#[derive(Debug, Clone)]
pub struct Session {
    pub tree: Tree,
    queue: VecDeque<ScanJob>,
    top_k: usize,

    pub errors: u64,
    pub jobs_started: u64,
    pub jobs_done: u64,
}

impl Session {
    pub fn new(root_path: PathBuf, top_k: usize) -> Self {
        // For now: root node name is the path string (lossy is fine for display)
        let root_name: ResourceName = root_path.to_string_lossy().to_string();

        let tree = Tree::new(root_name, top_k);
        let mut queue = VecDeque::new();
        queue.push_back(ScanJob {
            path: root_path,
            node_id: 0,
            depth: 0,
        });

        Self {
            tree,
            queue,
            top_k,
            errors: 0,
            jobs_started: 0,
            jobs_done: 0,
        }
    }

    pub fn run(&mut self, fs: &dyn FsAdapter) {
        while let Some(job) = self.queue.pop_front() {
            self.jobs_started += 1;

            scan_one_dir(fs, job, &mut |event| {
                self.apply_event(event);
            });
        }
    }

    pub fn run_parallel<F>(&mut self, fs: &F, num_threads: usize)
    where
        F: FsAdapter + Clone + 'static,
    {
        use crossbeam_channel::{unbounded};
        use std::thread;

        // Create channels
        let (job_tx, job_rx) = unbounded::<ScanJob>();
        let (event_tx, event_rx) = unbounded::<ScanEvent>();

        // Seed the job queue
        while let Some(job) = self.queue.pop_front() {
            job_tx.send(job).unwrap();
            self.jobs_started += 1;
        }

        // Spawn worker threads
        let mut handles = vec![];

        for _ in 0..num_threads {
            let job_rx = job_rx.clone();
            let event_tx = event_tx.clone();
            let fs = fs.clone();

            let handle = thread::spawn(move || {
                while let Ok(job) = job_rx.recv() {
                    let event_tx = event_tx.clone();
                    scan_one_dir(&fs, job, &mut |event| {
                        let _ = event_tx.send(event);
                    });
                }
            });

            handles.push(handle);
        }

        // Drop worker-side channel ends
        drop(job_rx);

        // Process events from workers
        let mut active_jobs = self.jobs_started;

        while active_jobs > 0 {
            if let Ok(event) = event_rx.recv() {
                match &event {
                    ScanEvent::Dir { parent_id, name, path } => {
                        let child_name = name.to_string_lossy().to_string();
                        let child_id = self.tree.add_child(*parent_id, child_name, self.top_k);

                        let new_job = ScanJob {
                            path: path.clone(),
                            node_id: child_id,
                            depth: 0,
                        };

                        // Send new job to workers
                        job_tx.send(new_job).ok();
                        self.jobs_started += 1;
                        active_jobs += 1;

                        self.tree.node_mut(*parent_id).mark_partial();
                    }
                    ScanEvent::Done { .. } => {
                        self.jobs_done += 1;
                        active_jobs -= 1;
                    }
                    _ => {}
                }

                // Apply the event (but skip Dir since we handled it above)
                if !matches!(event, ScanEvent::Dir { .. }) {
                    self.apply_event(event);
                }
            } else {
                break;
            }
        }

        // Signal workers to stop
        drop(job_tx);

        // Wait for workers to finish
        for handle in handles {
            handle.join().unwrap();
        }
    }

    pub fn run_parallel_with_callback<F, C>(&mut self, fs: &F, num_threads: usize, mut callback: C)
    where
        F: FsAdapter + Clone + 'static,
        C: FnMut(&Session),
    {
        use crossbeam_channel::{unbounded};
        use std::thread;

        // Create channels
        let (job_tx, job_rx) = unbounded::<ScanJob>();
        let (event_tx, event_rx) = unbounded::<ScanEvent>();

        // Seed the job queue
        while let Some(job) = self.queue.pop_front() {
            job_tx.send(job).unwrap();
            self.jobs_started += 1;
        }

        // Spawn worker threads
        let mut handles = vec![];

        for _ in 0..num_threads {
            let job_rx = job_rx.clone();
            let event_tx = event_tx.clone();
            let fs = fs.clone();

            let handle = thread::spawn(move || {
                while let Ok(job) = job_rx.recv() {
                    let event_tx = event_tx.clone();
                    scan_one_dir(&fs, job, &mut |event| {
                        let _ = event_tx.send(event);
                    });
                }
            });

            handles.push(handle);
        }

        // Drop worker-side channel ends
        drop(job_rx);

        // Process events from workers with periodic callbacks
        let mut active_jobs = self.jobs_started;
        let mut last_callback = Instant::now();
        let callback_interval = Duration::from_millis(200);

        while active_jobs > 0 {
            // Use recv_timeout to allow periodic callbacks
            match event_rx.recv_timeout(Duration::from_millis(50)) {
                Ok(event) => {
                    match &event {
                        ScanEvent::Dir { parent_id, name, path } => {
                            let child_name = name.to_string_lossy().to_string();
                            let child_id = self.tree.add_child(*parent_id, child_name, self.top_k);

                            let new_job = ScanJob {
                                path: path.clone(),
                                node_id: child_id,
                                depth: 0,
                            };

                            // Send new job to workers
                            job_tx.send(new_job).ok();
                            self.jobs_started += 1;
                            active_jobs += 1;

                            self.tree.node_mut(*parent_id).mark_partial();
                        }
                        ScanEvent::Done { .. } => {
                            self.jobs_done += 1;
                            active_jobs -= 1;
                        }
                        _ => {}
                    }

                    // Apply the event (but skip Dir since we handled it above)
                    if !matches!(event, ScanEvent::Dir { .. }) {
                        self.apply_event(event);
                    }

                    // Check if we should trigger callback
                    if last_callback.elapsed() >= callback_interval {
                        callback(self);
                        last_callback = Instant::now();
                    }
                }
                Err(_) => {
                    // Timeout - check if we should trigger callback
                    if last_callback.elapsed() >= callback_interval {
                        callback(self);
                        last_callback = Instant::now();
                    }
                }
            }
        }

        // Final callback
        callback(self);

        // Signal workers to stop
        drop(job_tx);

        // Wait for workers to finish
        for handle in handles {
            handle.join().unwrap();
        }
    }

    fn apply_event(&mut self, event: ScanEvent) {
        match event {
            ScanEvent::File { parent_id, name, size } => {
                // Update node
                let node = self.tree.node_mut(parent_id);
                node.add_file(name.to_string_lossy().to_string(), size);

                // Propagate known bytes up the tree (to ancestors only, not including this node)
                if let Some(grandparent) = self.tree.node(parent_id).parent_id() {
                    self.propagate_known_delta(grandparent, size);
                }
            }

            ScanEvent::Dir { parent_id, name, path } => {
                // Create child node in tree
                let child_name = name.to_string_lossy().to_string();
                let child_id = self.tree.add_child(parent_id, child_name, self.top_k);

                // Enqueue scan job
                self.queue.push_back(ScanJob {
                    path,
                    node_id: child_id,
                    depth: 0, // or parent depth + 1 if you store it
                });

                // Mark parent partial (we learned something new)
                self.tree.node_mut(parent_id).mark_partial();
            }

            ScanEvent::Done { node } => {
                self.jobs_done += 1;
                self.tree.node_mut(node).mark_complete();
            }

            ScanEvent::Error { node, .. } => {
                self.errors += 1;
                self.tree.node_mut(node).mark_partial();
            }
        }
    }

    fn propagate_known_delta(&mut self, from: NodeId, delta: u64) {
        // add to this node and all ancestors
        let mut cur = Some(from);
        while let Some(id) = cur {
            let parent = self.tree.node(id).parent_id(); // copy parent before mut borrow
            self.tree.node_mut(id).add_subtree_known(delta);
            cur = parent;
        }
    }
}
