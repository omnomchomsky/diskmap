use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::session::Session;

pub type ScanId = String;

#[derive(Clone)]
pub struct ScanStore {
    sessions: Arc<Mutex<HashMap<ScanId, Session>>>,
}

impl ScanStore {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn insert(&self, scan_id: ScanId, session: Session) {
        let mut sessions = self.sessions.lock().unwrap();
        sessions.insert(scan_id, session);
    }

    pub fn get(&self, scan_id: &ScanId) -> Option<Session> {
        let sessions = self.sessions.lock().unwrap();
        sessions.get(scan_id).cloned()
    }

    pub fn remove(&self, scan_id: &ScanId) -> Option<Session> {
        let mut sessions = self.sessions.lock().unwrap();
        sessions.remove(scan_id)
    }
}

impl Default for ScanStore {
    fn default() -> Self {
        Self::new()
    }
}
