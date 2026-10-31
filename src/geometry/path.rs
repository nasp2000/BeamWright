use serde::{Deserialize, Serialize};

use super::{Point2, bezier::CubicBezier};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Segment {
    Line { from: Point2, to: Point2 },
    Cubic(CubicBezier),
}

/// One editable node of a path: its point and the Bézier handles attached to
/// it. Handles are absolute positions, as in the segment data.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Node {
    pub point: Point2,
    pub handle_in: Option<Point2>,
    pub handle_out: Option<Point2>,
}

impl Node {
    pub fn new(point: Point2) -> Self {
        Self {
            point,
            handle_in: None,
            handle_out: None,
        }
    }

    /// Whether the node has a handle on both sides.
    pub fn is_smooth(&self) -> bool {
        self.handle_in.is_some() && self.handle_out.is_some()
    }
}

impl Segment {
    pub fn start(&self) -> Point2 {
        match self {
            Segment::Line { from, .. } => *from,
            Segment::Cubic(cubic) => cubic.p0,
        }
    }

    pub fn end(&self) -> Point2 {
        match self {
            Segment::Line { to, .. } => *to,
            Segment::Cubic(cubic) => cubic.p3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Path {
    pub segments: Vec<Segment>,
    pub closed: bool,
}

impl Path {
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
            closed: false,
        }
    }
    /// Build a path from a list of points (connected by lines).
    pub fn from_points(points: &[(f64, f64)], closed: bool) -> Self {
        let mut p = Path::new();
        p.closed = closed;
        for i in 1..points.len() {
            p.push(Segment::Line {
                from: Point2::new(points[i - 1].0, points[i - 1].1),
                to: Point2::new(points[i].0, points[i].1),
            });
        }
        if closed && points.len() >= 2 {
            p.push(Segment::Line {
                from: Point2::new(points[points.len() - 1].0, points[points.len() - 1].1),
                to: Point2::new(points[0].0, points[0].1),
            });
        }
        p
    }
    /// Extract endpoints of line segments.
    pub fn to_points(&self) -> Vec<(f64, f64)> {
        let mut pts = Vec::new();
        for seg in &self.segments {
            match seg {
                Segment::Line { from, .. } => {
                    pts.push((from.x, from.y));
                }
                Segment::Cubic(c) => {
                    pts.push((c.p0.x, c.p0.y));
                }
            }
        }
        if let Some(last) = self.segments.last() {
            match last {
                Segment::Line { to, .. } => pts.push((to.x, to.y)),
                Segment::Cubic(c) => pts.push((c.p3.x, c.p3.y)),
            }
        }
        pts
    }
    pub fn push(&mut self, seg: Segment) {
        self.segments.push(seg);
    }
    pub fn append(&mut self, other: &Path) {
        self.segments.extend(other.segments.clone());
        if other.closed {
            self.closed = true;
        }
    }
    pub fn pop(&mut self) -> Option<Segment> {
        self.segments.pop()
    }
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
    /// Reverse the direction of travel.
    ///
    /// Reversing only the order of the segments would leave every segment
    /// pointing the wrong way, so each one is flipped as well.
    pub fn reverse(&mut self) {
        self.segments.reverse();
        for segment in &mut self.segments {
            *segment = match *segment {
                Segment::Line { from, to } => Segment::Line { from: to, to: from },
                Segment::Cubic(c) => Segment::Cubic(CubicBezier {
                    p0: c.p3,
                    p1: c.p2,
                    p2: c.p1,
                    p3: c.p0,
                }),
            };
        }
    }
    pub fn close(&mut self) {
        self.closed = true;
    }
    pub fn last_point(&self) -> Option<Point2> {
        self.segments.last().map(|s| match s {
            Segment::Line { to, .. } => *to,
            Segment::Cubic(c) => c.end(),
        })
    }
    /// Bounding box of the path.
    pub fn bounding_box(&self) -> Option<(Point2, Point2)> {
        if self.segments.is_empty() {
            return None;
        }
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for seg in &self.segments {
            match seg {
                Segment::Line { from, to } => {
                    for p in [from, to] {
                        min_x = min_x.min(p.x);
                        min_y = min_y.min(p.y);
                        max_x = max_x.max(p.x);
                        max_y = max_y.max(p.y);
                    }
                }
                Segment::Cubic(c) => {
                    let (mn, mx) = c.bounding_box();
                    min_x = min_x.min(mn.x);
                    min_y = min_y.min(mn.y);
                    max_x = max_x.max(mx.x);
                    max_y = max_y.max(mx.y);
                }
            }
        }
        Some((Point2::new(min_x, min_y), Point2::new(max_x, max_y)))
    }
    /// Flatten the path into a polyline. `tolerance` is in document units and
    /// controls how many samples each cubic segment receives.
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    pub fn flatten(&self, tolerance: f64) -> Vec<Point2> {
        let tolerance = tolerance.max(0.001);
        let mut points = Vec::new();
        for seg in &self.segments {
            match seg {
                Segment::Line { from, to } => {
                    if points.is_empty() {
                        points.push(*from);
                    }
                    points.push(*to);
                }
                Segment::Cubic(c) => {
                    if points.is_empty() {
                        points.push(c.p0);
                    }
                    let steps = ((c.approx_length(16) / tolerance).ceil() as usize).clamp(4, 512);
                    for i in 1..=steps {
                        let t = i as f64 / steps as f64;
                        points.push(c.eval(t));
                    }
                }
            }
        }
        if self.closed && points.len() > 1 {
            let first = points[0];
            let last = *points.last().unwrap_or(&first);
            if first.distance(last) > 1e-9 {
                points.push(first);
            }
        }
        points
    }
    /// Editable nodes of the path.
    ///
    /// A closed path that repeats its first point at the end (the usual
    /// representation after drawing or importing) yields one node per distinct
    /// point: the repeated point is not listed twice. Handles are read from the
    /// adjacent cubic segments.
    pub fn nodes(&self) -> Vec<Node> {
        if self.segments.is_empty() {
            return Vec::new();
        }
        let mut points = Vec::with_capacity(self.segments.len() + 1);
        points.push(self.segments[0].start());
        for segment in &self.segments {
            points.push(segment.end());
        }
        let mut handle_in: Vec<Option<Point2>> = vec![None; points.len()];
        let mut handle_out: Vec<Option<Point2>> = vec![None; points.len()];
        for (index, segment) in self.segments.iter().enumerate() {
            if let Segment::Cubic(cubic) = segment {
                handle_out[index] = Some(cubic.p1);
                let end = if index + 1 < points.len() {
                    index + 1
                } else {
                    0
                };
                handle_in[end] = Some(cubic.p2);
            }
        }
        let mut nodes: Vec<Node> = points
            .iter()
            .enumerate()
            .map(|(index, point)| Node {
                point: *point,
                handle_in: handle_in[index],
                handle_out: handle_out[index],
            })
            .collect();
        let repeats_start = self.closed
            && nodes.len() > 2
            && nodes[0].point.distance(nodes[nodes.len() - 1].point) <= 1e-9;
        if repeats_start {
            let duplicate = nodes.pop().expect("path has nodes");
            nodes[0].handle_in = duplicate.handle_in;
        }
        nodes
    }

    /// Build a path from editable nodes.
    ///
    /// A segment is a cubic when either of its endpoints has a handle on that
    /// side; missing handles fall back to the one-third points, so a single
    /// handle still produces a smooth curve.
    pub fn from_nodes(nodes: &[Node], closed: bool) -> Path {
        let mut path = Path::new();
        if nodes.len() < 2 {
            return path;
        }
        let count = if closed { nodes.len() } else { nodes.len() - 1 };
        for index in 0..count {
            let from = nodes[index];
            let to = nodes[(index + 1) % nodes.len()];
            let default_out = Point2::new(
                from.point.x + (to.point.x - from.point.x) / 3.0,
                from.point.y + (to.point.y - from.point.y) / 3.0,
            );
            let default_in = Point2::new(
                from.point.x + (to.point.x - from.point.x) * 2.0 / 3.0,
                from.point.y + (to.point.y - from.point.y) * 2.0 / 3.0,
            );
            match (from.handle_out, to.handle_in) {
                (None, None) => path.push(Segment::Line {
                    from: from.point,
                    to: to.point,
                }),
                (handle_out, handle_in) => path.push(Segment::Cubic(CubicBezier {
                    p0: from.point,
                    p1: handle_out.unwrap_or(default_out),
                    p2: handle_in.unwrap_or(default_in),
                    p3: to.point,
                })),
            }
        }
        path.closed = closed;
        path
    }

    /// Move one node and its handles by `(dx, dy)`.
    pub fn move_node(&mut self, index: usize, dx: f64, dy: f64) -> bool {
        let mut nodes = self.nodes();
        let Some(node) = nodes.get_mut(index) else {
            return false;
        };
        node.point = Point2::new(node.point.x + dx, node.point.y + dy);
        for handle in [&mut node.handle_in, &mut node.handle_out]
            .into_iter()
            .flatten()
        {
            *handle = Point2::new(handle.x + dx, handle.y + dy);
        }
        *self = Path::from_nodes(&nodes, self.closed);
        true
    }

    /// Move one node to an absolute position, carrying its handles.
    pub fn set_node_point(&mut self, index: usize, point: Point2) -> bool {
        let nodes = self.nodes();
        let Some(node) = nodes.get(index) else {
            return false;
        };
        self.move_node(index, point.x - node.point.x, point.y - node.point.y)
    }

    /// Set (or clear) the handles of one node.
    pub fn set_node_handles(
        &mut self,
        index: usize,
        handle_in: Option<Point2>,
        handle_out: Option<Point2>,
    ) -> bool {
        let mut nodes = self.nodes();
        let Some(node) = nodes.get_mut(index) else {
            return false;
        };
        node.handle_in = handle_in;
        node.handle_out = handle_out;
        *self = Path::from_nodes(&nodes, self.closed);
        true
    }

    /// Make the segment starting at `index` straight.
    ///
    /// Both ends of a segment must be corners for it to be a line, so the
    /// outgoing handle of the start node and the incoming handle of the end
    /// node are cleared together.
    pub fn clear_segment(&mut self, index: usize) -> bool {
        let mut nodes = self.nodes();
        if nodes.len() < 2 {
            return false;
        }
        let count = if self.closed {
            nodes.len()
        } else {
            nodes.len() - 1
        };
        if index >= count {
            return false;
        }
        let next = (index + 1) % nodes.len();
        nodes[index].handle_out = None;
        nodes[next].handle_in = None;
        *self = Path::from_nodes(&nodes, self.closed);
        true
    }

    /// Convert one node to a corner (no handles) or a smooth curve.
    ///
    /// A smooth node gets handles along the line through its neighbours, one
    /// third of the shorter distance on each side.
    pub fn convert_node(&mut self, index: usize, smooth: bool) -> bool {
        let mut nodes = self.nodes();
        if index >= nodes.len() {
            return false;
        }
        if smooth {
            let count = nodes.len();
            let previous = if index > 0 {
                nodes[index - 1].point
            } else if self.closed {
                nodes[count - 1].point
            } else {
                nodes[index].point
            };
            let next = if index + 1 < count {
                nodes[index + 1].point
            } else if self.closed {
                nodes[0].point
            } else {
                nodes[index].point
            };
            let direction = Point2::new(next.x - previous.x, next.y - previous.y);
            let length = (direction.x * direction.x + direction.y * direction.y).sqrt();
            if length <= 1e-9 {
                return false;
            }
            let unit = Point2::new(direction.x / length, direction.y / length);
            let reach = (nodes[index].point.distance(previous))
                .min(nodes[index].point.distance(next))
                / 3.0;
            nodes[index].handle_in = Some(Point2::new(
                nodes[index].point.x - unit.x * reach,
                nodes[index].point.y - unit.y * reach,
            ));
            nodes[index].handle_out = Some(Point2::new(
                nodes[index].point.x + unit.x * reach,
                nodes[index].point.y + unit.y * reach,
            ));
        } else {
            nodes[index].handle_in = None;
            nodes[index].handle_out = None;
            // A segment is straight only when both of its ends are corners, so
            // clear the facing handle of each neighbour too.
            let count = nodes.len();
            let previous = if index > 0 {
                index - 1
            } else if self.closed {
                count - 1
            } else {
                index
            };
            let next = if index + 1 < count {
                index + 1
            } else if self.closed {
                0
            } else {
                index
            };
            nodes[previous].handle_out = None;
            nodes[next].handle_in = None;
        }
        *self = Path::from_nodes(&nodes, self.closed);
        true
    }

    /// Insert a node at parameter `t` along one segment and return its index.
    ///
    /// The segment index is the node index it starts from, so the new node is
    /// `segment + 1`.
    pub fn insert_node(&mut self, segment: usize, t: f64) -> Option<usize> {
        let nodes = self.nodes();
        if nodes.len() < 2 {
            return None;
        }
        let count = if self.closed {
            nodes.len()
        } else {
            nodes.len() - 1
        };
        if segment >= count {
            return None;
        }
        let t = t.clamp(0.001, 0.999);
        let from_index = segment;
        let to_index = (segment + 1) % nodes.len();
        let from = nodes[from_index];
        let to = nodes[to_index];
        let mut updated = nodes.clone();
        let new_node = match (from.handle_out, to.handle_in) {
            (None, None) => {
                let point = Point2::new(
                    from.point.x + (to.point.x - from.point.x) * t,
                    from.point.y + (to.point.y - from.point.y) * t,
                );
                Node::new(point)
            }
            (handle_out, handle_in) => {
                let default_out = Point2::new(
                    from.point.x + (to.point.x - from.point.x) / 3.0,
                    from.point.y + (to.point.y - from.point.y) / 3.0,
                );
                let default_in = Point2::new(
                    from.point.x + (to.point.x - from.point.x) * 2.0 / 3.0,
                    from.point.y + (to.point.y - from.point.y) * 2.0 / 3.0,
                );
                let cubic = CubicBezier {
                    p0: from.point,
                    p1: handle_out.unwrap_or(default_out),
                    p2: handle_in.unwrap_or(default_in),
                    p3: to.point,
                };
                let (left, right) = cubic.split(t);
                updated[from_index].handle_out = Some(left.p1);
                updated[to_index].handle_in = Some(right.p2);
                Node {
                    point: left.p3,
                    handle_in: Some(left.p2),
                    handle_out: Some(right.p1),
                }
            }
        };
        updated.insert(segment + 1, new_node);
        *self = Path::from_nodes(&updated, self.closed);
        Some(segment + 1)
    }

    /// Remove one node, connecting its neighbours with a straight segment.
    pub fn remove_node(&mut self, index: usize) -> bool {
        let mut nodes = self.nodes();
        if index >= nodes.len() || nodes.len() <= 2 {
            return false;
        }
        nodes.remove(index);
        if nodes.len() < 2 {
            return false;
        }
        *self = Path::from_nodes(&nodes, self.closed);
        true
    }

    /// Replace the path with a simplified polyline within `tolerance`.
    ///
    /// Curves are flattened first, so a simplified path is made of straight
    /// segments; the tolerance bounds how far the result may stray.
    pub fn simplified(&self, tolerance: f64) -> Path {
        let points: Vec<(f64, f64)> = self
            .flatten(0.1)
            .into_iter()
            .map(|point| (point.x, point.y))
            .collect();
        let simplified = crate::geometry::polygon::simplify(&points, tolerance);
        Path::from_points(&simplified, self.closed)
    }

    /// Open a closed path at one node, which becomes both ends.
    pub fn open_at_node(&mut self, index: usize) -> bool {
        let nodes = self.nodes();
        if !self.closed || index >= nodes.len() || nodes.len() < 3 {
            return false;
        }
        let mut opened: Vec<Node> = nodes[index..]
            .iter()
            .chain(nodes[..index].iter())
            .copied()
            .collect();
        let start = nodes[index];
        let mut last = Node::new(start.point);
        last.handle_in = start.handle_in;
        if let Some(first) = opened.first_mut() {
            first.handle_in = None;
        }
        opened.push(last);
        *self = Path::from_nodes(&opened, false);
        true
    }

    /// Split an open path at one node into two paths that share the node.
    pub fn split_at_node(&self, index: usize) -> Option<(Path, Path)> {
        if self.closed {
            return None;
        }
        let nodes = self.nodes();
        if index == 0 || index + 1 >= nodes.len() {
            return None;
        }
        let left = Path::from_nodes(&nodes[..=index], false);
        let right = Path::from_nodes(&nodes[index..], false);
        Some((left, right))
    }

    /// Start point of the path, if any.
    pub fn start_point(&self) -> Option<Point2> {
        self.segments.first().map(|s| match s {
            Segment::Line { from, .. } => *from,
            Segment::Cubic(c) => c.p0,
        })
    }
    pub fn transform(&self, t: &crate::geometry::transform::Transform2) -> Path {
        let mut p = Path::new();
        p.closed = self.closed;
        for seg in &self.segments {
            match seg {
                Segment::Line { from, to } => {
                    p.push(Segment::Line {
                        from: t.apply(*from),
                        to: t.apply(*to),
                    });
                }
                Segment::Cubic(c) => {
                    p.push(Segment::Cubic(crate::geometry::bezier::CubicBezier {
                        p0: t.apply(c.p0),
                        p1: t.apply(c.p1),
                        p2: t.apply(c.p2),
                        p3: t.apply(c.p3),
                    }));
                }
            }
        }
        p
    }
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }
    #[allow(clippy::cast_precision_loss)]
    pub fn approximate_length(&self) -> f64 {
        self.segments
            .iter()
            .map(|s| match s {
                Segment::Line { from, to } => from.distance(*to),
                Segment::Cubic(c) => c.approx_length(20),
            })
            .sum()
    }
    /// Alias for `approximate_length`.
    pub fn length(&self) -> f64 {
        self.approximate_length()
    }
    /// Check if path is closed.
    pub fn is_closed(&self) -> bool {
        self.closed
    }
}

impl Default for Path {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty() {
        let p = Path::new();
        assert!(p.is_empty());
        assert_eq!(p.segment_count(), 0);
    }

    #[test]
    fn reverse_empty() {
        let mut p = Path::new();
        p.reverse();
        assert!(p.is_empty());
    }

    #[test]
    fn reverse_flips_segment_direction() {
        let mut p = Path::from_points(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)], false);
        p.reverse();
        assert_eq!(p.start_point().unwrap(), Point2::new(10.0, 10.0));
        assert_eq!(p.last_point().unwrap(), Point2::new(0.0, 0.0));
        let points = p.to_points();
        assert_eq!(points, vec![(10.0, 10.0), (10.0, 0.0), (0.0, 0.0)]);
    }

    #[test]
    fn reverse_keeps_cubic_shape() {
        let curve = CubicBezier {
            p0: Point2::new(0.0, 0.0),
            p1: Point2::new(1.0, 2.0),
            p2: Point2::new(3.0, 2.0),
            p3: Point2::new(4.0, 0.0),
        };
        let mut p = Path::new();
        p.push(Segment::Cubic(curve));
        p.reverse();
        let Segment::Cubic(reversed) = p.segments[0] else {
            panic!("expected a cubic segment");
        };
        assert_eq!(reversed.p0, curve.p3);
        assert_eq!(reversed.p1, curve.p2);
        assert_eq!(reversed.p2, curve.p1);
        assert_eq!(reversed.p3, curve.p0);
    }

    #[test]
    fn close_sets_flag() {
        let mut p = Path::new();
        assert!(!p.closed);
        p.close();
        assert!(p.closed);
    }

    fn cubic_path() -> Path {
        let mut path = Path::new();
        path.push(Segment::Cubic(CubicBezier {
            p0: Point2::new(0.0, 0.0),
            p1: Point2::new(2.0, 4.0),
            p2: Point2::new(8.0, 4.0),
            p3: Point2::new(10.0, 0.0),
        }));
        path
    }

    #[test]
    fn nodes_of_an_open_polyline() {
        let path = Path::from_points(&[(0.0, 0.0), (5.0, 0.0), (5.0, 5.0)], false);
        let nodes = path.nodes();
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[2].point, Point2::new(5.0, 5.0));
        assert!(nodes.iter().all(|node| !node.is_smooth()));
    }

    #[test]
    fn closed_nodes_do_not_repeat_the_start() {
        let path = Path::from_points(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)], true);
        let nodes = path.nodes();
        assert_eq!(nodes.len(), 4);
        assert_eq!(nodes[0].point, Point2::new(0.0, 0.0));
        assert_eq!(nodes[3].point, Point2::new(0.0, 10.0));
    }

    #[test]
    fn from_nodes_round_trips_lines() {
        let original = Path::from_points(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)], true);
        let rebuilt = Path::from_nodes(&original.nodes(), true);
        assert!(rebuilt.closed);
        assert_eq!(rebuilt.to_points(), original.to_points());
    }

    #[test]
    fn cubic_handles_are_read_and_written() {
        let path = cubic_path();
        let nodes = path.nodes();
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].handle_out, Some(Point2::new(2.0, 4.0)));
        assert_eq!(nodes[1].handle_in, Some(Point2::new(8.0, 4.0)));
        assert!(nodes[0].handle_in.is_none());
        let rebuilt = Path::from_nodes(&nodes, false);
        assert_eq!(rebuilt.segments, path.segments);
    }

    #[test]
    fn moving_a_node_carries_its_handles() {
        let mut path = cubic_path();
        assert!(path.move_node(0, 1.0, 2.0));
        let nodes = path.nodes();
        assert_eq!(nodes[0].point, Point2::new(1.0, 2.0));
        assert_eq!(nodes[0].handle_out, Some(Point2::new(3.0, 6.0)));
        assert!(!path.move_node(9, 1.0, 1.0));
    }

    #[test]
    fn set_node_point_positions_absolutely() {
        let mut path = cubic_path();
        assert!(path.set_node_point(1, Point2::new(20.0, 5.0)));
        let nodes = path.nodes();
        assert_eq!(nodes[1].point, Point2::new(20.0, 5.0));
        assert_eq!(nodes[1].handle_in, Some(Point2::new(18.0, 9.0)));
    }

    #[test]
    fn insert_node_splits_a_line() {
        let mut path = Path::from_points(&[(0.0, 0.0), (10.0, 0.0)], false);
        let index = path.insert_node(0, 0.5).unwrap();
        assert_eq!(index, 1);
        let nodes = path.nodes();
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[1].point, Point2::new(5.0, 0.0));
        assert_eq!(path.segments.len(), 2);
    }

    #[test]
    fn insert_node_keeps_the_cubic_shape() {
        let mut path = cubic_path();
        let original = CubicBezier {
            p0: Point2::new(0.0, 0.0),
            p1: Point2::new(2.0, 4.0),
            p2: Point2::new(8.0, 4.0),
            p3: Point2::new(10.0, 0.0),
        };
        let index = path.insert_node(0, 0.4).unwrap();
        assert_eq!(index, 1);
        let nodes = path.nodes();
        assert_eq!(nodes.len(), 3);
        assert!(nodes[1].point.distance(original.eval(0.4)) < 1e-9);
        assert_eq!(path.segments.len(), 2);
        // The two halves are exactly the two halves of the original curve.
        let (left, right) = original.split(0.4);
        let Segment::Cubic(first) = path.segments[0] else {
            panic!("expected a cubic");
        };
        let Segment::Cubic(second) = path.segments[1] else {
            panic!("expected a cubic");
        };
        assert_eq!(first, left);
        assert_eq!(second, right);
    }

    #[test]
    fn remove_node_joins_its_neighbours() {
        let mut path = Path::from_points(&[(0.0, 0.0), (5.0, 5.0), (10.0, 0.0)], false);
        assert!(path.remove_node(1));
        let nodes = path.nodes();
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].point, Point2::new(0.0, 0.0));
        assert_eq!(nodes[1].point, Point2::new(10.0, 0.0));
        assert!(!path.remove_node(0));
    }

    #[test]
    fn clear_segment_straightens_only_that_segment() {
        let mut path = Path::new();
        path.push(Segment::Cubic(CubicBezier {
            p0: Point2::new(0.0, 0.0),
            p1: Point2::new(2.0, 4.0),
            p2: Point2::new(8.0, 4.0),
            p3: Point2::new(10.0, 0.0),
        }));
        path.push(Segment::Cubic(CubicBezier {
            p0: Point2::new(10.0, 0.0),
            p1: Point2::new(12.0, -4.0),
            p2: Point2::new(18.0, -4.0),
            p3: Point2::new(20.0, 0.0),
        }));
        assert!(path.clear_segment(0));
        assert!(matches!(path.segments[0], Segment::Line { .. }));
        // The second segment keeps its own handles.
        assert!(matches!(path.segments[1], Segment::Cubic(_)));
        assert!(!path.clear_segment(5));
    }

    #[test]
    fn convert_node_between_corner_and_smooth() {
        let mut path = Path::from_points(&[(0.0, 0.0), (5.0, 5.0), (10.0, 0.0)], false);
        assert!(path.convert_node(1, true));
        let nodes = path.nodes();
        assert!(nodes[1].is_smooth());
        assert!(matches!(path.segments[0], Segment::Cubic(_)));
        assert!(path.convert_node(1, false));
        let nodes = path.nodes();
        assert!(!nodes[1].is_smooth());
        assert!(matches!(path.segments[0], Segment::Line { .. }));
    }

    #[test]
    fn open_at_node_unwraps_a_closed_path() {
        let mut path =
            Path::from_points(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)], true);
        assert!(path.open_at_node(1));
        assert!(!path.closed);
        let nodes = path.nodes();
        assert_eq!(nodes.len(), 5);
        assert_eq!(nodes[0].point, Point2::new(10.0, 0.0));
        assert_eq!(nodes[4].point, Point2::new(10.0, 0.0));
        assert!(!path.open_at_node(0));
    }

    #[test]
    fn simplified_paths_drop_collinear_nodes() {
        let path = Path::from_points(&[(0.0, 0.0), (2.0, 0.01), (4.0, 0.0), (6.0, 0.0)], false);
        let simplified = path.simplified(0.1);
        assert!(simplified.segments.len() < path.segments.len());
        assert_eq!(simplified.start_point().unwrap(), Point2::new(0.0, 0.0));
        assert_eq!(simplified.last_point().unwrap(), Point2::new(6.0, 0.0));
        assert!(!simplified.closed);
    }

    #[test]
    fn simplified_paths_keep_their_closed_flag() {
        let path = Path::from_points(
            &[(0.0, 0.0), (2.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)],
            true,
        );
        let simplified = path.simplified(0.1);
        assert!(simplified.closed);
        assert!(simplified.nodes().len() <= path.nodes().len());
        // Every original point stays within the tolerance of the result.
        let result = simplified.flatten(0.05);
        for point in path.flatten(0.05) {
            let distances = result.windows(2).map(|pair| {
                crate::geometry::intersection::point_segment_distance(
                    point.x, point.y, pair[0].x, pair[0].y, pair[1].x, pair[1].y,
                )
            });
            let nearest = distances.fold(f64::INFINITY, f64::min);
            assert!(nearest <= 0.1 + 1e-9, "{nearest}");
        }
    }

    #[test]
    fn split_at_node_gives_two_paths() {
        let path = Path::from_points(&[(0.0, 0.0), (5.0, 5.0), (10.0, 0.0)], false);
        let (left, right) = path.split_at_node(1).unwrap();
        assert_eq!(left.nodes().len(), 2);
        assert_eq!(right.nodes().len(), 2);
        assert_eq!(left.last_point().unwrap(), Point2::new(5.0, 5.0));
        assert_eq!(right.start_point().unwrap(), Point2::new(5.0, 5.0));
        assert!(path.split_at_node(0).is_none());
    }

    #[test]
    fn last_point() {
        let mut p = Path::new();
        assert!(p.last_point().is_none());
        p.push(Segment::Line {
            from: Point2::new(0.0, 0.0),
            to: Point2::new(1.0, 1.0),
        });
        let lp = p.last_point().unwrap();
        assert!((lp.x - 1.0).abs() < 1e-9);
        assert!((lp.y - 1.0).abs() < 1e-9);
    }

    #[test]
    fn bounding_box_path() {
        let mut p = Path::new();
        p.push(Segment::Line {
            from: Point2::new(0.0, 0.0),
            to: Point2::new(10.0, 5.0),
        });
        let (mn, mx) = p.bounding_box().unwrap();
        assert!((mn.x - 0.0).abs() < 1e-9);
        assert!((mx.x - 10.0).abs() < 1e-9);
    }

    #[test]
    fn bounding_box_empty() {
        let p = Path::new();
        assert!(p.bounding_box().is_none());
    }

    #[test]
    fn length_alias() {
        let mut p = Path::new();
        p.push(Segment::Line {
            from: Point2::new(0.0, 0.0),
            to: Point2::new(3.0, 4.0),
        });
        assert!((p.length() - 5.0).abs() < 1e-9);
        assert!((p.approximate_length() - 5.0).abs() < 1e-9);
    }

    #[test]
    fn is_closed() {
        let mut p = Path::new();
        assert!(!p.is_closed());
        p.close();
        assert!(p.is_closed());
    }

    #[test]
    fn transform_path() {
        let mut p = Path::new();
        p.push(Segment::Line {
            from: Point2::new(0.0, 0.0),
            to: Point2::new(1.0, 0.0),
        });
        let t = crate::geometry::transform::Transform2::translate(5.0, 3.0);
        let r = p.transform(&t);
        match &r.segments[0] {
            Segment::Line { from, to } => {
                assert!((from.x - 5.0).abs() < 1e-9);
                assert!((to.x - 6.0).abs() < 1e-9);
            }
            Segment::Cubic(_) => panic!("wrong segment"),
        }
    }

    #[test]
    fn from_points() {
        let pts = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)];
        let p = Path::from_points(&pts, false);
        assert_eq!(p.segment_count(), 2);
        assert!(!p.closed);
    }

    #[test]
    fn from_points_closed() {
        let pts = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)];
        let p = Path::from_points(&pts, true);
        assert_eq!(p.segment_count(), 3);
        assert!(p.closed);
    }

    #[test]
    fn to_points() {
        let mut p = Path::new();
        p.push(Segment::Line {
            from: Point2::new(0.0, 0.0),
            to: Point2::new(1.0, 0.0),
        });
        let pts = p.to_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn append() {
        let mut a = Path::new();
        a.push(Segment::Line {
            from: Point2::new(0.0, 0.0),
            to: Point2::new(1.0, 0.0),
        });
        let mut b = Path::new();
        b.push(Segment::Line {
            from: Point2::new(1.0, 0.0),
            to: Point2::new(2.0, 0.0),
        });
        a.append(&b);
        assert_eq!(a.segment_count(), 2);
    }

    #[test]
    fn pop() {
        let mut p = Path::new();
        p.push(Segment::Line {
            from: Point2::new(0.0, 0.0),
            to: Point2::new(1.0, 0.0),
        });
        let popped = p.pop();
        assert!(popped.is_some());
        assert!(p.is_empty());
    }

    #[test]
    fn push_line() {
        let mut p = Path::new();
        p.push(Segment::Line {
            from: Point2::new(0.0, 0.0),
            to: Point2::new(1.0, 0.0),
        });
        assert_eq!(p.segment_count(), 1);
        assert!(!p.is_empty());
    }

    #[test]
    fn closed_flag() {
        let mut p = Path::new();
        p.closed = true;
        assert!(p.closed);
    }
}
