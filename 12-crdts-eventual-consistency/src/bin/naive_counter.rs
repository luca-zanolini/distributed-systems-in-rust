fn merge_max(mine: u64, theirs: u64) -> u64 {
    std::cmp::max(mine, theirs)
}

fn merge_add(mine: u64, theirs: u64) -> u64 {
    mine + theirs
}

fn main() {
    let mut a = 0;
    let mut b = 0;
    for _ in 0..3 {
        a += 1;
    }
    for _ in 0..5 {
        b += 1;
    }
    println!(
        "During partition: a={}, b={} (true global count: {})",
        a,
        b,
        a + b
    );
    let merged_max = merge_max(a, b);
    println!(
        "After merge (max): a={}, b={}, merged_max={}",
        a, b, merged_max
    );

    // Merge using add
    let merged_add = merge_add(a, b);
    println!(
        "After merge (add): a={}, b={}, merged_add={}",
        a, b, merged_add
    );

    let after_dup = merge_add(merged_add, b);
    println!("B's gossip arrives AGAIN: {after_dup} — the same news counted twice");
}
