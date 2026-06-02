use swarm_types::{Aabb, Pose, UrbanMap, UrbanRouteSegment};

use super::UrbanRouteError;

/// Interpolate a pose along a planned Urban route segment.
///
/// Distance is clamped to the segment length. A zero-length segment returns the
/// destination node pose.
pub fn pose_along_segment(
    map: &UrbanMap,
    segment: &UrbanRouteSegment,
    distance_m: f64,
) -> Result<Pose, UrbanRouteError> {
    let from = map
        .node(&segment.from)
        .map(|node| node.pose)
        .ok_or_else(|| UrbanRouteError::InvalidInput {
            field: "segment.from".to_owned(),
            message: format!("Unknown urban node id '{}'", segment.from),
        })?;
    let to = map.node(&segment.to).map(|node| node.pose).ok_or_else(|| {
        UrbanRouteError::InvalidInput {
            field: "segment.to".to_owned(),
            message: format!("Unknown urban node id '{}'", segment.to),
        }
    })?;
    if segment.length_m <= 0.0 {
        return Ok(to);
    }
    let ratio = (distance_m / segment.length_m).clamp(0.0, 1.0);
    Ok(Pose {
        x: from.x + (to.x - from.x) * ratio,
        y: from.y + (to.y - from.y) * ratio,
        z: from.z + (to.z - from.z) * ratio,
    })
}

pub(super) fn midpoint(from: Pose, to: Pose) -> Pose {
    Pose {
        x: (from.x + to.x) / 2.0,
        y: (from.y + to.y) / 2.0,
        z: (from.z + to.z) / 2.0,
    }
}

pub(super) fn segment_aabb_clearance(from: Pose, to: Pose, bounds: Aabb) -> f64 {
    if bounds.contains(&from) || bounds.contains(&to) || segment_intersects_aabb(from, to, bounds) {
        return 0.0;
    }

    let mut clearance = point_aabb_distance(from, bounds).min(point_aabb_distance(to, bounds));
    for corner in aabb_corners(bounds) {
        clearance = clearance.min(point_segment_distance(corner, from, to));
    }
    clearance
}

fn point_aabb_distance(point: Pose, bounds: Aabb) -> f64 {
    let dx = if point.x < bounds.min_x {
        bounds.min_x - point.x
    } else if point.x > bounds.max_x {
        point.x - bounds.max_x
    } else {
        0.0
    };
    let dy = if point.y < bounds.min_y {
        bounds.min_y - point.y
    } else if point.y > bounds.max_y {
        point.y - bounds.max_y
    } else {
        0.0
    };
    (dx * dx + dy * dy).sqrt()
}

fn aabb_corners(bounds: Aabb) -> [Pose; 4] {
    [
        Pose {
            x: bounds.min_x,
            y: bounds.min_y,
            ..Default::default()
        },
        Pose {
            x: bounds.min_x,
            y: bounds.max_y,
            ..Default::default()
        },
        Pose {
            x: bounds.max_x,
            y: bounds.min_y,
            ..Default::default()
        },
        Pose {
            x: bounds.max_x,
            y: bounds.max_y,
            ..Default::default()
        },
    ]
}

fn point_segment_distance(point: Pose, from: Pose, to: Pose) -> f64 {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let len_sq = dx * dx + dy * dy;
    if len_sq <= f64::EPSILON {
        return point.distance_to(&from);
    }
    let t = (((point.x - from.x) * dx + (point.y - from.y) * dy) / len_sq).clamp(0.0, 1.0);
    let projected = Pose {
        x: from.x + t * dx,
        y: from.y + t * dy,
        z: from.z + t * (to.z - from.z),
    };
    point.distance_to(&projected)
}

pub(super) fn segment_intersects_aabb(from: Pose, to: Pose, bounds: Aabb) -> bool {
    let mut t_min = 0.0;
    let mut t_max = 1.0;
    let dx = to.x - from.x;
    let dy = to.y - from.y;

    clip_axis(-dx, from.x - bounds.min_x, &mut t_min, &mut t_max)
        && clip_axis(dx, bounds.max_x - from.x, &mut t_min, &mut t_max)
        && clip_axis(-dy, from.y - bounds.min_y, &mut t_min, &mut t_max)
        && clip_axis(dy, bounds.max_y - from.y, &mut t_min, &mut t_max)
}

fn clip_axis(p: f64, q: f64, t_min: &mut f64, t_max: &mut f64) -> bool {
    if p == 0.0 {
        return q >= 0.0;
    }
    let r = q / p;
    if p < 0.0 {
        if r > *t_max {
            return false;
        }
        if r > *t_min {
            *t_min = r;
        }
    } else {
        if r < *t_min {
            return false;
        }
        if r < *t_max {
            *t_max = r;
        }
    }
    true
}
