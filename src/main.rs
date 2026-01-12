use crate::dm_core::session::Session;
use crate::dm_core::fs_adapter::UnixFsAdapter;

fn main() {
    let fs = UnixFsAdapter::new();
    let root = std::env::args()
        .nth(1)
        .unwrap_or_else(|| ".".to_string());
    let mut session = Session::new(root, 10);
    session.run(&fs);

    println!("{:?}", session.tree);
    println!("{:?}", session.top_k_files);
    println!("{:?}", session.errors);

}