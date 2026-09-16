#[derive(Debug, Clone)]
pub struct GCounter {
    pub slots: Vec<u64>, // index = replica id; I may only bump my own
}

impl GCounter {
    pub fn new(num_replicas: usize) -> Self {
        Self {
            slots: vec![0; num_replicas],
        }
    }

    pub fn increment(&mut self, replica_id: usize) {
        self.slots[replica_id] += 1;
    }

    pub fn value(&self) -> u64 {
        self.slots.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        for (mine, theirs) in self.slots.iter_mut().zip(&other.slots) {
            *mine = std::cmp::max(*mine, *theirs);
        }
    }
}
