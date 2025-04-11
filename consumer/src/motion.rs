use common::proto::ImuData;
use common::slog::{Logger, debug, warn};
use nalgebra::{Quaternion, UnitQuaternion, Vector3};

const MIN_DELTA_TIME: f32 = 0.001;
const MAX_DELTA_TIME: f32 = 0.1;

#[derive(Debug, Clone)]
pub struct MotionState {
    pub orientation: UnitQuaternion<f32>,
    pub velocity: Vector3<f32>,
    pub position: Vector3<f32>,
    last_acc_timestamp: u32,
    last_gyro_timestamp: u32,
}

impl Default for MotionState {
    fn default() -> Self {
        Self {
            orientation: UnitQuaternion::identity(),
            velocity: Vector3::zeros(),
            position: Vector3::zeros(),
            last_acc_timestamp: 0,
            last_gyro_timestamp: 0,
        }
    }
}

#[derive(Debug)]
pub struct MotionProcessor {
    state: MotionState,
    logger: Logger,
    acc_bias: Vector3<f32>,
    gyro_bias: Vector3<f32>,
    gyro_weight: f32,
    acc_weight: f32,
    velocity_decay: f32,
    disable_complementary_filter: bool,
}

impl MotionProcessor {
    pub fn new(logger: Logger) -> Self {
        Self {
            state: MotionState::default(),
            logger,
            acc_bias: Vector3::zeros(),
            gyro_bias: Vector3::zeros(),
            gyro_weight: 0.98,
            acc_weight: 0.02,
            velocity_decay: 0.98,
            disable_complementary_filter: false,
        }
    }

    pub fn process(&mut self, imu_data: &ImuData) -> &MotionState {
        self.update_orientation(imu_data);
        self.update_velocity_and_position(imu_data);
        &self.state
    }

    fn update_orientation(&mut self, imu_data: &ImuData) {
        let dt_gyro = if self.state.last_gyro_timestamp != 0 {
            imu_data.timestamp_gyro.saturating_sub(self.state.last_gyro_timestamp) as f32 / 1000.0
        } else {
            MIN_DELTA_TIME
        };
        self.state.last_gyro_timestamp = imu_data.timestamp_gyro;

        if dt_gyro > MAX_DELTA_TIME {
            warn!(self.logger, "Skipping orientation update due to excesive time delta"; "dt_gyro" => dt_gyro);
            return;
        }

        let gyro_x =
            (imu_data.x_gyro as f32 - self.gyro_bias.x) * 0.001 * std::f32::consts::PI / 180.0;
        let gyro_y =
            (imu_data.y_gyro as f32 - self.gyro_bias.y) * 0.001 * std::f32::consts::PI / 180.0;
        let gyro_z =
            (imu_data.z_gyro as f32 - self.gyro_bias.z) * 0.001 * std::f32::consts::PI / 180.0;

        let gyro_vec = Vector3::new(gyro_x, gyro_y, gyro_z);

        const EPSILON: f32 = 1e-6;
        let angle = gyro_vec.norm() * dt_gyro;

        if angle < EPSILON {
            debug!(self.logger, "Skipping orientation update due to small angle"; "angle" => angle);
            return;
        }

        let axis = if gyro_vec.norm() > EPSILON {
            gyro_vec.normalize()
        } else {
            Vector3::x()
        };

        let axis_unit = nalgebra::Unit::new_normalize(axis);
        let gyro_quat = UnitQuaternion::from_axis_angle(&axis_unit, angle);

        let gyro_orientation = self.state.orientation * gyro_quat;

        if self.disable_complementary_filter {
            debug!(
                self.logger,
                "Complementary filter disabled, using gyro orientation"; "gyro_orientation" => ?gyro_orientation
            );
            self.state.orientation = gyro_orientation;
        } else {
            let acc_vec = Vector3::new(
                imu_data.x_acc - self.acc_bias.x,
                imu_data.y_acc - self.acc_bias.y,
                imu_data.z_acc - self.acc_bias.z,
            );

            let acc_magnitude = acc_vec.norm();
            if (acc_magnitude > 950.0) && (acc_magnitude < 1050.0) {
                let acc_norm = acc_vec / acc_magnitude;

                let gravity = Vector3::new(0.0, 0.0, 1.0);
                let gravity_unit = nalgebra::Unit::new_normalize(gravity);
                let acc_norm_unit = nalgebra::Unit::new_normalize(acc_norm);

                let acc_quat = UnitQuaternion::rotation_between(&gravity_unit, &acc_norm_unit)
                    .unwrap_or(UnitQuaternion::identity());

                self.state.orientation = UnitQuaternion::from_quaternion(
                    Quaternion::new(
                        self.gyro_weight * gyro_orientation.scalar()
                            + self.acc_weight * acc_quat.scalar(),
                        self.gyro_weight * gyro_orientation.vector().x
                            + self.acc_weight * acc_quat.vector().x,
                        self.gyro_weight * gyro_orientation.vector().y
                            + self.acc_weight * acc_quat.vector().y,
                        self.gyro_weight * gyro_orientation.vector().z
                            + self.acc_weight * acc_quat.vector().z,
                    )
                    .normalize(),
                );
            } else {
                self.state.orientation = gyro_orientation;
            }
        }
    }

    fn update_velocity_and_position(&mut self, imu_data: &ImuData) {
        let dt_acc = if self.state.last_acc_timestamp != 0 {
            imu_data.timestamp_acc.saturating_sub(self.state.last_acc_timestamp) as f32 / 1000.0
        } else {
            MIN_DELTA_TIME
        };
        self.state.last_acc_timestamp = imu_data.timestamp_acc;

        if dt_acc > MAX_DELTA_TIME {
            warn!(self.logger, "Skipping velocity/position update due to excessive time delta"; "dt_acc" => dt_acc);
            return;
        }

        let acc_body = Vector3::new(
            (imu_data.x_acc - self.acc_bias.x) * 9.81 / 1000.0,
            (imu_data.y_acc - self.acc_bias.y) * 9.81 / 1000.0,
            (imu_data.z_acc - self.acc_bias.z) * 9.81 / 1000.0,
        );

        let gravity = Vector3::new(0.0, 0.0, 9.81);

        let acc_world = self.state.orientation * acc_body;
        let acc_world_no_gravity = acc_world - gravity;

        let acc_threshold = 0.01;
        let filtered_acc =
            acc_world_no_gravity.map(|a| if a.abs() < acc_threshold { 0.0 } else { a });

        self.state.velocity += filtered_acc * dt_acc;
        self.state.velocity *= self.velocity_decay;
        self.state.position += self.state.velocity * dt_acc;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use common::slog::{Discard, Logger, o};

    fn create_test_logger() -> Logger {
        Logger::root(Discard, o!())
    }

    fn create_test_imu_data(
        x_acc: f32,
        y_acc: f32,
        z_acc: f32,
        x_gyro: i32,
        y_gyro: i32,
        z_gyro: i32,
        timestamp: u32,
    ) -> ImuData {
        ImuData {
            x_acc,
            y_acc,
            z_acc,
            timestamp_acc: timestamp,
            x_gyro,
            y_gyro,
            z_gyro,
            timestamp_gyro: timestamp,
            x_mag: 0.0,
            y_mag: 0.0,
            z_mag: 0.0,
            timestamp_mag: timestamp,
        }
    }

    #[test]
    fn test_default_motion_state() {
        let state = MotionState::default();
        assert_eq!(state.position, Vector3::zeros());
        assert_eq!(state.velocity, Vector3::zeros());
        assert_eq!(state.orientation, UnitQuaternion::identity());
        assert_eq!(state.last_acc_timestamp, 0);
        assert_eq!(state.last_gyro_timestamp, 0);
    }

    #[test]
    fn test_acceleration_integration() {
        let logger = create_test_logger();
        let mut processor = MotionProcessor::new(logger);

        processor.velocity_decay = 1.0;

        let imu_data = create_test_imu_data(1000.0, 0.0, 1000.0, 0, 0, 0, 1000);

        let total_time_steps = 100;
        let dt_ms = 10;
        let dt_s = dt_ms as f32 / 1000.0;

        for i in 0..total_time_steps {
            let mut data = imu_data;
            let current_timestamp = 1000 + i * dt_ms;
            data.timestamp_acc = current_timestamp;
            data.timestamp_gyro = current_timestamp;
            processor.process(&data);
        }

        let expected_velocity_x = 9.81 * (total_time_steps as f32 * dt_s);

        let expected_position_x = 0.5 * 9.81 * (total_time_steps as f32 * dt_s).powi(2);

        println!(
            "Final Velocity: {:?}, Final Position: {:?}",
            processor.state.velocity, processor.state.position
        );

        assert_relative_eq!(
            processor.state.velocity.x,
            expected_velocity_x,
            epsilon = 0.1
        );
        assert_relative_eq!(
            processor.state.position.x,
            expected_position_x,
            epsilon = 0.1
        );

        assert_relative_eq!(processor.state.velocity.y, 0.0, epsilon = 0.01);
        assert_relative_eq!(processor.state.position.y, 0.0, epsilon = 0.01);

        assert_relative_eq!(processor.state.velocity.z, 0.0, epsilon = 0.1);
        assert_relative_eq!(processor.state.position.z, 0.0, epsilon = 0.1);
    }

    #[test]
    fn test_complementary_filter() {
        let logger = create_test_logger();
        let mut processor = MotionProcessor::new(logger);

        processor.gyro_weight = 0.5;
        processor.acc_weight = 0.5;

        let imu_data = create_test_imu_data(0.0, 0.0, 1000.0, 10000, 0, 0, 1000);

        for i in 0..10 {
            let mut data = imu_data;
            data.timestamp_acc = 1000 + i * 10;
            data.timestamp_gyro = 1000 + i * 10;
            processor.process(&data);
        }

        let (roll, _, _) = processor.state.orientation.euler_angles();

        assert!(roll > 0.0);
        assert!(roll < 0.17);
    }
}
