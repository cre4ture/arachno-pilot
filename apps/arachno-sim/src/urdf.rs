use std::fmt::Write;

use arachno_core::SimulationRobotSpec;

const LINK_RADIUS_M: f64 = 0.008;
const FOOT_RADIUS_M: f64 = 0.012;
const COXA_MASS_KG: f64 = 0.015;
const FEMUR_MASS_KG: f64 = 0.025;
const TIBIA_MASS_KG: f64 = 0.020;
const FOOT_MASS_KG: f64 = 0.005;

/// Generate a URDF robot description from the exported simulation spec.
///
/// Topology per leg (parent → child):
///   body → [coxa_joint] → {leg}_coxa
///   {leg}_coxa → [femur_joint] → {leg}_femur
///   {leg}_femur → [tibia_joint] → {leg}_tibia
///   {leg}_tibia → [foot_joint] → {leg}_foot
///
/// All links are oriented so that +X is the "outward / distal" direction.
/// The coxa joint rotates about body +Z; femur and tibia joints rotate about
/// link-frame +Y (swing up/down in the sagittal plane).
pub fn generate_urdf(spec: &SimulationRobotSpec) -> String {
    let mut out = String::new();
    let body = &spec.simulation;
    let [bx, by, bz] = body.body_size_cm.map(|v| v as f64 / 100.0);
    let body_mass_kg = body.body_mass_kg as f64;

    writeln!(&mut out, r#"<?xml version="1.0"?>"#).unwrap();
    writeln!(&mut out, r#"<robot name="{}">"#, spec.robot_name).unwrap();
    writeln!(&mut out).unwrap();

    // ── Body link ─────────────────────────────────────────────────────────
    // Body frame origin is at body centre (mount positions are body-centred).
    // Box geometry and CoM are both at [0,0,0].
    push_body_link(&mut out, "body", body_mass_kg, bx, by, bz);

    // ── Per-leg links and joints ───────────────────────────────────────────
    for leg in &spec.legs {
        let name = &leg.name;
        let [mx, my, mz] = leg.mount_position_cm.map(|v| v as f64 / 100.0);
        let coxa_m = leg.link_lengths_cm.coxa_cm as f64 / 100.0;
        let femur_m = leg.link_lengths_cm.femur_cm as f64 / 100.0;
        let tibia_m = leg.link_lengths_cm.tibia_cm as f64 / 100.0;

        // Coxa joint (body → coxa). Rotates about body +Z.
        // RPY encodes the zero heading so the coxa link's +X points in the
        // natural outward direction when the joint is at 0.
        let coxa_heading_rad = (leg.coxa_zero_heading_deg as f64).to_radians();
        push_joint(
            &mut out,
            &format!("{name}_coxa_joint"),
            "body",
            &format!("{name}_coxa"),
            [mx, my, mz],
            [0.0, 0.0, coxa_heading_rad],
            [0.0, 0.0, 1.0],
            // Coxa limits: ±90° symmetric swing
            (-std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2),
        );
        push_link(
            &mut out,
            &format!("{name}_coxa"),
            COXA_MASS_KG,
            [coxa_m / 2.0, 0.0, 0.0],
            &cylinder_xml(coxa_m, LINK_RADIUS_M),
            [0.0, std::f64::consts::FRAC_PI_2, 0.0],
        );

        // Femur joint (coxa → femur). Rotates about link-frame +Y.
        push_joint(
            &mut out,
            &format!("{name}_femur_joint"),
            &format!("{name}_coxa"),
            &format!("{name}_femur"),
            [coxa_m, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            (-std::f64::consts::FRAC_PI_2 * 1.5, std::f64::consts::FRAC_PI_2 * 1.5),
        );
        push_link(
            &mut out,
            &format!("{name}_femur"),
            FEMUR_MASS_KG,
            [femur_m / 2.0, 0.0, 0.0],
            &cylinder_xml(femur_m, LINK_RADIUS_M),
            [0.0, std::f64::consts::FRAC_PI_2, 0.0],
        );

        // Tibia joint (femur → tibia). Same axis as femur.
        push_joint(
            &mut out,
            &format!("{name}_tibia_joint"),
            &format!("{name}_femur"),
            &format!("{name}_tibia"),
            [femur_m, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            (-std::f64::consts::PI, 0.0),
        );
        push_link(
            &mut out,
            &format!("{name}_tibia"),
            TIBIA_MASS_KG,
            [tibia_m / 2.0, 0.0, 0.0],
            &cylinder_xml(tibia_m, LINK_RADIUS_M),
            [0.0, std::f64::consts::FRAC_PI_2, 0.0],
        );

        // Foot (fixed, sphere for contact).
        push_fixed_joint(
            &mut out,
            &format!("{name}_foot_joint"),
            &format!("{name}_tibia"),
            &format!("{name}_foot"),
            [tibia_m, 0.0, 0.0],
        );
        push_foot_link(&mut out, &format!("{name}_foot"), FOOT_MASS_KG, FOOT_RADIUS_M);
    }

    writeln!(&mut out, "</robot>").unwrap();
    out
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn cylinder_xml(length: f64, radius: f64) -> String {
    format!(r#"<cylinder length="{length:.4}" radius="{radius:.4}"/>"#)
}

/// Body link: box centred on the body frame origin (mount positions are body-centred).
fn push_body_link(out: &mut String, name: &str, mass: f64, bx: f64, by: f64, bz: f64) {
    let [ixx, ixy, ixz, iyy, iyz, izz] = inertia_box(mass, bx, by, bz);
    writeln!(
        out,
        r#"  <link name="{name}">
    <inertial>
      <origin xyz="0 0 0" rpy="0 0 0"/>
      <mass value="{mass:.4}"/>
      <inertia ixx="{ixx:.6}" ixy="{ixy:.6}" ixz="{ixz:.6}"
               iyy="{iyy:.6}" iyz="{iyz:.6}" izz="{izz:.6}"/>
    </inertial>
    <visual>
      <origin xyz="0 0 0" rpy="0 0 0"/>
      <geometry><box size="{bx:.4} {by:.4} {bz:.4}"/></geometry>
    </visual>
    <collision>
      <origin xyz="0 0 0" rpy="0 0 0"/>
      <geometry><box size="{bx:.4} {by:.4} {bz:.4}"/></geometry>
    </collision>
  </link>"#
    )
    .unwrap();
}

fn inertia_cylinder(mass: f64, length: f64, radius: f64) -> [f64; 6] {
    // Solid cylinder, axis along Z (link frame has +X as primary axis, but inertia
    // is expressed in the link's principal frame — adjust as needed for accuracy).
    let ixx = mass / 12.0 * (3.0 * radius * radius + length * length);
    let iyy = ixx;
    let izz = 0.5 * mass * radius * radius;
    [ixx, 0.0, 0.0, iyy, 0.0, izz] // ixx ixy ixz iyy iyz izz
}

fn inertia_box(mass: f64, x: f64, y: f64, z: f64) -> [f64; 6] {
    let ixx = mass / 12.0 * (y * y + z * z);
    let iyy = mass / 12.0 * (x * x + z * z);
    let izz = mass / 12.0 * (x * x + y * y);
    [ixx, 0.0, 0.0, iyy, 0.0, izz]
}

/// Limb link: cylinder geometry, CoM at the midpoint along +X.
fn push_link(
    out: &mut String,
    name: &str,
    mass: f64,
    com: [f64; 3],
    geometry: &str,
    visual_rpy: [f64; 3],
) {
    let [cx, cy, cz] = com;
    let [rx, ry, rz] = visual_rpy;
    let [ixx, ixy, ixz, iyy, iyz, izz] = inertia_cylinder(mass, cx * 2.0, LINK_RADIUS_M);

    writeln!(
        out,
        r#"  <link name="{name}">
    <inertial>
      <origin xyz="{cx:.4} {cy:.4} {cz:.4}" rpy="0 0 0"/>
      <mass value="{mass:.4}"/>
      <inertia ixx="{ixx:.6}" ixy="{ixy:.6}" ixz="{ixz:.6}"
               iyy="{iyy:.6}" iyz="{iyz:.6}" izz="{izz:.6}"/>
    </inertial>
    <visual>
      <origin xyz="{cx:.4} {cy:.4} {cz:.4}" rpy="{rx:.4} {ry:.4} {rz:.4}"/>
      <geometry>{geometry}</geometry>
    </visual>
    <collision>
      <origin xyz="{cx:.4} {cy:.4} {cz:.4}" rpy="{rx:.4} {ry:.4} {rz:.4}"/>
      <geometry>{geometry}</geometry>
    </collision>
  </link>"#
    )
    .unwrap();
}

fn push_foot_link(out: &mut String, name: &str, mass: f64, radius: f64) {
    let izz = 2.0 * mass * radius * radius / 5.0;
    let ixx = izz;
    let iyy = izz;
    writeln!(
        out,
        r#"  <link name="{name}">
    <inertial>
      <origin xyz="0 0 0" rpy="0 0 0"/>
      <mass value="{mass:.4}"/>
      <inertia ixx="{ixx:.6}" ixy="0" ixz="0"
               iyy="{iyy:.6}" iyz="0" izz="{izz:.6}"/>
    </inertial>
    <visual>
      <origin xyz="0 0 0" rpy="0 0 0"/>
      <geometry><sphere radius="{radius:.4}"/></geometry>
    </visual>
    <collision>
      <origin xyz="0 0 0" rpy="0 0 0"/>
      <geometry><sphere radius="{radius:.4}"/></geometry>
    </collision>
  </link>"#
    )
    .unwrap();
}

fn push_joint(
    out: &mut String,
    name: &str,
    parent: &str,
    child: &str,
    xyz: [f64; 3],
    rpy: [f64; 3],
    axis: [f64; 3],
    limits: (f64, f64),
) {
    let [x, y, z] = xyz;
    let [r, p, ya] = rpy;
    let [ax, ay, az] = axis;
    let (lower, upper) = limits;
    // effort in N·m and velocity in rad/s — reasonable defaults for hobby servos
    writeln!(
        out,
        r#"  <joint name="{name}" type="revolute">
    <parent link="{parent}"/>
    <child link="{child}"/>
    <origin xyz="{x:.4} {y:.4} {z:.4}" rpy="{r:.4} {p:.4} {ya:.4}"/>
    <axis xyz="{ax:.4} {ay:.4} {az:.4}"/>
    <limit lower="{lower:.4}" upper="{upper:.4}" effort="3.0" velocity="4.2"/>
  </joint>"#
    )
    .unwrap();
}

fn push_fixed_joint(out: &mut String, name: &str, parent: &str, child: &str, xyz: [f64; 3]) {
    let [x, y, z] = xyz;
    writeln!(
        out,
        r#"  <joint name="{name}" type="fixed">
    <parent link="{parent}"/>
    <child link="{child}"/>
    <origin xyz="{x:.4} {y:.4} {z:.4}" rpy="0 0 0"/>
  </joint>"#
    )
    .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use arachno_core::RobotConfig;
    use std::path::PathBuf;

    fn load_spec() -> SimulationRobotSpec {
        let config_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/robot/default.toml");
        RobotConfig::load_from_path(config_path)
            .expect("default config should load")
            .simulation_spec()
    }

    #[test]
    fn urdf_output_is_valid_xml_with_all_links_and_joints() {
        let spec = load_spec();
        let urdf = generate_urdf(&spec);

        // Must open and close the robot element.
        assert!(urdf.contains("<robot name="), "missing robot element");
        assert!(urdf.contains("</robot>"), "robot element not closed");

        // Body link must be present.
        assert!(urdf.contains(r#"<link name="body">"#), "body link missing");

        // Every leg must have all four links and joints.
        for leg in &spec.legs {
            let n = &leg.name;
            for suffix in ["_coxa", "_femur", "_tibia", "_foot"] {
                assert!(
                    urdf.contains(&format!(r#"<link name="{n}{suffix}">"#)),
                    "{n}{suffix} link missing"
                );
            }
            for suffix in ["_coxa_joint", "_femur_joint", "_tibia_joint", "_foot_joint"] {
                assert!(
                    urdf.contains(&format!(r#"<joint name="{n}{suffix}""#)),
                    "{n}{suffix} joint missing"
                );
            }
        }
    }

    #[test]
    fn urdf_coxa_joint_encodes_zero_heading() {
        let spec = load_spec();
        let urdf = generate_urdf(&spec);

        // front_left has coxa_zero_heading_deg = 45°; rpy yaw should be ~0.7854 rad
        let expected_yaw = format!("{:.4}", 45_f64.to_radians());
        assert!(
            urdf.contains(&expected_yaw),
            "expected coxa heading {expected_yaw} rad in URDF"
        );
    }

    #[test]
    fn urdf_link_lengths_use_metres() {
        let spec = load_spec();
        let urdf = generate_urdf(&spec);

        let first_leg = &spec.legs[0];
        let femur_m = format!("{:.4}", first_leg.link_lengths_cm.femur_cm as f64 / 100.0);
        // femur_m should appear as the length of a cylinder
        assert!(
            urdf.contains(&femur_m),
            "expected femur length {femur_m} m in URDF"
        );
    }
}
