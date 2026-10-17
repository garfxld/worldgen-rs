use shulkr::util::BlockPosition;
use std::iter;
use std::mem;

const DEFAULT_CAPACITY: usize = 16;
const TREEIFY_THRESHOLD: usize = 8;
const UNTREEIFY_THRESHOLD: usize = 6;
const MIN_TREEIFY_CAPACITY: usize = 64;

#[derive(Clone, Copy)]
struct Node {
    pos: BlockPosition,
    hash: i32,
    next: Option<usize>,
    prev: Option<usize>,
    parent: Option<usize>,
    left: Option<usize>,
    right: Option<usize>,
    red: bool,
}

pub struct JavaHashSet {
    nodes: Vec<Node>,
    table: Vec<Option<usize>>,
    tree_bin: Vec<bool>,
    size: usize,
}

impl JavaHashSet {
    pub fn new() -> JavaHashSet {
        JavaHashSet {
            nodes: Vec::new(),
            table: vec![None; DEFAULT_CAPACITY],
            tree_bin: vec![false; DEFAULT_CAPACITY],
            size: 0,
        }
    }

    const fn hash(pos: BlockPosition) -> i32 {
        let raw = pos
            .y()
            .wrapping_add(pos.z().wrapping_mul(31))
            .wrapping_mul(31)
            .wrapping_add(pos.x());
        raw ^ ((raw as u32) >> 16) as i32
    }

    const fn index_for(&self, hash: i32) -> usize {
        hash as u32 as usize & (self.table.len() - 1)
    }

    const fn threshold(&self) -> usize {
        self.table.len() * 3 / 4
    }

    pub const fn len(&self) -> usize {
        self.size
    }

    pub const fn is_empty(&self) -> bool {
        self.size == 0
    }

    fn chain(&self, head: Option<usize>) -> impl Iterator<Item = usize> + '_ {
        iter::successors(head, |&node| self.nodes[node].next)
    }

    pub fn contains(&self, pos: BlockPosition) -> bool {
        let hash = Self::hash(pos);
        let head = self.table[self.index_for(hash)];
        self.chain(head)
            .any(|node| self.nodes[node].hash == hash && self.nodes[node].pos == pos)
    }

    fn new_node(&mut self, pos: BlockPosition, hash: i32, next: Option<usize>) -> usize {
        self.nodes.push(Node {
            pos,
            hash,
            next,
            prev: None,
            parent: None,
            left: None,
            right: None,
            red: false,
        });
        self.nodes.len() - 1
    }

    pub fn insert(&mut self, pos: BlockPosition) -> bool {
        if self.contains(pos) {
            return false;
        }
        let hash = Self::hash(pos);
        let index = self.index_for(hash);
        match self.table[index] {
            None => {
                let node = self.new_node(pos, hash, None);
                self.table[index] = Some(node);
            }
            Some(first) if self.tree_bin[index] => self.put_tree_val(index, first, pos, hash),
            Some(first) => {
                let mut p = first;
                let mut bin_count = 0;
                loop {
                    match self.nodes[p].next {
                        None => {
                            let node = self.new_node(pos, hash, None);
                            self.nodes[p].next = Some(node);
                            if bin_count >= TREEIFY_THRESHOLD - 1 {
                                self.treeify_bin(hash);
                            }
                            break;
                        }
                        Some(next) => p = next,
                    }
                    bin_count += 1;
                }
            }
        }
        self.size += 1;
        if self.size > self.threshold() {
            self.resize();
        }
        true
    }

    fn treeify_bin(&mut self, hash: i32) {
        if self.table.len() < MIN_TREEIFY_CAPACITY {
            self.resize();
            return;
        }
        let index = self.index_for(hash);
        let Some(head) = self.table[index] else {
            return;
        };
        let mut previous = None;
        let mut node = Some(head);
        while let Some(current) = node {
            self.nodes[current].prev = previous;
            previous = Some(current);
            node = self.nodes[current].next;
        }
        self.tree_bin[index] = true;
        self.treeify(head);
    }

    fn treeify(&mut self, head: usize) {
        let mut root: Option<usize> = None;
        let mut x = Some(head);
        while let Some(current) = x {
            let next = self.nodes[current].next;
            self.nodes[current].left = None;
            self.nodes[current].right = None;
            match root {
                None => {
                    self.nodes[current].parent = None;
                    self.nodes[current].red = false;
                    root = Some(current);
                }
                Some(tree_root) => {
                    let h = self.nodes[current].hash;
                    let mut p = tree_root;
                    loop {
                        let ph = self.nodes[p].hash;
                        let dir = if ph > h {
                            -1
                        } else if ph < h {
                            1
                        } else {
                            Self::tie_break_order()
                        };
                        let xp = p;
                        let child = if dir <= 0 {
                            self.nodes[p].left
                        } else {
                            self.nodes[p].right
                        };
                        match child {
                            Some(child) => p = child,
                            None => {
                                self.nodes[current].parent = Some(xp);
                                if dir <= 0 {
                                    self.nodes[xp].left = Some(current);
                                } else {
                                    self.nodes[xp].right = Some(current);
                                }
                                root = Some(self.balance_insertion(tree_root, current));
                                break;
                            }
                        }
                    }
                }
            }
            x = next;
        }
        if let Some(root) = root {
            self.move_root_to_front(root);
        }
    }

    const fn tie_break_order() -> i32 {
        -1
    }

    fn put_tree_val(&mut self, index: usize, first: usize, pos: BlockPosition, hash: i32) {
        let root = self.root_of(first);
        let mut p = root;
        loop {
            let ph = self.nodes[p].hash;
            let dir = if ph > hash {
                -1
            } else if ph < hash {
                1
            } else {
                Self::tie_break_order()
            };
            let xp = p;
            let child = if dir <= 0 {
                self.nodes[p].left
            } else {
                self.nodes[p].right
            };
            match child {
                Some(child) => p = child,
                None => {
                    let xpn = self.nodes[xp].next;
                    let x = self.new_node(pos, hash, xpn);
                    if dir <= 0 {
                        self.nodes[xp].left = Some(x);
                    } else {
                        self.nodes[xp].right = Some(x);
                    }
                    self.nodes[xp].next = Some(x);
                    self.nodes[x].parent = Some(xp);
                    self.nodes[x].prev = Some(xp);
                    if let Some(xpn) = xpn {
                        self.nodes[xpn].prev = Some(x);
                    }
                    let new_root = self.balance_insertion(root, x);
                    debug_assert_eq!(self.index_for(self.nodes[new_root].hash), index);
                    self.move_root_to_front(new_root);
                    return;
                }
            }
        }
    }

    fn root_of(&self, node: usize) -> usize {
        let mut r = node;
        while let Some(parent) = self.nodes[r].parent {
            r = parent;
        }
        r
    }

    fn move_root_to_front(&mut self, root: usize) {
        let index = self.index_for(self.nodes[root].hash);
        let first = self.table[index];
        if first == Some(root) {
            return;
        }
        let rn = self.nodes[root].next;
        let rp = self.nodes[root].prev;
        if let Some(rn) = rn {
            self.nodes[rn].prev = rp;
        }
        if let Some(rp) = rp {
            self.nodes[rp].next = rn;
        }
        if let Some(first) = first {
            self.nodes[first].prev = Some(root);
        }
        self.nodes[root].next = first;
        self.nodes[root].prev = None;
        self.table[index] = Some(root);
    }

    fn is_red(&self, node: Option<usize>) -> bool {
        node.is_some_and(|node| self.nodes[node].red)
    }

    fn rotate_left(&mut self, mut root: usize, p: usize) -> usize {
        let Some(r) = self.nodes[p].right else {
            return root;
        };
        let rl = self.nodes[r].left;
        self.nodes[p].right = rl;
        if let Some(rl) = rl {
            self.nodes[rl].parent = Some(p);
        }
        let pp = self.nodes[p].parent;
        self.nodes[r].parent = pp;
        match pp {
            None => {
                root = r;
                self.nodes[r].red = false;
            }
            Some(pp) => {
                if self.nodes[pp].left == Some(p) {
                    self.nodes[pp].left = Some(r);
                } else {
                    self.nodes[pp].right = Some(r);
                }
            }
        }
        self.nodes[r].left = Some(p);
        self.nodes[p].parent = Some(r);
        root
    }

    fn rotate_right(&mut self, mut root: usize, p: usize) -> usize {
        let Some(l) = self.nodes[p].left else {
            return root;
        };
        let lr = self.nodes[l].right;
        self.nodes[p].left = lr;
        if let Some(lr) = lr {
            self.nodes[lr].parent = Some(p);
        }
        let pp = self.nodes[p].parent;
        self.nodes[l].parent = pp;
        match pp {
            None => {
                root = l;
                self.nodes[l].red = false;
            }
            Some(pp) => {
                if self.nodes[pp].right == Some(p) {
                    self.nodes[pp].right = Some(l);
                } else {
                    self.nodes[pp].left = Some(l);
                }
            }
        }
        self.nodes[l].right = Some(p);
        self.nodes[p].parent = Some(l);
        root
    }

    fn balance_insertion(&mut self, mut root: usize, mut x: usize) -> usize {
        self.nodes[x].red = true;
        loop {
            let Some(mut xp) = self.nodes[x].parent else {
                self.nodes[x].red = false;
                return x;
            };
            if !self.nodes[xp].red {
                return root;
            }
            let Some(mut xpp) = self.nodes[xp].parent else {
                return root;
            };
            let xppl = self.nodes[xpp].left;
            if xppl == Some(xp) {
                let xppr = self.nodes[xpp].right;
                if self.is_red(xppr) {
                    let xppr = xppr.expect("red node exists");
                    self.nodes[xppr].red = false;
                    self.nodes[xp].red = false;
                    self.nodes[xpp].red = true;
                    x = xpp;
                } else {
                    if self.nodes[xp].right == Some(x) {
                        x = xp;
                        root = self.rotate_left(root, x);
                        match self.nodes[x].parent {
                            Some(parent) => {
                                xp = parent;
                                match self.nodes[parent].parent {
                                    Some(grand) => xpp = grand,
                                    None => {
                                        self.nodes[xp].red = false;
                                        continue;
                                    }
                                }
                            }
                            None => continue,
                        }
                    }
                    self.nodes[xp].red = false;
                    self.nodes[xpp].red = true;
                    root = self.rotate_right(root, xpp);
                }
            } else {
                if self.is_red(xppl) {
                    let xppl = xppl.expect("red node exists");
                    self.nodes[xppl].red = false;
                    self.nodes[xp].red = false;
                    self.nodes[xpp].red = true;
                    x = xpp;
                } else {
                    if self.nodes[xp].left == Some(x) {
                        x = xp;
                        root = self.rotate_right(root, x);
                        match self.nodes[x].parent {
                            Some(parent) => {
                                xp = parent;
                                match self.nodes[parent].parent {
                                    Some(grand) => xpp = grand,
                                    None => {
                                        self.nodes[xp].red = false;
                                        continue;
                                    }
                                }
                            }
                            None => continue,
                        }
                    }
                    self.nodes[xp].red = false;
                    self.nodes[xpp].red = true;
                    root = self.rotate_left(root, xpp);
                }
            }
        }
    }

    fn resize(&mut self) {
        let old_capacity = self.table.len();
        let new_capacity = old_capacity * 2;
        let old_table = mem::replace(&mut self.table, vec![None; new_capacity]);
        let old_tree = mem::replace(&mut self.tree_bin, vec![false; new_capacity]);
        for (j, head) in old_table.into_iter().enumerate() {
            let Some(head) = head else {
                continue;
            };
            if self.nodes[head].next.is_none() {
                let index = self.index_for(self.nodes[head].hash);
                self.table[index] = Some(head);
                self.tree_bin[index] = old_tree[j];
            } else if old_tree[j] {
                self.split(head, j, old_capacity);
            } else {
                let mut lo: Vec<usize> = Vec::new();
                let mut hi: Vec<usize> = Vec::new();
                for node in self.chain(Some(head)).collect::<Vec<_>>() {
                    if self.nodes[node].hash as u32 as usize & old_capacity == 0 {
                        lo.push(node);
                    } else {
                        hi.push(node);
                    }
                }
                self.link_list(j, &lo);
                self.link_list(j + old_capacity, &hi);
            }
        }
    }

    fn link_list(&mut self, index: usize, nodes: &[usize]) {
        if nodes.is_empty() {
            return;
        }
        for pair in nodes.windows(2) {
            self.nodes[pair[0]].next = Some(pair[1]);
        }
        let last = nodes[nodes.len() - 1];
        self.nodes[last].next = None;
        self.table[index] = Some(nodes[0]);
        self.tree_bin[index] = false;
    }

    fn split(&mut self, head: usize, index: usize, bit: usize) {
        let chain: Vec<usize> = self.chain(Some(head)).collect();
        let mut lo: Vec<usize> = Vec::new();
        let mut hi: Vec<usize> = Vec::new();
        for node in chain {
            self.nodes[node].next = None;
            let list = if self.nodes[node].hash as u32 as usize & bit == 0 {
                &mut lo
            } else {
                &mut hi
            };
            self.nodes[node].prev = list.last().copied();
            if let Some(&tail) = list.last() {
                self.nodes[tail].next = Some(node);
            }
            list.push(node);
        }
        if let Some(&lo_head) = lo.first() {
            if lo.len() <= UNTREEIFY_THRESHOLD {
                self.link_list(index, &lo);
            } else {
                self.table[index] = Some(lo_head);
                self.tree_bin[index] = true;
                if !hi.is_empty() {
                    self.treeify(lo_head);
                }
            }
        }
        if let Some(&hi_head) = hi.first() {
            if hi.len() <= UNTREEIFY_THRESHOLD {
                self.link_list(index + bit, &hi);
            } else {
                self.table[index + bit] = Some(hi_head);
                self.tree_bin[index + bit] = true;
                if !lo.is_empty() {
                    self.treeify(hi_head);
                }
            }
        }
    }

    pub fn remove_first(&mut self) -> Option<BlockPosition> {
        let index = self.table.iter().position(Option::is_some)?;
        let node = self.table[index].expect("bin is non-empty");
        let pos = self.nodes[node].pos;
        if self.tree_bin[index] {
            self.remove_tree_node(node);
        } else {
            self.table[index] = self.nodes[node].next;
        }
        self.size -= 1;
        Some(pos)
    }

    fn remove_tree_node(&mut self, p: usize) {
        let index = self.index_for(self.nodes[p].hash);
        let Some(original_first) = self.table[index] else {
            return;
        };
        let mut root = original_first;
        let succ = self.nodes[p].next;
        let pred = self.nodes[p].prev;
        let first = match pred {
            None => {
                self.table[index] = succ;
                succ
            }
            Some(pred) => {
                self.nodes[pred].next = succ;
                self.table[index]
            }
        };
        if let Some(succ) = succ {
            self.nodes[succ].prev = pred;
        }
        if first.is_none() {
            self.tree_bin[index] = false;
            return;
        }
        if self.nodes[root].parent.is_some() {
            root = self.root_of(root);
        }

        let pl = self.nodes[p].left;
        let pr = self.nodes[p].right;
        let replacement;
        if let (Some(pl), Some(pr)) = (pl, pr) {
            let mut s = pr;
            while let Some(sl) = self.nodes[s].left {
                s = sl;
            }
            let c = self.nodes[s].red;
            self.nodes[s].red = self.nodes[p].red;
            self.nodes[p].red = c;
            let sr = self.nodes[s].right;
            let pp = self.nodes[p].parent;
            if s == pr {
                self.nodes[p].parent = Some(s);
                self.nodes[s].right = Some(p);
            } else {
                let sp = self.nodes[s].parent;
                self.nodes[p].parent = sp;
                if let Some(sp) = sp {
                    if self.nodes[sp].left == Some(s) {
                        self.nodes[sp].left = Some(p);
                    } else {
                        self.nodes[sp].right = Some(p);
                    }
                }
                self.nodes[s].right = Some(pr);
                self.nodes[pr].parent = Some(s);
            }
            self.nodes[p].left = None;
            self.nodes[p].right = sr;
            if let Some(sr) = sr {
                self.nodes[sr].parent = Some(p);
            }
            self.nodes[s].left = Some(pl);
            self.nodes[pl].parent = Some(s);
            self.nodes[s].parent = pp;
            match pp {
                None => root = s,
                Some(pp) => {
                    if self.nodes[pp].left == Some(p) {
                        self.nodes[pp].left = Some(s);
                    } else {
                        self.nodes[pp].right = Some(s);
                    }
                }
            }
            replacement = sr.unwrap_or(p);
        } else if let Some(pl) = pl {
            replacement = pl;
        } else if let Some(pr) = pr {
            replacement = pr;
        } else {
            replacement = p;
        }
        if replacement != p {
            let pp = self.nodes[p].parent;
            self.nodes[replacement].parent = pp;
            match pp {
                None => {
                    root = replacement;
                    self.nodes[replacement].red = false;
                }
                Some(pp) => {
                    if self.nodes[pp].left == Some(p) {
                        self.nodes[pp].left = Some(replacement);
                    } else {
                        self.nodes[pp].right = Some(replacement);
                    }
                }
            }
            self.nodes[p].left = None;
            self.nodes[p].right = None;
            self.nodes[p].parent = None;
        }
        if !self.nodes[p].red {
            self.balance_deletion(root, replacement);
        }
        if replacement == p {
            let pp = self.nodes[p].parent;
            self.nodes[p].parent = None;
            if let Some(pp) = pp {
                if self.nodes[pp].left == Some(p) {
                    self.nodes[pp].left = None;
                } else if self.nodes[pp].right == Some(p) {
                    self.nodes[pp].right = None;
                }
            }
        }
    }

    fn balance_deletion(&mut self, mut root: usize, mut x: usize) -> usize {
        loop {
            if x == root {
                return root;
            }
            let Some(mut xp) = self.nodes[x].parent else {
                self.nodes[x].red = false;
                return x;
            };
            if self.nodes[x].red {
                self.nodes[x].red = false;
                return root;
            }
            if self.nodes[xp].left == Some(x) {
                let mut xpr = self.nodes[xp].right;
                if self.is_red(xpr) {
                    let sibling = xpr.expect("red node exists");
                    self.nodes[sibling].red = false;
                    self.nodes[xp].red = true;
                    root = self.rotate_left(root, xp);
                    match self.nodes[x].parent {
                        Some(parent) => {
                            xp = parent;
                            xpr = self.nodes[parent].right;
                        }
                        None => xpr = None,
                    }
                }
                match xpr {
                    None => x = xp,
                    Some(mut sibling) => {
                        let sl = self.nodes[sibling].left;
                        let sr = self.nodes[sibling].right;
                        if !self.is_red(sr) && !self.is_red(sl) {
                            self.nodes[sibling].red = true;
                            x = xp;
                        } else {
                            let mut xp_opt = Some(xp);
                            let mut sibling_opt = Some(sibling);
                            if !self.is_red(sr) {
                                if let Some(sl) = sl {
                                    self.nodes[sl].red = false;
                                }
                                self.nodes[sibling].red = true;
                                root = self.rotate_right(root, sibling);
                                xp_opt = self.nodes[x].parent;
                                sibling_opt = xp_opt.and_then(|xp| self.nodes[xp].right);
                            }
                            if let Some(s) = sibling_opt {
                                sibling = s;
                                self.nodes[sibling].red =
                                    xp_opt.is_some_and(|xp| self.nodes[xp].red);
                                if let Some(sr) = self.nodes[sibling].right {
                                    self.nodes[sr].red = false;
                                }
                            }
                            if let Some(xp) = xp_opt {
                                self.nodes[xp].red = false;
                                root = self.rotate_left(root, xp);
                            }
                            x = root;
                        }
                    }
                }
            } else {
                let mut xpl = self.nodes[xp].left;
                if self.is_red(xpl) {
                    let sibling = xpl.expect("red node exists");
                    self.nodes[sibling].red = false;
                    self.nodes[xp].red = true;
                    root = self.rotate_right(root, xp);
                    match self.nodes[x].parent {
                        Some(parent) => {
                            xp = parent;
                            xpl = self.nodes[parent].left;
                        }
                        None => xpl = None,
                    }
                }
                match xpl {
                    None => x = xp,
                    Some(mut sibling) => {
                        let sl = self.nodes[sibling].left;
                        let sr = self.nodes[sibling].right;
                        if !self.is_red(sl) && !self.is_red(sr) {
                            self.nodes[sibling].red = true;
                            x = xp;
                        } else {
                            let mut xp_opt = Some(xp);
                            let mut sibling_opt = Some(sibling);
                            if !self.is_red(sl) {
                                if let Some(sr) = sr {
                                    self.nodes[sr].red = false;
                                }
                                self.nodes[sibling].red = true;
                                root = self.rotate_left(root, sibling);
                                xp_opt = self.nodes[x].parent;
                                sibling_opt = xp_opt.and_then(|xp| self.nodes[xp].left);
                            }
                            if let Some(s) = sibling_opt {
                                sibling = s;
                                self.nodes[sibling].red =
                                    xp_opt.is_some_and(|xp| self.nodes[xp].red);
                                if let Some(sl) = self.nodes[sibling].left {
                                    self.nodes[sl].red = false;
                                }
                            }
                            if let Some(xp) = xp_opt {
                                self.nodes[xp].red = false;
                                root = self.rotate_right(root, xp);
                            }
                            x = root;
                        }
                    }
                }
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = BlockPosition> + '_ {
        self.table
            .iter()
            .flat_map(move |&head| self.chain(head).map(move |node| self.nodes[node].pos))
    }
}

impl Default for JavaHashSet {
    fn default() -> JavaHashSet {
        JavaHashSet::new()
    }
}

impl FromIterator<BlockPosition> for JavaHashSet {
    fn from_iter<T: IntoIterator<Item = BlockPosition>>(iter: T) -> JavaHashSet {
        let mut set = JavaHashSet::new();
        for pos in iter {
            set.insert(pos);
        }
        set
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    #[test]
    fn matches_java_hash_set_reference_cases() {
        let Ok(path) = env::var("JAVA_HASH_SET_CASES") else {
            return;
        };
        let text = fs::read_to_string(path).expect("cases file");
        let parse = |s: &str| {
            let v: Vec<i32> = s.split(' ').map(|n| n.parse().expect("int")).collect();
            BlockPosition::new(v[0], v[1], v[2])
        };
        for (line_no, line) in text.lines().enumerate() {
            let (ops, expected) = line.split_once('|').expect("separator");
            let mut set = JavaHashSet::new();
            for op in ops.split(';').filter(|op| !op.is_empty()) {
                if op == "R" {
                    set.remove_first();
                } else {
                    set.insert(parse(&op[2..]));
                }
            }
            let expected: Vec<BlockPosition> = expected
                .split(';')
                .filter(|p| !p.is_empty())
                .map(parse)
                .collect();
            let actual: Vec<BlockPosition> = set.iter().collect();
            assert_eq!(actual, expected, "case {line_no}");
            assert_eq!(set.len(), expected.len(), "case {line_no}");
        }
    }
}
