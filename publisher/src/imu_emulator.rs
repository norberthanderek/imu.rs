use common::proto::ImuData;
use rand::prelude::*;
use rand_distr::{Distribution, Normal};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const ACC_MAX_CHANGE: f32 = 100.0; // mg per update
const GYRO_MAX_CHANGE: i32 = 500; // mDeg/s per update
const MAG_MAX_CHANGE: f32 = 20.0; // mGauss per update

const ACC_NOISE_STD_DEV: f32 = 2.0; // mg
const GYRO_NOISE_STD_DEV: f32 = 50.0; // mDeg/s
const MAG_NOISE_STD_DEV: f32 = 5.0; // mGauss

// Low-pass filter coefficient for sensor data smoothing
const ALPHA: f32 = 0.7; // 0 < ALPHA < 1, higher = more filtering

#[allow(dead_code)]
pub struct ImuEmulator {
    data: ImuData,
    next_target_change: SystemTime,
    rng: ThreadRng,
    acc_target: (f32, f32, f32),
    gyro_target: (i32, i32, i32),
    mag_target: (f32, f32, f32),
    acc_noise: Normal<f32>,
    gyro_noise: Normal<f32>,
    mag_noise: Normal<f32>,
}

#[allow(dead_code)]
impl ImuEmulator {
    pub fn new() -> Self {
        ImuEmulator {
            data: common::proto::ImuData::default(),
            acc_target: (0.0, 0.0, 0.0),
            gyro_target: (0, 0, 0),
            mag_target: (0.0, 0.0, 0.0),
            next_target_change: UNIX_EPOCH,
            rng: rand::rng(),
            // *_STD_DEV are constant and finite, so unwrap is "safe"
            acc_noise: Normal::new(0.0, ACC_NOISE_STD_DEV).unwrap(),
            gyro_noise: Normal::new(0.0, GYRO_NOISE_STD_DEV).unwrap(),
            mag_noise: Normal::new(0.0, MAG_NOISE_STD_DEV).unwrap(),
        }
    }

    pub fn generate_data(&mut self) -> &ImuData {
        let now = SystemTime::now();

        if now >= self.next_target_change {
            self.update_targets();
            self.next_target_change =
                now + Duration::from_millis(self.rng.random_range(1000..3000));
        }

        self.update_accelerometer(now);
        self.update_gyroscope(now);
        self.update_magnetometer(now);

        &self.data
    }

    fn update_targets(&mut self) {
        self.acc_target = (
            self.rng.random_range(-300.0..300.0),
            self.rng.random_range(-300.0..300.0),
            self.rng.random_range(900.0..1100.0), // close to gravity (9.81*100 mg)
        );

        self.gyro_target = (
            self.rng.random_range(-2000..2000),
            self.rng.random_range(-2000..2000),
            self.rng.random_range(-2000..2000),
        );

        self.mag_target = (
            self.rng.random_range(-400.0..400.0),
            self.rng.random_range(-400.0..400.0),
            self.rng.random_range(-400.0..400.0),
        );
    }

    fn get_timestamp(&self, now: SystemTime) -> u32 {
        now.duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_millis() as u32
    }

    fn should_update_sensor(
        &mut self,
        now: SystemTime,
        last_timestamp: u32,
        jitter_range: std::ops::Range<u64>,
    ) -> bool {
        let last_time = UNIX_EPOCH + Duration::from_millis(last_timestamp as u64);
        let elapsed = now
            .duration_since(last_time)
            .unwrap_or(Duration::from_millis(0))
            .as_millis() as u64;

        elapsed >= self.rng.random_range(jitter_range.start..jitter_range.end)
    }

    fn update_accelerometer(&mut self, now: SystemTime) {
        // Update every ~1ms on average with some jitter
        if !self.should_update_sensor(now, self.data.timestamp_acc, 0..2) {
            return;
        }

        self.data.x_acc =
            self.move_toward_target_float(self.data.x_acc, self.acc_target.0, ACC_MAX_CHANGE);
        self.data.y_acc =
            self.move_toward_target_float(self.data.y_acc, self.acc_target.1, ACC_MAX_CHANGE);
        self.data.z_acc =
            self.move_toward_target_float(self.data.z_acc, self.acc_target.2, ACC_MAX_CHANGE);

        self.data.x_acc += self.acc_noise.sample(&mut self.rng);
        self.data.y_acc += self.acc_noise.sample(&mut self.rng);
        self.data.z_acc += self.acc_noise.sample(&mut self.rng);

        self.data.timestamp_acc = self.get_timestamp(now)
    }

    fn update_gyroscope(&mut self, now: SystemTime) {
        // Update every ~1.25ms on average with some jitter
        if !self.should_update_sensor(now, self.data.timestamp_gyro, 1..2) {
            return;
        }

        self.data.x_gyro =
            self.move_toward_target_int(self.data.x_gyro, self.gyro_target.0, GYRO_MAX_CHANGE);
        self.data.y_gyro =
            self.move_toward_target_int(self.data.y_gyro, self.gyro_target.1, GYRO_MAX_CHANGE);
        self.data.z_gyro =
            self.move_toward_target_int(self.data.z_gyro, self.gyro_target.2, GYRO_MAX_CHANGE);

        self.data.x_gyro += self.gyro_noise.sample(&mut self.rng) as i32;
        self.data.y_gyro += self.gyro_noise.sample(&mut self.rng) as i32;
        self.data.z_gyro += self.gyro_noise.sample(&mut self.rng) as i32;

        self.data.timestamp_gyro = self.get_timestamp(now);
    }

    fn update_magnetometer(&mut self, now: SystemTime) {
        // Update every ~2ms on average with some jitter
        if !self.should_update_sensor(now, self.data.timestamp_mag, 1..3) {
            return;
        }

        self.data.x_mag =
            self.move_toward_target_float(self.data.x_mag, self.mag_target.0, MAG_MAX_CHANGE);
        self.data.y_mag =
            self.move_toward_target_float(self.data.y_mag, self.mag_target.1, MAG_MAX_CHANGE);
        self.data.z_mag =
            self.move_toward_target_float(self.data.z_mag, self.mag_target.2, MAG_MAX_CHANGE);

        self.data.x_mag += self.mag_noise.sample(&mut self.rng);
        self.data.y_mag += self.mag_noise.sample(&mut self.rng);
        self.data.z_mag += self.mag_noise.sample(&mut self.rng);

        self.data.timestamp_mag = self.get_timestamp(now);
    }

    fn move_toward_target_float(&self, current: f32, target: f32, max_change: f32) -> f32 {
        let diff = target - current;
        if diff.abs() <= max_change {
            return target;
        }

        let step = if diff > 0.0 { max_change } else { -max_change };
        let new_value = current + step;

        ALPHA * new_value + (1.0 - ALPHA) * current
    }

    fn move_toward_target_int(&self, current: i32, target: i32, max_change: i32) -> i32 {
        let diff = target - current;
        if diff.abs() <= max_change {
            return target;
        }

        let step = if diff > 0 { max_change } else { -max_change };
        let new_value = current + step;

        (ALPHA * new_value as f32 + (1.0 - ALPHA) * current as f32) as i32
    }

    #[allow(dead_code)]
    pub fn get_data(&self) -> &ImuData {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn test_generate_data_updates_values() {
        let mut emulator = ImuEmulator::new();
        let initial_acc_x = emulator.data.x_acc;
        let initial_gyro_y = emulator.data.y_gyro;
        let initial_mag_z = emulator.data.z_mag;

        sleep(Duration::from_millis(10));
        emulator.generate_data();

        assert!(emulator.data.x_acc != initial_acc_x || emulator.data.timestamp_acc > 0);
        assert!(emulator.data.y_gyro != initial_gyro_y || emulator.data.timestamp_gyro > 0);
        assert!(emulator.data.z_mag != initial_mag_z || emulator.data.timestamp_mag > 0);
    }

    #[test]
    fn test_timestamps_increase_monotonically() {
        let mut emulator = ImuEmulator::new();

        let initial_ts_acc = emulator.data.timestamp_acc;
        let initial_ts_gyro = emulator.data.timestamp_gyro;
        let initial_ts_mag = emulator.data.timestamp_mag;

        sleep(Duration::from_millis(10));
        for _ in 0..5 {
            emulator.generate_data();
            sleep(Duration::from_millis(5));
        }

        assert!(emulator.data.timestamp_acc > initial_ts_acc);
        assert!(emulator.data.timestamp_gyro > initial_ts_gyro);
        assert!(emulator.data.timestamp_mag > initial_ts_mag);
    }

    #[test]
    fn test_move_toward_target_float() {
        let emulator = ImuEmulator::new();

        // Reached the target
        let result = emulator.move_toward_target_float(5.0, 6.0, 2.0);
        assert_eq!(result, 6.0);

        // Limited by max change (positive direction)
        let result = emulator.move_toward_target_float(5.0, 10.0, 2.0);
        assert!(result > 5.0 && result < 10.0);
        assert!((result - 7.0).abs() < ALPHA);

        // Limited by max change (negative direction)
        let result = emulator.move_toward_target_float(5.0, 0.0, 2.0);
        assert!(result < 5.0 && result > 0.0);
        assert!((result - 3.0).abs() < ALPHA);
    }

    #[test]
    fn test_move_toward_target_int() {
        let emulator = ImuEmulator::new();

        // Reached the target
        let result = emulator.move_toward_target_int(100, 150, 100);
        assert_eq!(result, 150);

        // Limited by max change (positive direction)
        let result = emulator.move_toward_target_int(100, 300, 50);
        assert!(result > 100 && result < 300);
        let expected = (ALPHA * 150.0 + (1.0 - ALPHA) * 100.0) as i32;
        assert_eq!(result, expected);

        // Limited by max change (negative direction)
        let result = emulator.move_toward_target_int(100, 0, 50);
        assert!(result < 100 && result > 0);
        let expected = (ALPHA * 50.0 + (1.0 - ALPHA) * 100.0) as i32;
        assert_eq!(result, expected);
    }

    #[test]
    fn test_update_targets_changes_all_targets() {
        let mut emulator = ImuEmulator::new();
        let initial_acc_target = emulator.acc_target;
        let initial_gyro_target = emulator.gyro_target;
        let initial_mag_target = emulator.mag_target;

        emulator.update_targets();
        assert!(
            emulator.acc_target != initial_acc_target
                || emulator.gyro_target != initial_gyro_target
                || emulator.mag_target != initial_mag_target
        );
    }

    #[test]
    fn test_data_changes_smoothly() {
        let mut emulator = ImuEmulator::new();

        let mut acc_x_values = Vec::new();
        let mut gyro_y_values = Vec::new();
        let mut mag_z_values = Vec::new();

        for _ in 0..10 {
            emulator.generate_data();
            acc_x_values.push(emulator.data.x_acc);
            gyro_y_values.push(emulator.data.y_gyro);
            mag_z_values.push(emulator.data.z_mag);
            sleep(Duration::from_millis(5));
        }

        // Check for smooth changes (no large jumps)
        for i in 1..acc_x_values.len() {
            let diff = (acc_x_values[i] - acc_x_values[i - 1]).abs();
            assert!(diff <= ACC_MAX_CHANGE + ACC_NOISE_STD_DEV * 3.0);
        }
        for i in 1..gyro_y_values.len() {
            let diff = (gyro_y_values[i] - gyro_y_values[i - 1]).abs();
            assert!(diff as f32 <= GYRO_MAX_CHANGE as f32 + GYRO_NOISE_STD_DEV * 3.0);
        }
        for i in 1..mag_z_values.len() {
            let diff = (mag_z_values[i] - mag_z_values[i - 1]).abs();
            assert!(diff <= MAG_MAX_CHANGE + MAG_NOISE_STD_DEV * 3.0);
        }
    }
}
