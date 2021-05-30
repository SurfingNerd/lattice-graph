use fixedbitset::FixedBitSet;
use itertools::*;
use petgraph::{
    data::{DataMap, DataMapMut},
    graph::IndexType,
    visit::{
        Data, EdgeRef, GraphBase, GraphProp, IntoEdgeReferences, IntoEdges, IntoNeighbors,
        IntoNodeIdentifiers, IntoNodeReferences, NodeCompactIndexable, NodeCount, NodeIndexable,
        VisitMap, Visitable,
    },
    Undirected,
};
use std::{iter::FusedIterator, marker::PhantomData, ops::Range, usize};

#[derive(Clone, Debug)]
pub struct SquareGraph<N, E, Ix = usize>
where
    Ix: IndexType,
{
    nodes: Vec</*horizontal*/ Box<[N]>>,
    vertical: Vec<Box<[E]>>,   //↓
    horizontal: Vec<Box<[E]>>, //→
    pd: PhantomData<Ix>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Axis {
    Vertical,
    Horizontal,
}

impl Axis {
    pub fn is_vertical(&self) -> bool {
        *self == Axis::Vertical
    }
    pub fn is_horizontal(&self) -> bool {
        *self == Axis::Horizontal
    }
}

impl<N, E, Ix> SquareGraph<N, E, Ix>
where
    Ix: IndexType,
{
    pub fn new_raw(
        nodes: Vec<Box<[N]>>,
        vertical: Vec<Box<[E]>>,
        horizontal: Vec<Box<[E]>>,
    ) -> Self {
        let s = Self {
            nodes,
            vertical,
            horizontal,
            pd: PhantomData,
        };
        debug_assert!(s.check_gen());
        s
    }

    pub fn new(v: usize, h: usize) -> Self
    where
        N: Default,
        E: Default,
    {
        Self::new_with(v, h, |_, _| N::default(), |_, _, _| E::default())
    }

    pub fn new_with<FN, FE>(v: usize, h: usize, mut fnode: FN, mut fedge: FE) -> Self
    where
        FN: FnMut(usize, usize) -> N,
        FE: FnMut(usize, usize, Axis) -> E,
    {
        let mut nodes = Vec::with_capacity(v);
        let mut vertical = Vec::with_capacity(v - 1);
        let mut horizontal = Vec::with_capacity(v);

        for vi in 0..v - 1 {
            let mut nv = Vec::with_capacity(h);
            let mut vv = Vec::with_capacity(h);
            let mut hv = Vec::with_capacity(h - 1);
            for hi in 0..h - 1 {
                nv.push(fnode(vi, hi));
                vv.push(fedge(vi, hi, Axis::Vertical));
                hv.push(fedge(vi, hi, Axis::Horizontal));
            }
            nv.push(fnode(vi, h - 1));
            vv.push(fedge(vi, h - 1, Axis::Vertical));
            nodes.push(nv.into_boxed_slice());
            vertical.push(vv.into_boxed_slice());
            horizontal.push(hv.into_boxed_slice());
        }
        let mut nv = Vec::with_capacity(h);
        let mut hv = Vec::with_capacity(h - 1);
        for hi in 0..h - 1 {
            nv.push(fnode(v - 1, hi));
            hv.push(fedge(v - 1, hi, Axis::Horizontal));
        }
        nv.push(fnode(v - 1, h - 1));
        nodes.push(nv.into_boxed_slice());
        horizontal.push(hv.into_boxed_slice());
        Self::new_raw(nodes, vertical, horizontal)
    }

    pub fn vertical_node_count(&self) -> usize {
        self.nodes.len()
    }
    pub fn horizontal_node_count(&self) -> usize {
        self.nodes.get(0).map(|x| x.len()).unwrap_or(0)
    }

    fn check_gen(&self) -> bool {
        let v = self.vertical_node_count();
        let h = self.horizontal_node_count();
        self.nodes.iter().all(|x| x.len() == h)
            && self.vertical.len() == v - 1
            && self.vertical.iter().all(|x| x.len() == h)
            && self.horizontal.len() == v
            && self.horizontal.iter().all(|x| x.len() == h - 1)
    }

    /// Get a reference to the square graph's nodes.
    pub fn nodes(&self) -> &[Box<[N]>] {
        &self.nodes
    }

    /// Get a mutable reference to the square graph's vertical.
    pub fn vertical(&self) -> &[Box<[E]>] {
        &self.vertical
    }

    /// Get a mutable reference to the square graph's horizontal.
    pub fn horizontal(&self) -> &[Box<[E]>] {
        &self.horizontal
    }

    /// Get a reference to the square graph's nodes.
    pub fn nodes_mut(&mut self) -> &mut [Box<[N]>] {
        &mut self.nodes
    }

    /// Get a mutable reference to the square graph's vertical.
    pub fn vertical_mut(&mut self) -> &mut [Box<[E]>] {
        &mut self.vertical
    }

    /// Get a mutable reference to the square graph's horizontal.
    pub fn horizontal_mut(&mut self) -> &mut [Box<[E]>] {
        &mut self.horizontal
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeIndex<Ix: IndexType> {
    pub vertical: Ix,
    pub horizontal: Ix,
}

impl<Ix: IndexType> NodeIndex<Ix> {
    pub fn new(vertical: Ix, horizontal: Ix) -> Self {
        Self {
            vertical,
            horizontal,
        }
    }

    /// manhattan distance
    pub fn distance<T: Into<(usize, usize)>>(&self, target: T) -> usize {
        let target: (usize, usize) = target.into();
        (self.vertical.index() as isize - target.0 as isize).abs() as usize
            + (self.horizontal.index() as isize - target.1 as isize).abs() as usize
    }
}

impl<Ix: IndexType> PartialEq<(usize, usize)> for NodeIndex<Ix> {
    fn eq(&self, value: &(usize, usize)) -> bool {
        &(self.vertical.index(), self.horizontal.index()) == value
    }
}

impl<Ix: IndexType> From<(usize, usize)> for NodeIndex<Ix> {
    fn from(value: (usize, usize)) -> Self {
        NodeIndex::new(Ix::new(value.0), Ix::new(value.1))
    }
}

impl<Ix: IndexType> From<NodeIndex<Ix>> for (usize, usize) {
    fn from(value: NodeIndex<Ix>) -> Self {
        (value.vertical.index(), value.horizontal.index())
    }
}

impl<N, E, Ix> GraphBase for SquareGraph<N, E, Ix>
where
    Ix: IndexType,
{
    type NodeId = NodeIndex<Ix>;
    type EdgeId = (NodeIndex<Ix>, Axis);
}

impl<N, E, Ix> Data for SquareGraph<N, E, Ix>
where
    Ix: IndexType,
{
    type NodeWeight = N;
    type EdgeWeight = E;
}

impl<N, E, Ix> DataMap for SquareGraph<N, E, Ix>
where
    Ix: IndexType,
{
    fn node_weight(self: &Self, id: Self::NodeId) -> Option<&Self::NodeWeight> {
        self.nodes
            .get(id.vertical.index())?
            .get(id.horizontal.index())
    }

    fn edge_weight(self: &Self, id: Self::EdgeId) -> Option<&Self::EdgeWeight> {
        match id.1 {
            Axis::Vertical => &self.vertical,
            Axis::Horizontal => &self.horizontal,
        }
        .get(id.0.vertical.index())?
        .get(id.0.horizontal.index())
    }
}

impl<N, E, Ix> DataMapMut for SquareGraph<N, E, Ix>
where
    Ix: IndexType,
{
    fn node_weight_mut(self: &mut Self, id: Self::NodeId) -> Option<&mut Self::NodeWeight> {
        self.nodes
            .get_mut(id.vertical.index())?
            .get_mut(id.horizontal.index())
    }

    fn edge_weight_mut(self: &mut Self, id: Self::EdgeId) -> Option<&mut Self::EdgeWeight> {
        match id.1 {
            Axis::Vertical => &mut self.vertical,
            Axis::Horizontal => &mut self.horizontal,
        }
        .get_mut(id.0.vertical.index())?
        .get_mut(id.0.horizontal.index())
    }
}

impl<N, E, Ix> GraphProp for SquareGraph<N, E, Ix>
where
    Ix: IndexType,
{
    type EdgeType = Undirected;
}
impl<'a, N, E, Ix> IntoEdgeReferences for &'a SquareGraph<N, E, Ix>
where
    Ix: IndexType,
    E: Copy,
    Range<Ix>: Iterator<Item = Ix>,
{
    type EdgeRef = EdgeReference<'a, E, Ix>;
    type EdgeReferences = EdgeReferences<'a, E, Ix>;

    fn edge_references(self) -> Self::EdgeReferences {
        EdgeReferences::new(&self)
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct EdgeReference<'a, E, Ix: IndexType>((NodeIndex<Ix>, Axis), &'a E, bool);

impl<'a, E, Ix: IndexType> EdgeReference<'a, E, Ix> {
    fn get_node(&self, is_source: bool) -> NodeIndex<Ix> {
        if is_source {
            self.0 .0
        } else {
            match self.0 .1 {
                Axis::Vertical => NodeIndex::new(
                    Ix::new(self.0 .0.vertical.index() + 1),
                    self.0 .0.horizontal,
                ),
                Axis::Horizontal => NodeIndex::new(
                    self.0 .0.vertical,
                    Ix::new(self.0 .0.horizontal.index() + 1),
                ),
            }
        }
    }
}
impl<'a, E: Copy, Ix: IndexType> EdgeRef for EdgeReference<'a, E, Ix> {
    type NodeId = NodeIndex<Ix>;
    type EdgeId = (NodeIndex<Ix>, Axis);
    type Weight = E;

    fn source(&self) -> Self::NodeId {
        self.get_node(self.2)
    }

    fn target(&self) -> Self::NodeId {
        self.get_node(!self.2)
    }

    fn weight(&self) -> &Self::Weight {
        self.1
    }

    fn id(&self) -> Self::EdgeId {
        self.0
    }
}
#[derive(Clone, Debug)]
pub struct EdgeReferences<'a, E, Ix: IndexType> {
    vertical: &'a Vec<Box<[E]>>,
    horizontal: &'a Vec<Box<[E]>>,
    nodes: NodeIndices<Ix>,
    prv: Option<(NodeIndex<Ix>, Axis)>,
}

impl<'a, E, Ix: IndexType> EdgeReferences<'a, E, Ix> {
    fn new<N>(graph: &'a SquareGraph<N, E, Ix>) -> Self {
        Self {
            vertical: &graph.vertical,
            horizontal: &graph.horizontal,
            nodes: NodeIndices::new(graph.vertical_node_count(), graph.horizontal_node_count()),
            prv: None,
        }
    }
}

impl<'a, E, Ix> Iterator for EdgeReferences<'a, E, Ix>
where
    Ix: IndexType,
    Range<Ix>: Iterator<Item = Ix>,
{
    type Item = EdgeReference<'a, E, Ix>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(ref mut e) = self.prv {
                if e.1 == Axis::Vertical {
                    let item = self.horizontal[e.0.vertical.index()].get(e.0.horizontal.index());
                    if let Some(item) = item {
                        e.1 = Axis::Horizontal;
                        return Some(EdgeReference(*e, item, true));
                    }
                }
            }
            if let Some(next) = self.nodes.next() {
                let item = self
                    .vertical
                    .get(next.vertical.index())
                    .map(|x| x.get(next.horizontal.index()))
                    .flatten();
                let e = (
                    NodeIndex::new(next.vertical, next.horizontal),
                    Axis::Vertical,
                );
                self.prv = Some(e);
                if let Some(item) = item {
                    return Some(EdgeReference(e, item, true));
                }
            } else {
                return None;
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lo, hi) = self.nodes.size_hint();
        (lo - self.vertical.len() - self.horizontal.len(), hi)
    }
}

impl<'a, N, E, Ix> IntoEdges for &'a SquareGraph<N, E, Ix>
where
    Ix: IndexType,
    E: Copy,
    Range<Ix>: Iterator<Item = Ix>,
{
    type Edges = std::vec::IntoIter<EdgeReference<'a, E, Ix>>;

    fn edges(self, a: Self::NodeId) -> Self::Edges {
        let v = self.vertical_node_count();
        let h = self.horizontal_node_count();
        let va = a.vertical.index();
        let ha = a.horizontal.index();
        let mut vec = Vec::new();
        if va != 0 {
            vec.push(EdgeReference(
                (
                    NodeIndex::new(Ix::new(va - 1), a.horizontal),
                    Axis::Vertical,
                ),
                &self.vertical[va - 1][ha],
                false,
            ));
        }
        if va < v - 1 {
            vec.push(EdgeReference(
                (a, Axis::Vertical),
                &self.vertical[va][ha],
                true,
            ));
        }
        if ha != 0 {
            vec.push(EdgeReference(
                (
                    NodeIndex::new(a.vertical, Ix::new(ha - 1)),
                    Axis::Horizontal,
                ),
                &self.horizontal[va][ha - 1],
                false,
            ));
        }
        if ha < h - 1 {
            vec.push(EdgeReference(
                (a, Axis::Horizontal),
                &self.horizontal[va][ha],
                true,
            ));
        }
        vec.into_iter()
    }
}

impl<'a, N, E, Ix> IntoNeighbors for &'a SquareGraph<N, E, Ix>
where
    Ix: IndexType,
{
    type Neighbors = std::vec::IntoIter<NodeIndex<Ix>>;

    fn neighbors(self: Self, a: Self::NodeId) -> Self::Neighbors {
        let v = self.vertical_node_count();
        let h = self.horizontal_node_count();
        let va = a.vertical.index();
        let ha = a.horizontal.index();
        let mut vec = Vec::new();
        if va != 0 {
            vec.push(NodeIndex::new(Ix::new(va - 1), a.horizontal));
        }
        if va < v - 1 {
            vec.push(NodeIndex::new(Ix::new(va + 1), a.horizontal));
        }
        if ha != 0 {
            vec.push(NodeIndex::new(a.vertical, Ix::new(ha - 1)));
        }
        if ha < h - 1 {
            vec.push(NodeIndex::new(a.vertical, Ix::new(ha + 1)));
        }
        vec.into_iter()
    }
}

impl<'a, N, E, Ix> IntoNodeIdentifiers for &'a SquareGraph<N, E, Ix>
where
    Ix: IndexType,
    Range<Ix>: Iterator<Item = Ix>,
{
    type NodeIdentifiers = NodeIndices<Ix>;

    fn node_identifiers(self) -> Self::NodeIdentifiers {
        NodeIndices::new(self.vertical_node_count(), self.horizontal_node_count())
    }
}

#[derive(Clone, Debug)]
pub struct NodeIndices<Ix> {
    p: itertools::Product<Range<usize>, Range<usize>>,
    pd: PhantomData<Ix>,
}

impl<Ix> NodeIndices<Ix> {
    fn new(v: usize, h: usize) -> Self {
        Self {
            p: (0..v).cartesian_product(0..h),
            pd: PhantomData,
        }
    }
}

impl<Ix: IndexType> Iterator for NodeIndices<Ix>
where
    Range<Ix>: Iterator<Item = Ix>,
{
    type Item = NodeIndex<Ix>;

    fn next(&mut self) -> Option<Self::Item> {
        self.p.next().map(|x| (x.0, x.1).into())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.p.size_hint()
    }

    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.p.fold(init, |x, item| (&mut f)(x, item.into()))
    }
}

impl<Ix: IndexType> FusedIterator for NodeIndices<Ix> where Range<Ix>: Iterator<Item = Ix> {}

impl<'a, N: Clone, E, Ix> IntoNodeReferences for &'a SquareGraph<N, E, Ix>
where
    Ix: IndexType,
    Range<Ix>: Iterator<Item = Ix>,
{
    type NodeRef = (NodeIndex<Ix>, &'a N);
    type NodeReferences = NodeReferences<'a, N, Ix>;

    fn node_references(self) -> Self::NodeReferences {
        NodeReferences {
            indices: self.node_identifiers(),
            nodes: &self.nodes,
        }
    }
}

pub struct NodeReferences<'a, N, Ix> {
    indices: NodeIndices<Ix>,
    nodes: &'a Vec<Box<[N]>>,
}

impl<'a, N, Ix> Iterator for NodeReferences<'a, N, Ix>
where
    Ix: IndexType,
    Range<Ix>: Iterator<Item = Ix>,
{
    type Item = (NodeIndex<Ix>, &'a N);

    fn next(&mut self) -> Option<Self::Item> {
        self.indices.next().map(|x| {
            (x, unsafe {
                self.nodes
                    .get_unchecked(x.vertical.index())
                    .get_unchecked(x.horizontal.index())
            })
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.indices.size_hint()
    }
}

impl<N, E, Ix> NodeCompactIndexable for SquareGraph<N, E, Ix> where Ix: IndexType {}

impl<N, E, Ix> NodeCount for SquareGraph<N, E, Ix>
where
    Ix: IndexType,
{
    fn node_count(self: &Self) -> usize {
        self.vertical_node_count() * self.horizontal_node_count()
    }
}

impl<N, E, Ix> NodeIndexable for SquareGraph<N, E, Ix>
where
    Ix: IndexType,
{
    fn node_bound(self: &Self) -> usize {
        self.horizontal_node_count() * self.vertical_node_count()
    }

    fn to_index(self: &Self, a: Self::NodeId) -> usize {
        a.vertical.index() * self.horizontal_node_count() + a.horizontal.index()
    }

    fn from_index(self: &Self, i: usize) -> Self::NodeId {
        let h = self.horizontal_node_count();
        (i / h, i % h).into()
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct VisMap {
    v: Vec<FixedBitSet>,
}

impl VisMap {
    pub fn new(v: usize, h: usize) -> Self {
        let mut vec = Vec::with_capacity(v);
        for _ in 0..v {
            vec.push(FixedBitSet::with_capacity(h));
        }
        Self { v: vec }
    }
}

impl<Ix: IndexType> VisitMap<NodeIndex<Ix>> for VisMap {
    fn visit(&mut self, a: NodeIndex<Ix>) -> bool {
        !self.v[a.vertical.index()].put(a.horizontal.index())
    }

    fn is_visited(&self, a: &NodeIndex<Ix>) -> bool {
        self.v[a.vertical.index()].contains(a.horizontal.index())
    }
}

impl<N, E, Ix> Visitable for SquareGraph<N, E, Ix>
where
    Ix: IndexType,
{
    type Map = VisMap;

    fn visit_map(self: &Self) -> Self::Map {
        VisMap::new(self.vertical_node_count(), self.horizontal_node_count())
    }

    fn reset_map(self: &Self, map: &mut Self::Map) {
        map.v.iter_mut().for_each(|x| x.clear())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gen() {
        let sq = SquareGraph::<_, _, u32>::new_with(
            4,
            3,
            |x, y| x + 2 * y,
            |x, y, d| (x + 2 * y) as i32 * (if d.is_vertical() { 1 } else { -1 }),
        );
        assert_eq!(sq.vertical_node_count(), 4);
        assert_eq!(sq.horizontal_node_count(), 3);
        assert_eq!(sq.node_weight((0, 0).into()), Some(&0));
        assert_eq!(sq.node_weight((3, 0).into()), Some(&3));
        assert_eq!(sq.node_weight((4, 0).into()), None);
        assert_eq!(sq.node_weight((0, 2).into()), Some(&4));
        assert_eq!(sq.node_weight((0, 3).into()), None);
        assert_eq!(sq.edge_weight(((0, 0).into(), Axis::Vertical)), Some(&0));
        assert_eq!(sq.edge_weight(((0, 2).into(), Axis::Vertical)), Some(&4));
        assert_eq!(sq.edge_weight(((0, 2).into(), Axis::Horizontal)), None);
        assert_eq!(sq.edge_weight(((3, 0).into(), Axis::Vertical)), None);
        assert_eq!(sq.edge_weight(((3, 0).into(), Axis::Horizontal)), Some(&-3));
    }

    #[test]
    fn node_identifiers() {
        let sq = SquareGraph::<_, _, u32>::new_with(
            5,
            3,
            |x, y| x + 2 * y,
            |x, y, d| (x + 2 * y) as i32 * (if d.is_vertical() { 1 } else { -1 }),
        );
        for (i, x) in sq.node_identifiers().enumerate() {
            let x = x;
            let x2 = sq.to_index(x);
            assert_eq!(x2, i);
            let x3 = sq.from_index(x2);
            assert_eq!(x, x3);
        }
    }

    #[test]
    fn edge_references() {
        let sq = SquareGraph::<_, _, u32>::new_with(
            5,
            3,
            |x, y| x + 2 * y,
            |x, y, d| (x + 2 * y) as i32 * (if d.is_vertical() { 1 } else { -1 }),
        );

        let mut i = 0;
        let mut x = -1;
        for e in sq.edge_references().filter(|x| x.id().1 == Axis::Vertical) {
            let y = sq.to_index(e.0 .0) as i32;
            assert!(x < y);
            x = y;
            i += 1;
        }
        assert_eq!(i, 12);
    }

    #[test]
    fn astar() {
        let sq = SquareGraph::<_, _, u32>::new_with(
            4,
            3,
            |_, _| (),
            |x, y, d| (x + 2 * y) as i32 * (if d.is_vertical() { 1 } else { 3 }),
        );

        let x = petgraph::algo::astar(
            &sq,
            (0, 0).into(),
            |x| x == (2, 1),
            |e| *e.weight(),
            |x| x.distance((2, 1)) as i32,
        );
        assert!(x.is_some());
        let (d, p) = x.unwrap();
        assert_eq!(d, 5);
        assert_eq!(p, [(0, 0), (0, 1), (1, 1), (2, 1)]);

        let x = petgraph::algo::astar(
            &sq,
            (2, 1).into(),
            |x| x == (0, 0),
            |e| *e.weight(),
            |x| x.distance((0, 0)) as i32,
        );
        assert!(x.is_some());
        let (d, p) = x.unwrap();
        assert_eq!(d, 5);
        assert_eq!(p, [(2, 1), (1, 1), (0, 1), (0, 0)])
    }
}
