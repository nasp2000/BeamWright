use crate::gcode::toolpath::{MoveKind, Toolpath};
use crate::geometry::Point2;

/// One straight movement in the simulated job.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackSegment {
    pub from: Point2,
    pub to: Point2,
    pub kind: MoveKind,
    /// Length in millimetres.
    pub length: f64,
    /// Feed rate in millimetres per minute (travel speed for rapids).
    pub speed: f64,
    /// Laser power in the writer's `S` range (0 for rapids).
    pub power: f64,
    /// One-based pass number.
    pub pass: u32,
}

/// Laser position and settings at a point in the simulation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaybackPosition {
    pub point: Point2,
    pub kind: MoveKind,
    pub power: f64,
    pub pass: u32,
}

/// Flattened, timed view of a job used by the preview window.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Playback {
    pub segments: Vec<PlaybackSegment>,
    pub total_length: f64,
    pub duration_seconds: f64,
}

impl Playback {
    /// Flatten every toolpath, repeating its moves for each pass.
    pub fn from_toolpaths(toolpaths: &[Toolpath], travel_speed_mm_min: f64) -> Self {
        let travel_speed = if travel_speed_mm_min > 0.0 {
            travel_speed_mm_min
        } else {
            3000.0
        };
        let mut segments = Vec::new();
        let mut total_length = 0.0;
        let mut duration_seconds = 0.0;
        for toolpath in toolpaths {
            let passes = toolpath.passes.max(1);
            for pass in 1..=passes {
                let mut previous: Option<Point2> = None;
                for movement in &toolpath.moves {
                    let to = Point2::new(movement.x, movement.y);
                    if let Some(from) = previous {
                        let length = from.distance(to);
                        let (speed, power) = match movement.kind {
                            MoveKind::Cut => (toolpath.speed, toolpath.power),
                            MoveKind::Travel => (travel_speed, 0.0),
                        };
                        if length > 0.0 {
                            total_length += length;
                            if speed > 0.0 {
                                duration_seconds += length / speed * 60.0;
                            }
                            segments.push(PlaybackSegment {
                                from,
                                to,
                                kind: movement.kind,
                                length,
                                speed,
                                power,
                                pass,
                            });
                        }
                    }
                    previous = Some(to);
                }
            }
        }
        Self {
            segments,
            total_length,
            duration_seconds,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// Laser state after travelling `length` millimetres of the job.
    pub fn position_at(&self, length: f64) -> Option<PlaybackPosition> {
        let length = length.clamp(0.0, self.total_length);
        let mut travelled = 0.0;
        let last = self.segments.len().saturating_sub(1);
        for (index, segment) in self.segments.iter().enumerate() {
            if length <= travelled + segment.length || index == last {
                let ratio = if segment.length > 0.0 {
                    ((length - travelled) / segment.length).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                return Some(PlaybackPosition {
                    point: Point2::new(
                        segment.from.x + (segment.to.x - segment.from.x) * ratio,
                        segment.from.y + (segment.to.y - segment.from.y) * ratio,
                    ),
                    kind: segment.kind,
                    power: segment.power,
                    pass: segment.pass,
                });
            }
            travelled += segment.length;
        }
        None
    }

    /// How far the job has travelled after `seconds` of cutting time.
    pub fn length_at_seconds(&self, seconds: f64) -> f64 {
        if seconds <= 0.0 || self.duration_seconds <= 0.0 {
            return 0.0;
        }
        if seconds >= self.duration_seconds {
            return self.total_length;
        }
        let mut elapsed = 0.0;
        let mut travelled = 0.0;
        for segment in &self.segments {
            let duration = if segment.speed > 0.0 {
                segment.length / segment.speed * 60.0
            } else {
                0.0
            };
            if elapsed + duration >= seconds {
                let ratio = if duration > 0.0 {
                    (seconds - elapsed) / duration
                } else {
                    1.0
                };
                return travelled + segment.length * ratio;
            }
            elapsed += duration;
            travelled += segment.length;
        }
        self.total_length
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toolpath(passes: u32) -> Toolpath {
        let mut path = Toolpath::new("a", 600.0, 250.0, passes);
        path.push(MoveKind::Travel, 0.0, 0.0);
        path.push(MoveKind::Cut, 10.0, 0.0);
        path.push(MoveKind::Travel, 10.0, 10.0);
        path.push(MoveKind::Cut, 20.0, 10.0);
        path
    }

    #[test]
    fn segments_record_length_kind_and_power() {
        let playback = Playback::from_toolpaths(&[toolpath(1)], 3000.0);
        assert_eq!(playback.segments.len(), 3);
        assert_eq!(playback.segments[0].kind, MoveKind::Cut);
        assert!((playback.segments[0].length - 10.0).abs() < 1e-9);
        assert!((playback.segments[0].power - 250.0).abs() < 1e-9);
        assert_eq!(playback.segments[1].kind, MoveKind::Travel);
        assert!((playback.segments[1].power).abs() < 1e-9);
        assert!((playback.total_length - 30.0).abs() < 1e-9);
    }

    #[test]
    fn passes_repeat_the_path_and_duration() {
        let playback = Playback::from_toolpaths(&[toolpath(2)], 3000.0);
        assert_eq!(playback.segments.len(), 6);
        assert!((playback.total_length - 60.0).abs() < 1e-9);
        assert_eq!(playback.segments[5].pass, 2);
        // 20 mm of cutting at 600 mm/min plus 10 mm of rapids at 3000 mm/min.
        let expected = (20.0 / 600.0 + 10.0 / 3000.0) * 60.0 * 2.0;
        assert!((playback.duration_seconds - expected).abs() < 1e-9);
    }

    #[test]
    fn position_follows_the_travelled_length() {
        let playback = Playback::from_toolpaths(&[toolpath(1)], 3000.0);
        let start = playback.position_at(0.0).unwrap();
        assert!((start.point.x).abs() < 1e-9);
        assert_eq!(start.kind, MoveKind::Cut);
        let middle = playback.position_at(5.0).unwrap();
        assert!((middle.point.x - 5.0).abs() < 1e-9);
        assert_eq!(middle.power, 250.0);
        let end = playback.position_at(playback.total_length).unwrap();
        assert!((end.point.x - 20.0).abs() < 1e-9);
        assert!((end.point.y - 10.0).abs() < 1e-9);
    }

    #[test]
    fn travel_segments_report_no_power() {
        let playback = Playback::from_toolpaths(&[toolpath(1)], 3000.0);
        let travel = playback.position_at(10.5).unwrap();
        assert_eq!(travel.kind, MoveKind::Travel);
        assert!((travel.power).abs() < 1e-9);
    }

    #[test]
    fn length_at_seconds_matches_the_segments() {
        let playback = Playback::from_toolpaths(&[toolpath(1)], 3000.0);
        assert!((playback.length_at_seconds(0.0)).abs() < 1e-9);
        assert!((playback.length_at_seconds(1.0) - 10.0).abs() < 1e-9);
        // 10 mm of cutting takes 1 s, the 10 mm rapid takes 0.2 s.
        assert!((playback.length_at_seconds(1.1) - 15.0).abs() < 1e-9);
        assert!((playback.length_at_seconds(999.0) - playback.total_length).abs() < 1e-9);
    }

    #[test]
    fn empty_job_has_no_position() {
        let playback = Playback::from_toolpaths(&[], 3000.0);
        assert!(playback.is_empty());
        assert_eq!(playback.position_at(0.0), None);
        assert!((playback.total_length).abs() < 1e-9);
    }
}
