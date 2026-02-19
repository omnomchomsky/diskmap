pub mod tree;
pub mod model;
pub mod top_files;
pub mod session;
mod scanner;
pub mod fs_adapter;
pub mod scan_store;
pub mod view;

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
