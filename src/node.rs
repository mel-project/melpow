use crate::hash::{self, HashFunction};

use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use std::convert::TryInto;
use std::fmt;

pub type SVec<T> = SmallVec<[T; 40]>;

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct Node {
    pub bv: u64,
    pub len: usize,
}

impl Node {
    pub fn new_zero() -> Self {
        Node { bv: 0, len: 0 }
    }

    pub fn new(bv: u64, len: usize) -> Self {
        Node { bv, len }
    }

    pub fn take(self, n: usize) -> Self {
        let mut new = self;
        new.bv &= (1 << n) - 1;
        new.len = n;
        new
    }

    pub fn append(self, n: usize) -> Self {
        let mut nd = self;
        nd.bv |= (n << nd.len) as u64;
        nd.len += 1;
        nd
    }

    pub fn get_bit(self, n: usize) -> u64 {
        self.bv >> n & 1
    }

    pub fn get_parents(&self, n: usize) -> Vec<Node> {
        let mut v = vec![];
        self.foreach_parent(n, |p| v.push(p));
        v
    }

    pub fn foreach_parent(self, n: usize, mut f: impl FnMut(Node)) {
        if self.len == n {
            for index in 0..n {
                if (self.bv >> index) & 1 != 0 {
                    f(self.take(index).append(0))
                }
            }
        } else {
            f(self.append(0));
            f(self.append(1));
        }
    }

    pub fn uniqid(self) -> u64 {
        (self.len as u64) << 56 | self.bv
    }

    pub fn to_bytes(self) -> [u8; 8] {
        self.uniqid().to_be_bytes()
    }

    pub fn from_bytes(bts: &[u8]) -> Option<Self> {
        let uniqid = u64::from_be_bytes(bts.try_into().ok()?);
        // highest 8 bits is length
        let len = (uniqid >> 56) as usize;
        // lowest 56 bits is the number
        let num = uniqid << 8 >> 8;
        Some(Node { bv: num, len })
    }
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let str = {
            if self.len == 0 {
                String::from("ε")
            } else {
                (0..self.len)
                    .map(|i| if (self.bv >> i) & 1 != 0 { '1' } else { '0' })
                    .collect()
            }
        };
        write!(f, "{}", str)
    }
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let str = {
            if self.len == 0 {
                String::from("ε")
            } else {
                (0..self.len)
                    .map(|i| if (self.bv >> i) & 1 != 0 { '1' } else { '0' })
                    .collect()
            }
        };
        write!(f, "{}", str)
    }
}

pub fn calc_labels<H: HashFunction>(chi: &[u8], n: usize, f: &mut impl FnMut(Node, &[u8]), h: H) {
    calc_labels_helper(chi, n, Node::new_zero(), f, &mut FxHashMap::default(), &h);
    // // iterative implementation
    // let mut memoizer: FxHashMap<Node, SVec<u8>> = FxHashMap::default();
    // // let mut memoizer: Vec<(Node, SVec<u8>)> = Default::default();
    // let mut stack = Vec::with_capacity(32);
    // stack.push((false, Node::new_zero()));
    // while let Some((revisit, nd)) = stack.pop() {
    //     // eprintln!(
    //     //     "visiting {} at stack size {} and memoizer size {} ",
    //     //     nd,
    //     //     stack.len(),
    //     //     memoizer.len()
    //     // );
    //     if nd.len == n {
    //         let mut lab_gen = hash::Accumulator::new(chi);
    //         lab_gen.add(&nd.to_bytes());
    //         nd.foreach_parent(n, |parent| {
    //             lab_gen.add(memoizer.get(&parent).unwrap());
    //         });

    //         let lab = lab_gen.hash();
    //         f(nd, &lab);
    //         memoizer.insert(nd, lab);
    //     } else if !revisit {
    //         stack.push((true, nd));
    //         stack.push((false, nd.append(1)));
    //         stack.push((false, nd.append(0)));
    //     } else {
    //         let l0 = memoizer.get(&nd.append(0)).unwrap().clone();
    //         let l1 = memoizer.get(&nd.append(1)).unwrap().clone();
    //         memoizer.remove(&nd.append(0));
    //         memoizer.remove(&nd.append(1));
    //         let lab = hash::Accumulator::new(chi)
    //             .add(&nd.to_bytes())
    //             .add(&l0)
    //             .add(&l1)
    //             .hash();
    //         f(nd, &lab);
    //         memoizer.insert(nd, lab);
    //     }
    // }
}

#[inline]
fn calc_labels_helper<H: HashFunction>(
    chi: &[u8],
    n: usize,
    nd: Node,
    f: &mut impl FnMut(Node, &[u8]),
    ell: &mut FxHashMap<Node, SVec<u8>>,
    hasher: &H,
) -> SVec<u8> {
    if nd.len == n {
        let mut lab_gen = hash::Accumulator::new(chi, hasher);
        lab_gen.add(&nd.to_bytes());
        nd.foreach_parent(n, |parent| {
            lab_gen.add(&ell[&parent]);
        });

        let lab = lab_gen.hash();
        f(nd, &lab);
        lab
    } else {
        // left tree
        let l0 = calc_labels_helper(chi, n, nd.append(0), f, ell, hasher);
        ell.insert(nd.append(0), l0.clone());
        // right tree
        let l1 = calc_labels_helper(chi, n, nd.append(1), f, ell, hasher);
        ell.remove(&nd.append(0));
        // calculate label
        let lab = hash::Accumulator::new(chi, hasher)
            .add(&nd.to_bytes())
            .add(&l0)
            .add(&l1)
            .hash();
        f(nd, &lab);
        lab
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    fn print_dag(n: usize, b: Node) {
        println!("digraph G {{");
        println!("rankdir = BT;");
        println!("graph [splines=line];");
        println!("subgraph {{");
        print_dag_helper(n, b, &mut HashSet::new());
        println!("}}\n}}");
    }

    fn print_dag_helper(n: usize, b: Node, printed: &mut HashSet<(usize, Node)>) {
        if printed.contains(&(n, b)) {
            return;
        }
        printed.insert((n, b));

        b.get_parents(n).iter().for_each(|parent| {
            if parent.len <= b.len {
                println!("\"{}\" -> \"{}\" [constraint=false]", parent, b)
            } else {
                println!("\"{}\" -> \"{}\"", parent, b)
            }
            print_dag_helper(n, *parent, printed)
        });
    }

    #[test]
    fn test_dag() {
        let n = 4;
        let root = Node::new_zero();
        print_dag(n, root)
    }
}
