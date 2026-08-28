use std::{path::{Path, PathBuf}, time::{Duration, Instant}};

use sysinfo::{Disks, MemoryRefreshKind, Pid, ProcessesToUpdate, System};
use anyhow::Result;

pub struct SystemInfo {
    process_id: Pid,
    disk_mount_point: PathBuf,
    disks: Disks,
    system: System,

    drive_refresh_timer: Instant,
    drive_refresh_interval: Duration,

    available_physical_cores: Option<u32>,
    max_allowed_cores: u32,
    total_memory: u64,
}

const DEFAULT_MAX_CORES: u32 = 4;
const DRIVE_SPACE_REFRESH_INTERVAL: u64 = 1000; // in ms

impl SystemInfo {
    pub fn init(working_dir: &Path) -> Result<SystemInfo> {
        let physical_cores = match System::physical_core_count() {
            Some(cores) => Some(cores as u32),
            None => None,
        };

        let max_allowed_cores = physical_cores.unwrap_or(DEFAULT_MAX_CORES);

        let disks = Disks::new_with_refreshed_list();

        let disk_mount_point = disks
            .list()
            .iter()
            .filter(|disk| working_dir.starts_with(disk.mount_point()))
            .max_by_key(|disk| disk.mount_point().components().count())
            .map(|disk| disk.mount_point().to_path_buf());

        let disk_mount_point = match disk_mount_point {
            Some(path) => path,
            None => {
                return Err(anyhow::format_err!("couldn't determine drive for working directory: {}", working_dir.display()));
            }
        };

        let process_id = std::process::id();
        let process_id = Pid::from_u32(process_id);

        let mut system = System::new();
        let drive_refresh_timer = Instant::now();
        let drive_refresh_interval = Duration::from_millis(DRIVE_SPACE_REFRESH_INTERVAL);

        system.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
        let total_memory = system.total_memory();

        Ok(SystemInfo {
            process_id,
            disk_mount_point,
            disks,
            system,
            drive_refresh_timer,
            drive_refresh_interval,

            available_physical_cores: physical_cores,
            max_allowed_cores: max_allowed_cores,
            total_memory: total_memory,
        })
    }

    pub fn set_max_allowed_cores(&mut self, cores: u32) {
        let mut max_cores = cores.max(1); // at least 1

        if let Some(available_physical_cores) = self.available_physical_cores {
            max_cores = max_cores.min(available_physical_cores);
        }

        self.max_allowed_cores = max_cores;
    }

    pub fn max_allowed_cores(&self) -> u32 {
        self.max_allowed_cores
    }

    pub fn available_physical_cores(&self) -> Option<u32> {
        self.available_physical_cores
    }

    pub fn available_memory(&mut self) -> u64 {
        self.system.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
        self.system.available_memory()
    }

    pub fn total_memory(&self) -> u64 {
        self.total_memory
    }

    pub fn process_memory_usage(&mut self) -> u64 {
        self.system.refresh_processes(ProcessesToUpdate::Some(&[self.process_id]), true);

        self.system
            .process(self.process_id)
            .expect("failed to retrieve current process from sysinfo")
            .memory()
    }

    fn get_disk(&mut self) -> Option<&mut sysinfo::Disk> {
        if self.drive_refresh_timer.elapsed() >= self.drive_refresh_interval {
            // NOTE: we're refreshing the entire disks collection here instead of an individual disk as
            // refreshing the individual disk has a bug with macOS where available_space doesn't update
            // see https://github.com/GuillaumeGomez/sysinfo/issues/1046
            self.disks.refresh(false);
            self.drive_refresh_timer = Instant::now();
        }

        self
            .disks
            .list_mut()
            .iter_mut()
            .find(|disk| disk.mount_point() == &self.disk_mount_point)
    }

    pub fn total_drive_space(&mut self) -> Option<u64> {
        let disk = self.get_disk()?;
        Some(disk.total_space())
    }

    pub fn free_drive_space(&mut self) -> Option<u64> {
        let disk = self.get_disk()?;
        Some(disk.available_space())
    }

    pub fn get_process_memory_usage(process_id: u32) -> Option<u64> {
        let process_id = Pid::from_u32(process_id);

        let mut system = System::new();
        system.refresh_processes(ProcessesToUpdate::Some(&[process_id]), true);

        match system.process(process_id) {
            Some(process) => Some(process.memory()),
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::{ops::Sub, thread};

    fn get_sys_info() -> SystemInfo {
        let working_dir = std::fs::canonicalize("./")
            .expect("could niot get canonical path");

        SystemInfo::init(&working_dir)
            .expect("could not create SystemInfo")
    }

    #[test]
    fn should_return_valid_values() {
        let mut sys_info = get_sys_info();

        // drive space methods
        let total_drive_space = sys_info.total_drive_space().unwrap_or_else(|| {
            panic!("`total_drive_space` should return a value");
        });
        assert!(total_drive_space > 0, "`total_drive_space()` should be more than zero");

        let free_drive_space = sys_info.free_drive_space().unwrap_or_else(|| {
            panic!("`free_drive_space` should return a value");
        });
        assert!(free_drive_space > 0, "`free_drive_space`() should be more than zero");

        // memeory
        let available_memory = sys_info.available_memory();
        assert!(available_memory > 0, "`available_memory()` should be more than zero");

        let total_memory = sys_info.total_memory();
        assert!(total_memory > 0, "`total_memory()` should be more than zero");
        assert!(total_memory > available_memory, "``total_memory()` should be higher than `available_memory()`");

        // CPU
        let expected_total_cores = System::physical_core_count()
            .expect("could not determine core count") as u32;

        let total_cores = sys_info.available_physical_cores().unwrap_or_else(|| {
            panic!("`total_cores` should return a value");
        });
        assert_eq!(total_cores, expected_total_cores, "`total_cores()` should be more than zero");

    }

    #[test]
    fn hard_space_should_update() {
        let mut sys_info = get_sys_info();

        sys_info.drive_refresh_interval = Duration::from_millis(100);
        let timer = Instant::now();

        // multiple calls to free_drive_space should return same value (cached for 1 second)
        let free_drive_space = sys_info.free_drive_space().unwrap_or_else(|| {
            panic!("`free_drive_space` should return a value");
        });

        for _ in 0..5 {
            let new_free_drive_space = sys_info.free_drive_space().unwrap_or_else(|| {
                panic!("`free_drive_space` should return a value");
            });

            assert_eq!(free_drive_space, new_free_drive_space);
        }

        // reduce free space by 1MB
        let working_dir = std::fs::canonicalize("./")
            .expect("could niot get canonical path");
        let test_file = working_dir.join("app_system_info_hard_space_should_update_test_file.bin");
        std::fs::write(&test_file, vec![0_u8; 1024 * 1024])
            .expect("could not create test file");

        // wait for cache to expire
        thread::sleep(Duration::from_millis(101).sub(timer.elapsed()));

        // free space should be reduced
        let updated_free_drive_space = sys_info.free_drive_space().unwrap_or_else(|| {
            panic!("`free_drive_space` should return a value");
        });

        // space can go up as wel as down if another process is writing to disk
        let updated_space_has_been_changed = updated_free_drive_space != free_drive_space;

        std::fs::remove_file(&test_file)
            .expect("could not remove test file");

        assert!(updated_space_has_been_changed, "available drive space should have changed");
     }

    #[test]
     fn check_external_process_memory() {
        let mut sys_info = get_sys_info();
        let this_process_mem_usage = sys_info.process_memory_usage();

        let process_id = std::process::id();
        let external_mem_usage = SystemInfo::get_process_memory_usage(process_id)
            .expect("could not get external process memory usage");

        // output as rounded to nearest MB as there can be slight variation of
        // RAM usage as the unit test is running, this level of accuracy is sufficient for our purposes
        let this_memory_usage_mb = (this_process_mem_usage as f64 / 1000.0 / 1000.0).round();
        let external_mem_usage_mb = (external_mem_usage as f64 / 1000.0 / 1000.0).round();

        assert_eq!(this_memory_usage_mb, external_mem_usage_mb,
            "current & external process should be using the same amount of memory because they are the same process");
     }

     #[test]
     fn max_allowed_cores_should_be_bullied_into_range() {
        let mut sys_info = get_sys_info();

        let total_cores = System::physical_core_count()
            .expect("could not determine core count") as u32;

        sys_info.set_max_allowed_cores(0);
        assert_eq!(sys_info.max_allowed_cores(), 1, "should not be lower than 1");

        sys_info.set_max_allowed_cores(total_cores + 1);
        assert_eq!(sys_info.max_allowed_cores(), total_cores, "should not be more than system's total cores");
     }
}
