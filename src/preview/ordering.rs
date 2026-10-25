/// Return indices sorted by the provided priority slice (ascending).
pub fn order_indices(priorities: &[i32]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..priorities.len()).collect();
    idx.sort_by_key(|&i| priorities[i]);
    idx
}

#[cfg(test)]
mod tests {
    use super::order_indices;

    #[test]
    fn empty() {
        assert_eq!(order_indices(&[]), [] as [usize; 0]);
    }

    #[test]
    fn sorted() {
        assert_eq!(order_indices(&[3, 1, 2]), vec![1, 2, 0]);
    }

    #[test]
    fn stable_equal() {
        // equal priorities keep original order (stable sort)
        assert_eq!(order_indices(&[1, 1, 1]), vec![0, 1, 2]);
    }
}
