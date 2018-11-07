use std::path::Path;
use super::{Result,calculate_time_difference};

/// Measurement of cpu stats at a certain time
#[derive(Debug,PartialEq)]
pub struct CpuMeasurement {
    pub precise_time_ns: u64,
    pub stat: CpuStat
}

impl CpuMeasurement {
    /// Calculate the cpu stats based on this measurement and a measurement in the future.
    /// It is advisable to make the next measurement roughly a minute from this one for the
    /// most reliable result.
    pub fn calculate_per_minute(&self, next_measurement: &CpuMeasurement) -> Result<CpuStat> {
        let time_difference = calculate_time_difference(self.precise_time_ns, next_measurement.precise_time_ns)?;

        Ok(CpuStat {
            user: super::time_adjusted("user", next_measurement.stat.user, self.stat.user, time_difference)?,
            nice: super::time_adjusted("nice", next_measurement.stat.nice, self.stat.nice, time_difference)?,
            system: super::time_adjusted("system", next_measurement.stat.system, self.stat.system, time_difference)?,
            idle: super::time_adjusted("idle", next_measurement.stat.idle, self.stat.idle, time_difference)?,
            iowait: super::time_adjusted("iowait", next_measurement.stat.iowait, self.stat.iowait, time_difference)?,
            irq: super::time_adjusted("irq", next_measurement.stat.irq, self.stat.irq, time_difference)?,
            softirq: super::time_adjusted("softirq", next_measurement.stat.softirq, self.stat.softirq, time_difference)?,
            steal: super::time_adjusted("steal", next_measurement.stat.steal, self.stat.steal, time_difference)?,
            guest: super::time_adjusted("guest", next_measurement.stat.guest, self.stat.guest, time_difference)?,
            guestnice: super::time_adjusted("guestnice", next_measurement.stat.guestnice, self.stat.guestnice, time_difference)?
        })
    }
}

/// Cpu stats for a minute
#[derive(Debug,PartialEq)]
pub struct CpuStat {
    pub user: u64,
    pub nice: u64,
    pub system: u64,
    pub idle: u64,
    pub iowait: u64,
    pub irq: u64,
    pub softirq: u64,
    pub steal: u64,
    pub guest: u64,
    pub guestnice: u64
}

impl CpuStat {
    /// Calculate the weight of the various components in percentages
    pub fn in_percentages(&self) -> CpuStatPercentages {
        let idlealltime = self.idle + self.iowait;
        let systemalltime = self.system + self.irq + self.softirq;
        let virtualtime = self.guest + self.guestnice;
        let total = (self.user + self.nice + systemalltime + idlealltime + self.steal + virtualtime) as f64;

        CpuStatPercentages {
            user: Self::percentage_of_total(self.user, total),
            nice: Self::percentage_of_total(self.nice, total),
            system: Self::percentage_of_total(self.system, total),
            idle: Self::percentage_of_total(self.idle, total),
            iowait: Self::percentage_of_total(self.iowait, total),
            irq: Self::percentage_of_total(self.irq, total),
            softirq: Self::percentage_of_total(self.softirq, total),
            steal: Self::percentage_of_total(self.steal, total),
            guest: Self::percentage_of_total(self.guest, total),
            guestnice: Self::percentage_of_total(self.guestnice, total)
        }
    }

    fn percentage_of_total(value: u64, total: f64) -> f32 {
        (value as f64 / total * 100.0) as f32
    }
}

/// Cpu stats converted to percentages
#[derive(Debug,PartialEq)]
pub struct CpuStatPercentages {
    pub user: f32,
    pub nice: f32,
    pub system: f32,
    pub idle: f32,
    pub iowait: f32,
    pub irq: f32,
    pub softirq: f32,
    pub steal: f32,
    pub guest: f32,
    pub guestnice: f32
}

#[cfg(target_os = "linux")]
pub fn read() -> Result<CpuMeasurement> {
    // columns: user nice system idle iowait irq softirq
    os::read_and_parse_proc_stat(&Path::new("/proc/stat"))
}

#[cfg(target_os = "linux")]
mod os {
    use std::path::Path;
    use std::io::BufRead;
    use time;
    use super::super::{Result,file_to_buf_reader,parse_u64,path_to_string};
    use super::{CpuMeasurement,CpuStat};
    use error::ProbeError;

    pub fn read_and_parse_proc_stat(path: &Path) -> Result<CpuMeasurement> {
        let mut line = String::new();
        let mut reader = file_to_buf_reader(path)?;
        let time = time::precise_time_ns();
        reader.read_line(&mut line).map_err(|e| ProbeError::IO(e, path_to_string(path)))?;

        let stats: Vec<&str> = line
            .split_whitespace()
            .skip(1)
            .collect();

        let length = stats.len();
        if length < 5 {
            return Err(ProbeError::UnexpectedContent("Incorrect number of stats".to_owned()));
        }

        let mut usertime = parse_u64(stats[0])?;
        let mut nicetime = parse_u64(stats[1])?;
        let guest = parse_u64(*stats.get(8).unwrap_or(&"0"))?;
        let guestnice = parse_u64(*stats.get(9).unwrap_or(&"0"))?;
        usertime = usertime - guest;
        nicetime = nicetime - guestnice;

        Ok(CpuMeasurement {
            precise_time_ns: time,
            stat: CpuStat {
                user: usertime,
                nice: nicetime,
                system: parse_u64(stats[2])?,
                idle: parse_u64(stats[3])?,
                iowait: parse_u64(stats[4])?,
                irq: parse_u64(*stats.get(5).unwrap_or(&"0"))?,
                softirq: parse_u64(*stats.get(6).unwrap_or(&"0"))?,
                steal: parse_u64(*stats.get(7).unwrap_or(&"0"))?,
                guest: guest,
                guestnice: guestnice
            }
        })
    }
}

#[cfg(test)]
mod test {
    use super::{CpuMeasurement,CpuStat,CpuStatPercentages};
    use super::os::read_and_parse_proc_stat;
    use std::path::Path;
    use error::ProbeError;

    #[test]
    fn test_read_cpu_measurement() {
        let measurement = read_and_parse_proc_stat(&Path::new("fixtures/linux/cpu/proc_stat")).unwrap();
        assert_eq!(measurement.stat.user, 8);
        assert_eq!(measurement.stat.nice, 2);
        assert_eq!(measurement.stat.system, 7);
        assert_eq!(measurement.stat.idle, 6);
        assert_eq!(measurement.stat.iowait, 5);
        assert_eq!(measurement.stat.irq, 4);
        assert_eq!(measurement.stat.softirq, 3);
        assert_eq!(measurement.stat.steal, 1);
        assert_eq!(measurement.stat.guest, 2);
        assert_eq!(measurement.stat.guestnice, 1);
    }

    #[test]
    fn test_read_cpu_measurement_from_partial() {
        let measurement = read_and_parse_proc_stat(&Path::new("fixtures/linux/cpu/proc_stat_partial")).unwrap();
        assert_eq!(measurement.stat.user, 10);
        assert_eq!(measurement.stat.nice, 3);
        assert_eq!(measurement.stat.system, 7);
        assert_eq!(measurement.stat.idle, 6);
        assert_eq!(measurement.stat.iowait, 5);
        assert_eq!(measurement.stat.irq, 0);
        assert_eq!(measurement.stat.softirq, 0);
        assert_eq!(measurement.stat.steal, 0);
        assert_eq!(measurement.stat.guest, 0);
        assert_eq!(measurement.stat.guestnice, 0);
    }

    #[test]
    fn test_wrong_path() {
        match read_and_parse_proc_stat(&Path::new("bananas")) {
            Err(ProbeError::IO(_, _)) => (),
            r => panic!("Unexpected result: {:?}", r)
        }
    }

    #[test]
    fn test_read_and_parse_proc_stat_incomplete() {
        match read_and_parse_proc_stat(&Path::new("fixtures/linux/cpu/proc_stat_incomplete")) {
            Err(ProbeError::UnexpectedContent(_)) => (),
            r => panic!("Unexpected result: {:?}", r)
        }
    }

    #[test]
    fn test_read_and_parse_proc_stat_garbage() {
        let path = Path::new("fixtures/linux/cpu/proc_stat_garbage");
        match read_and_parse_proc_stat(&path) {
            Err(ProbeError::UnexpectedContent(_)) => (),
            r => panic!("Unexpected result: {:?}", r)
        }
    }

    #[test]
    fn test_calculate_per_minute_wrong_times() {
        let measurement1 = CpuMeasurement {
            precise_time_ns: 90_000_000_000,
            stat: CpuStat {
                user: 0,
                nice: 0,
                system: 0,
                idle: 0,
                iowait: 0,
                irq: 0,
                softirq: 0,
                steal: 0,
                guest: 0,
                guestnice: 0
            }
        };

        let measurement2 = CpuMeasurement {
            precise_time_ns: 60_000_000_000,
            stat: CpuStat {
                user: 0,
                nice: 0,
                system: 0,
                idle: 0,
                iowait: 0,
                irq: 0,
                softirq: 0,
                steal: 0,
                guest: 0,
                guestnice: 0
            }
        };

        match measurement1.calculate_per_minute(&measurement2) {
            Err(ProbeError::InvalidInput(_)) => (),
            r => panic!("Unexpected result: {:?}", r)
        }
    }

    #[test]
    fn test_calculate_per_minute_full_minute() {
        let measurement1 = CpuMeasurement {
            precise_time_ns: 60_000_000_000,
            stat: CpuStat {
                user: 1000,
                nice: 1100,
                system: 1200,
                idle: 1300,
                iowait: 1400,
                irq: 50,
                softirq: 10,
                steal: 20,
                guest: 200,
                guestnice: 100
            }
        };

        let measurement2 = CpuMeasurement {
            precise_time_ns: 120_000_000_000,
            stat: CpuStat {
                user: 1006,
                nice: 1106,
                system: 1206,
                idle: 1306,
                iowait: 1406,
                irq: 56,
                softirq: 16,
                steal: 26,
                guest: 206,
                guestnice: 106
            }
        };

        let expected = CpuStat {
            user: 6,
            nice: 6,
            system: 6,
            idle: 6,
            iowait: 6,
            irq: 6,
            softirq: 6,
            steal: 6,
            guest: 6,
            guestnice: 6
        };

        let stat = measurement1.calculate_per_minute(&measurement2).unwrap();

        assert_eq!(stat, expected);
    }

    #[test]
    fn test_calculate_per_minute_partial_minute() {
        let measurement1 = CpuMeasurement {
            precise_time_ns: 60_000_000_000,
            stat: CpuStat {
                user: 1000,
                nice: 1100,
                system: 1200,
                idle: 1300,
                iowait: 1400,
                irq: 50,
                softirq: 10,
                steal: 20,
                guest: 200,
                guestnice: 100
            }
        };

        let measurement2 = CpuMeasurement {
            precise_time_ns: 90_000_000_000,
            stat: CpuStat {
                user: 1006,
                nice: 1106,
                system: 1206,
                idle: 1306,
                iowait: 1406,
                irq: 56,
                softirq: 16,
                steal: 26,
                guest: 206,
                guestnice: 106
            }
        };

        let expected = CpuStat {
            user: 3,
            nice: 3,
            system: 3,
            idle: 3,
            iowait: 3,
            irq: 3,
            softirq: 3,
            steal: 3,
            guest: 3,
            guestnice: 3
        };

        let stat = measurement1.calculate_per_minute(&measurement2).unwrap();

        assert_eq!(stat, expected);
    }

    #[test]
    fn test_calculate_per_minute_values_lower() {
        let measurement1 = CpuMeasurement {
            precise_time_ns: 60_000_000_000,
            stat: CpuStat {
                user: 1000,
                nice: 1100,
                system: 1200,
                idle: 1300,
                iowait: 1400,
                irq: 50,
                softirq: 10,
                steal: 20,
                guest: 200,
                guestnice: 100
            }
        };

        let measurement2 = CpuMeasurement {
            precise_time_ns: 90_000_000_000,
            stat: CpuStat {
                user: 106,
                nice: 116,
                system: 126,
                idle: 136,
                iowait: 146,
                irq: 56,
                softirq: 16,
                steal: 26,
                guest: 206,
                guestnice: 106
            }
        };

        match measurement1.calculate_per_minute(&measurement2) {
            Err(ProbeError::UnexpectedContent(_)) => (),
            r => panic!("Unexpected result: {:?}", r)
        }
    }

    #[test]
    fn test_in_percentages() {
        let stat = CpuStat {
            user: 450,
            nice: 70,
            system: 100,
            idle: 100,
            iowait: 120,
            irq: 10,
            softirq: 20,
            steal: 50,
            guest: 50,
            guestnice: 30
        };

        let expected = CpuStatPercentages {
            user: 45.0,
            nice: 7.0,
            system: 10.0,
            idle: 10.0,
            iowait: 12.0,
            irq: 1.0,
            softirq: 2.0,
            steal: 5.0,
            guest: 5.0,
            guestnice: 3.0
        };

        assert_eq!(stat.in_percentages(), expected);
    }

    #[test]
    fn test_in_percentages_fractions() {
        let stat = CpuStat {
            user: 445,
            nice: 65,
            system: 100,
            idle: 100,
            iowait: 147,
            irq: 1,
            softirq: 2,
            steal: 50,
            guest: 55,
            guestnice: 35
        };

        let expected = CpuStatPercentages {
            user: 44.5,
            nice: 6.5,
            system: 10.0,
            idle: 10.0,
            iowait: 14.7,
            irq: 0.1,
            softirq: 0.2,
            steal: 5.0,
            guest: 5.5,
            guestnice: 3.5
        };

        assert_eq!(stat.in_percentages(), expected);
    }

    #[test]
    fn test_in_percentages_integration() {
        let mut measurement1 = read_and_parse_proc_stat(&Path::new("fixtures/linux/cpu/proc_stat_1")).unwrap();
        measurement1.precise_time_ns = 60_000_000_000;
        let mut measurement2 = read_and_parse_proc_stat(&Path::new("fixtures/linux/cpu/proc_stat_2")).unwrap();
        measurement2.precise_time_ns = 120_000_000_000;

        let stat = measurement1.calculate_per_minute(&measurement2).unwrap();
        let in_percentages = stat.in_percentages();

        // Rounding in the floating point calculations can vary, so check if this
        // is in the correct range.

        assert!(in_percentages.user > 4.0);
        assert!(in_percentages.user < 5.0);

        assert!(in_percentages.nice < 0.21);
        assert!(in_percentages.nice > 0.2);

        assert!(in_percentages.system > 1.51);
        assert!(in_percentages.system < 1.52);

        assert!(in_percentages.idle > 92.3);
        assert!(in_percentages.idle < 92.4);

        assert!(in_percentages.iowait < 0.05);
        assert!(in_percentages.iowait > 0.04);

        assert!(in_percentages.irq < 0.5);
        assert!(in_percentages.irq > 0.49);

        assert!(in_percentages.softirq > 0.028);
        assert!(in_percentages.softirq < 0.029);

        assert!(in_percentages.steal < 0.41);
        assert!(in_percentages.steal > 0.40);

        assert!(in_percentages.guest < 0.41);
        assert!(in_percentages.guest > 0.40);

        assert!(in_percentages.guestnice < 0.21);
        assert!(in_percentages.guestnice > 0.20);

        // The total of these values should be 100.
        let idlealltime = in_percentages.idle + in_percentages.iowait;
        let systemalltime = in_percentages.system + in_percentages.irq + in_percentages.softirq;
        let virtualtime = in_percentages.guest + in_percentages.guestnice;
        let total = (in_percentages.user + in_percentages.nice + systemalltime + idlealltime + in_percentages.steal + virtualtime) as f64;

        assert!(total < 100.1);
        assert!(total > 99.9);
    }
}
