use std::cmp::Reverse;
use std::collections::BinaryHeap;
use crate::model::ResourceName;

#[derive(Debug, Clone)]
pub struct FileMetaData  {
    pub name: ResourceName,
    pub size: u64,
    pub mtime: u64
}


#[derive(Debug, Clone)]
pub struct TopKFiles {
    k: usize,
    heap: BinaryHeap<(Reverse<u64>, String)>,
}

impl TopKFiles {
    pub fn new(k: usize) -> Self {
        Self { k, heap: BinaryHeap::new() }
    }

    pub fn offer(& mut self, name: impl Into<ResourceName>, size: u64){
        let name = name.into();

        if self.k == 0 {
            return;
        }

        if self.heap.len() < self.k {
            self.heap.push((Reverse(size), name));
            return;
        }

        let (Reverse(min_size), _) = self.heap.peek().unwrap();

        if size < *min_size {
            return;
        }

        self.heap.pop();
        self.heap.push((Reverse(size), name));
    }

    pub fn to_sorted_vec_desc(&self) -> Vec<FileMetaData> {
        let mut v: Vec<FileMetaData> = self
            .heap
            .iter()
            .map(|(Reverse(size), name)| FileMetaData { name: name.clone(), size: *size, mtime: 0 })
            .collect();
        v.sort_by_key(|f| Reverse(f.size));
        v
    }
}

