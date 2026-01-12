mod cli_treemap;

use dm_core::session::Session;
use dm_core::fs_adapter::UnixFsAdapter;

fn main() {
    let now = std::time::Instant::now();
    let fs = UnixFsAdapter;
    let root = std::env::args()
        .nth(1)
        .unwrap_or_else(|| ".".to_string());
    let mut session = Session::new(root.parse().unwrap(), 10);
    session.run(&fs);


    println!("{:?}", session.tree);
    println!("{:?}", session.errors);
    println!("{:?}", now.elapsed());

}