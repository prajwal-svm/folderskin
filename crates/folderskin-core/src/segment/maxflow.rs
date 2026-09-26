//! The minimum cut of a graph with a source and a sink, by Boykov and Kolmogorov's max-flow: two
//! search trees grown from the terminals, augmented along the paths where they meet and repaired
//! by adopting the nodes an augmentation orphans. It is the algorithm of Kolmogorov's maxflow 3.0,
//! and fast on the graphs a picture makes: a node per pixel, joined to its neighbours.
//!
//! Capacities are whole numbers, so a cut is exact and the same on every computer.

/// No node, arc or parent.
const NONE: u32 = u32::MAX;
/// A node's parent is a terminal.
const TERMINAL: u32 = u32::MAX - 1;
/// A node lost its parent and waits for adoption.
const ORPHAN: u32 = u32::MAX - 2;
const INFINITE_DISTANCE: u32 = u32::MAX;

/// One direction of an edge. Arcs are added in pairs, so an arc's sister is its index with the
/// lowest bit flipped.
#[derive(Clone, Copy)]
struct Arc {
    head: u32,
    next: u32,
    residual: i32,
}

/// A graph whose nodes are joined to the source, the sink and each other.
pub struct Graph {
    first: Vec<u32>,
    parent: Vec<u32>,
    /// When a node's distance to its terminal was last known to be right.
    stamp: Vec<u32>,
    distance: Vec<u32>,
    in_sink_tree: Vec<bool>,
    /// What the node can still take from the source (positive) or give the sink (negative).
    terminal: Vec<i64>,
    queued: Vec<bool>,
    arcs: Vec<Arc>,
    active: std::collections::VecDeque<u32>,
    orphans: std::collections::VecDeque<u32>,
    time: u32,
    flow: i64,
}

impl Graph {
    /// A graph of `nodes` nodes with room for `edges` edges between them.
    pub fn new(nodes: usize, edges: usize) -> Self {
        Graph {
            first: vec![NONE; nodes],
            parent: vec![NONE; nodes],
            stamp: vec![0; nodes],
            distance: vec![0; nodes],
            in_sink_tree: vec![false; nodes],
            terminal: vec![0; nodes],
            queued: vec![false; nodes],
            arcs: Vec::with_capacity(edges * 2),
            active: Default::default(),
            orphans: Default::default(),
            time: 0,
            flow: 0,
        }
    }

    /// Joins node `i` to the source with `from_source` and to the sink with `to_sink`. Only the
    /// difference goes into the graph: the rest flows from one terminal to the other whatever the
    /// cut.
    pub fn add_terminals(&mut self, i: usize, from_source: i64, to_sink: i64) {
        let (mut source, mut sink) = (from_source, to_sink);
        let before = self.terminal[i];
        if before > 0 {
            source += before;
        } else {
            sink -= before;
        }
        self.flow += source.min(sink);
        self.terminal[i] = source - sink;
    }

    /// Joins `i` to `j` with `capacity` each way.
    pub fn add_edge(&mut self, i: usize, j: usize, capacity: i32) {
        let a = self.arcs.len() as u32;
        self.arcs.push(Arc {
            head: j as u32,
            next: self.first[i],
            residual: capacity,
        });
        self.first[i] = a;
        self.arcs.push(Arc {
            head: i as u32,
            next: self.first[j],
            residual: capacity,
        });
        self.first[j] = a + 1;
    }

    fn activate(&mut self, i: u32) {
        if !self.queued[i as usize] {
            self.queued[i as usize] = true;
            self.active.push_back(i);
        }
    }

    fn next_active(&mut self) -> Option<u32> {
        while let Some(i) = self.active.pop_front() {
            self.queued[i as usize] = false;
            if self.parent[i as usize] != NONE {
                return Some(i);
            }
        }
        None
    }

    fn orphan_front(&mut self, i: u32) {
        self.parent[i as usize] = ORPHAN;
        self.orphans.push_front(i);
    }

    fn orphan_back(&mut self, i: u32) {
        self.parent[i as usize] = ORPHAN;
        self.orphans.push_back(i);
    }

    /// Pushes as much as the graph carries from the source to the sink and returns how much, with
    /// the terminals' own flow.
    pub fn maxflow(&mut self) -> i64 {
        for i in 0..self.first.len() {
            self.queued[i] = false;
            self.stamp[i] = 0;
            if self.terminal[i] != 0 {
                self.in_sink_tree[i] = self.terminal[i] < 0;
                self.parent[i] = TERMINAL;
                self.distance[i] = 1;
                self.activate(i as u32);
            } else {
                self.parent[i] = NONE;
            }
        }
        let mut current: Option<u32> = None;
        loop {
            // The node worked from last round loses its mark whether or not it's still in a tree:
            // one left marked would never be queued again.
            let current_now = current.take().filter(|&i| {
                self.queued[i as usize] = false;
                self.parent[i as usize] != NONE
            });
            let Some(i) = current_now.or_else(|| self.next_active()) else {
                break;
            };
            let meeting = self.grow(i);
            self.time += 1;
            if meeting != NONE {
                // Keep growing from `i` next round.
                self.queued[i as usize] = true;
                current = Some(i);
                self.augment(meeting);
                while let Some(o) = self.orphans.pop_front() {
                    self.adopt(o);
                }
            }
        }
        self.flow
    }

    /// Grows `i`'s tree by its free neighbours; the arc from the source's tree to the sink's where
    /// they meet, or [`NONE`].
    fn grow(&mut self, i: u32) -> u32 {
        let iu = i as usize;
        let sink = self.in_sink_tree[iu];
        let mut a = self.first[iu];
        while a != NONE {
            // The arc a path from the source would take: out of `i` in the source's tree, into
            // `i` in the sink's.
            let along = if sink { a ^ 1 } else { a };
            if self.arcs[along as usize].residual > 0 {
                let j = self.arcs[a as usize].head as usize;
                if self.parent[j] == NONE {
                    self.in_sink_tree[j] = sink;
                    self.parent[j] = a ^ 1;
                    self.stamp[j] = self.stamp[iu];
                    self.distance[j] = self.distance[iu] + 1;
                    self.activate(j as u32);
                } else if self.in_sink_tree[j] != sink {
                    return along;
                } else if self.stamp[j] <= self.stamp[iu] && self.distance[j] > self.distance[iu] {
                    // A shorter way to the terminal for `j`.
                    self.parent[j] = a ^ 1;
                    self.stamp[j] = self.stamp[iu];
                    self.distance[j] = self.distance[iu] + 1;
                }
            }
            a = self.arcs[a as usize].next;
        }
        NONE
    }

    /// Pushes the bottleneck along the path through `middle`, from the source's tree into the
    /// sink's, and orphans the nodes whose way to their terminal it saturated.
    fn augment(&mut self, middle: u32) {
        let mut bottleneck = self.arcs[middle as usize].residual as i64;
        // Back up the source's tree: each node's parent arc points at its parent, so the flow
        // runs along its sister.
        let mut i = self.arcs[(middle ^ 1) as usize].head;
        loop {
            let a = self.parent[i as usize];
            if a == TERMINAL {
                break;
            }
            bottleneck = bottleneck.min(self.arcs[(a ^ 1) as usize].residual as i64);
            i = self.arcs[a as usize].head;
        }
        bottleneck = bottleneck.min(self.terminal[i as usize]);
        let mut i = self.arcs[middle as usize].head;
        loop {
            let a = self.parent[i as usize];
            if a == TERMINAL {
                break;
            }
            bottleneck = bottleneck.min(self.arcs[a as usize].residual as i64);
            i = self.arcs[a as usize].head;
        }
        bottleneck = bottleneck.min(-self.terminal[i as usize]);
        let b = bottleneck as i32;

        self.arcs[middle as usize].residual -= b;
        self.arcs[(middle ^ 1) as usize].residual += b;
        let mut i = self.arcs[(middle ^ 1) as usize].head;
        loop {
            let a = self.parent[i as usize];
            if a == TERMINAL {
                break;
            }
            self.arcs[a as usize].residual += b;
            self.arcs[(a ^ 1) as usize].residual -= b;
            let up = self.arcs[a as usize].head;
            if self.arcs[(a ^ 1) as usize].residual == 0 {
                self.orphan_front(i);
            }
            i = up;
        }
        self.terminal[i as usize] -= bottleneck;
        if self.terminal[i as usize] == 0 {
            self.orphan_front(i);
        }
        let mut i = self.arcs[middle as usize].head;
        loop {
            let a = self.parent[i as usize];
            if a == TERMINAL {
                break;
            }
            self.arcs[(a ^ 1) as usize].residual += b;
            self.arcs[a as usize].residual -= b;
            let up = self.arcs[a as usize].head;
            if self.arcs[a as usize].residual == 0 {
                self.orphan_front(i);
            }
            i = up;
        }
        self.terminal[i as usize] += bottleneck;
        if self.terminal[i as usize] == 0 {
            self.orphan_front(i);
        }
        self.flow += bottleneck;
    }

    /// Finds orphan `i` a new parent in its tree, the one nearest the terminal; failing that it
    /// leaves the tree, and so do the children it had.
    fn adopt(&mut self, i: u32) {
        let iu = i as usize;
        let sink = self.in_sink_tree[iu];
        let mut best = NONE;
        let mut best_distance = INFINITE_DISTANCE;
        let mut a0 = self.first[iu];
        while a0 != NONE {
            // The arc flow would take from the neighbour to `i` (source's tree) or on from `i`
            // (sink's).
            let along = if sink { a0 } else { a0 ^ 1 };
            let j0 = self.arcs[a0 as usize].head;
            if self.arcs[along as usize].residual > 0
                && self.in_sink_tree[j0 as usize] == sink
                && self.parent[j0 as usize] != NONE
            {
                // How far `j0` is from the terminal, if it still reaches it.
                let mut j = j0;
                let mut d = 0u32;
                loop {
                    if self.stamp[j as usize] == self.time {
                        d = d.saturating_add(self.distance[j as usize]);
                        break;
                    }
                    let a = self.parent[j as usize];
                    d += 1;
                    if a == TERMINAL {
                        self.stamp[j as usize] = self.time;
                        self.distance[j as usize] = 1;
                        break;
                    }
                    if a == ORPHAN {
                        d = INFINITE_DISTANCE;
                        break;
                    }
                    j = self.arcs[a as usize].head;
                }
                if d < INFINITE_DISTANCE {
                    if d < best_distance {
                        best = a0;
                        best_distance = d;
                    }
                    // Remember the distances along the way for the next orphan.
                    let mut j = j0;
                    while self.stamp[j as usize] != self.time {
                        self.stamp[j as usize] = self.time;
                        self.distance[j as usize] = d;
                        d -= 1;
                        j = self.arcs[self.parent[j as usize] as usize].head;
                    }
                }
            }
            a0 = self.arcs[a0 as usize].next;
        }
        if best != NONE {
            self.parent[iu] = best;
            self.stamp[iu] = self.time;
            self.distance[iu] = best_distance + 1;
            return;
        }
        self.parent[iu] = NONE;
        let mut a0 = self.first[iu];
        while a0 != NONE {
            let j = self.arcs[a0 as usize].head;
            let a = self.parent[j as usize];
            if self.in_sink_tree[j as usize] == sink && a != NONE {
                let along = if sink { a0 } else { a0 ^ 1 };
                if self.arcs[along as usize].residual > 0 {
                    self.activate(j);
                }
                if a != TERMINAL && a != ORPHAN && self.arcs[a as usize].head == i {
                    self.orphan_back(j);
                }
            }
            a0 = self.arcs[a0 as usize].next;
        }
    }

    /// After [`Graph::maxflow`]: whether node `i` is on the source's side of the minimum cut.
    pub fn on_source_side(&self, i: usize) -> bool {
        self.parent[i] != NONE && !self.in_sink_tree[i]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Edmonds-Karp on a dense matrix: slow and plainly right.
    fn reference(terminals: &[(i64, i64)], edges: &[(usize, usize, i32)]) -> i64 {
        let n = terminals.len();
        let (source, sink) = (n, n + 1);
        let mut cap = vec![vec![0i64; n + 2]; n + 2];
        for (i, &(s, t)) in terminals.iter().enumerate() {
            cap[source][i] += s;
            cap[i][sink] += t;
        }
        for &(i, j, c) in edges {
            cap[i][j] += c as i64;
            cap[j][i] += c as i64;
        }
        let mut flow = 0;
        loop {
            let mut prev = vec![usize::MAX; n + 2];
            prev[source] = source;
            let mut queue = std::collections::VecDeque::from([source]);
            while let Some(u) = queue.pop_front() {
                for v in 0..n + 2 {
                    if prev[v] == usize::MAX && cap[u][v] > 0 {
                        prev[v] = u;
                        queue.push_back(v);
                    }
                }
            }
            if prev[sink] == usize::MAX {
                return flow;
            }
            let mut bottleneck = i64::MAX;
            let mut v = sink;
            while v != source {
                bottleneck = bottleneck.min(cap[prev[v]][v]);
                v = prev[v];
            }
            let mut v = sink;
            while v != source {
                cap[prev[v]][v] -= bottleneck;
                cap[v][prev[v]] += bottleneck;
                v = prev[v];
            }
            flow += bottleneck;
        }
    }

    /// The flow is the maximum on thousands of random graphs, and the cut it leaves costs exactly
    /// that much.
    #[test]
    fn the_flow_is_the_maximum_and_the_cut_costs_as_much() {
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut random = |m: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % m
        };
        for round in 0..4000 {
            let n = 2 + random(if round % 2 == 0 { 14 } else { 60 }) as usize;
            let terminals: Vec<(i64, i64)> = (0..n)
                .map(|_| (random(20) as i64, random(20) as i64))
                .collect();
            let mut edges = Vec::new();
            for _ in 0..random(if n > 16 { 240 } else { 40 }) {
                let (i, j) = (random(n as u64) as usize, random(n as u64) as usize);
                if i != j {
                    edges.push((i, j, random(15) as i32));
                }
            }
            let mut graph = Graph::new(n, edges.len());
            for (i, &(s, t)) in terminals.iter().enumerate() {
                graph.add_terminals(i, s, t);
            }
            for &(i, j, c) in &edges {
                graph.add_edge(i, j, c);
            }
            let flow = graph.maxflow();
            assert_eq!(flow, reference(&terminals, &edges), "round {round}");
            let side: Vec<bool> = (0..n).map(|i| graph.on_source_side(i)).collect();
            let mut cut = 0i64;
            for (i, &(s, t)) in terminals.iter().enumerate() {
                cut += if side[i] { t } else { s };
            }
            for &(i, j, c) in &edges {
                if side[i] != side[j] {
                    cut += c as i64;
                }
            }
            assert_eq!(cut, flow, "round {round}");
        }
    }
}
