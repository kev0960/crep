pub struct PermutationIterator {
    limit: Vec<u32>,
    pub current: Option<Vec<u32>>,
}

impl PermutationIterator {
    pub fn new(limit: &[u32]) -> Self {
        let current = vec![0; limit.len()];

        if limit.iter().any(|v| v == &0) {
            panic!("Limit cannot contain zero");
        }

        Self {
            limit: limit.to_vec(),
            current: Some(current),
        }
    }
}

impl Iterator for PermutationIterator {
    type Item = Vec<u32>;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current.clone()?;
        let next = self.current.as_mut().unwrap();

        for i in (0..next.len()).rev() {
            if next[i] < self.limit[i] - 1 {
                next[i] += 1;

                next[i + 1..].fill(0);

                return Some(current);
            }

            next[i] = 0;
        }

        self.current = None;
        Some(current)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_permutation() {
        let mut itr = PermutationIterator::new(&[1, 2, 3]);

        assert_eq!(itr.next(), Some(vec![0, 0, 0]));
        assert_eq!(itr.next(), Some(vec![0, 0, 1]));
        assert_eq!(itr.next(), Some(vec![0, 0, 2]));
        assert_eq!(itr.next(), Some(vec![0, 1, 0]));
        assert_eq!(itr.next(), Some(vec![0, 1, 1]));
        assert_eq!(itr.next(), Some(vec![0, 1, 2]));
        assert_eq!(itr.next(), None);
    }

    #[test]
    fn test_permutation_all_ones() {
        let mut itr = PermutationIterator::new(&[1, 1, 1]);

        assert_eq!(itr.next(), Some(vec![0, 0, 0]));
        assert_eq!(itr.next(), None);
    }

    #[test]
    fn test_permutation_binary() {
        let mut itr = PermutationIterator::new(&[2, 2, 2]);

        assert_eq!(itr.next(), Some(vec![0, 0, 0]));
        assert_eq!(itr.next(), Some(vec![0, 0, 1]));
        assert_eq!(itr.next(), Some(vec![0, 1, 0]));
        assert_eq!(itr.next(), Some(vec![0, 1, 1]));
        assert_eq!(itr.next(), Some(vec![1, 0, 0]));
        assert_eq!(itr.next(), Some(vec![1, 0, 1]));
        assert_eq!(itr.next(), Some(vec![1, 1, 0]));
        assert_eq!(itr.next(), Some(vec![1, 1, 1]));
        assert_eq!(itr.next(), None);
    }
}
