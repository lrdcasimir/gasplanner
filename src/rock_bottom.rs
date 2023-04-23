use std::convert::TryFrom;
use std::vec;
use std::string::String;
use conv::prelude::ValueFrom;
use conv;

const SAFETY_STOP_DEPTH: f64 = 4.5;
const ASCENT_RATE: f64 = 9.14;
const SAFETY_STOP_MINUTES: f64 = 3.0;

#[derive(Clone)]
struct Tank {
    service_pressure: u16,
    capacity_cuft: f64,
    gauge_pressure: f64,
    f_o2: f64,
    f_n2: f64,
}

struct Diver {
    name: String,
    rmv: f64,
    kit: Kit,
}

struct Kit {
    tanks: vec::Vec<Tank>
}

impl Tank {
    fn gas_volume_cuft(&self) -> Option<f64> {
        self.tank_factor().and_then(|tank_factor| {
            Some(self.gauge_pressure * tank_factor)
        })
    }

    fn tank_factor(&self) -> Option<f64> {
        f64::try_from(self.service_pressure).and_then(|sp| {
            Ok(self.capacity_cuft / sp)
        }).ok()
    }

    fn pO2(&self, depth_m: f64) -> f64 {
        atmospheres(depth_m) * self.f_o2
    }

    fn breathable_at(&self, depth_m: f64) -> bool {
        self.pO2(depth_m) <= 1.4
    }

    fn with_volume(self, volume: f64) -> Option<Tank> {
        self.tank_factor().and_then(|tank_factor| {
            Some(Tank{
                service_pressure: self.service_pressure,
                capacity_cuft: self.capacity_cuft,
                f_o2: self.f_o2,
                f_n2: self.f_n2,
                gauge_pressure: volume / tank_factor
            })
        }) 
    }

    fn add_volume(self, volume: f64) -> Option<Tank> {
        self.gas_volume_cuft().and_then(|gv| {
            self.with_volume(gv + volume)
        })
    }
}

impl Diver {
    fn rock_bottom_pressure_rec(&self, depth_m: f64) -> Result<Vec<Tank>, &'static str> {
        let ascent_ata = atmospheres((depth_m - SAFETY_STOP_DEPTH) / 2.0);
        let bottom_ata = atmospheres(depth_m);
        
        let ascent_minutes = depth_m / ASCENT_RATE;  // 30ft / min
        let problem_gas = self.rmv * 2.0 * bottom_ata * 4.0;
        let ascent_gas = ascent_ata * ascent_minutes * self.rmv * 2.0;
        let stop_gas = atmospheres(SAFETY_STOP_DEPTH) * SAFETY_STOP_MINUTES * self.rmv * 2.0;
        
        
        let bottom_tanks = self.kit.tanks
            .clone()
            .into_iter()
            .filter(|t| t.breathable_at(depth_m))
            .collect::<Vec<Tank>>();
        let mut stop_tanks = self.kit.tanks
            .clone()
            .into_iter()
            .filter(|t| {
                t.breathable_at(SAFETY_STOP_DEPTH) && !t.breathable_at(depth_m)
            }).collect::<Vec<Tank>>();
        let bottom_tanks = divide_gas_among(bottom_tanks, problem_gas + ascent_gas,  &mut Tank::with_volume)
            .expect("Failed to allocate gas to tanks.");
        let all_tanks = match bottom_tanks {
            Some(mut bt) => {
                bt.append(&mut stop_tanks);
                Ok(bt)
            },
            None => Err("No valid bottom tanks")
        }?;
        match divide_gas_among(all_tanks, stop_gas, &mut Tank::add_volume).expect("OOF") {
            Some(t) => Ok(t),
            None => Err("BIG OOF.")
        }

    }
}

fn divide_gas_among<F>(tanks: Vec<Tank>, needed_gas: f64,  volume_allocator: &mut F) -> Result<Option<Vec<Tank>>, conv::PosOverflow<usize>>
where F: FnMut(Tank, f64) -> Option<Tank> {
    f64::value_from(tanks.len()).and_then(|tank_count| {
        let gas_per_ascent_tank = needed_gas / tank_count;
        Ok(tanks
            .into_iter()
            .map(|t| { volume_allocator(t, gas_per_ascent_tank)})
            .collect::<Option<Vec<_>>>())
    })
}

fn atmospheres(depth_m: f64) -> f64 {
    1.0 + depth_m / 10.0
}

#[cfg(test)]
mod tests {
    use crate::rock_bottom::{Tank, Diver, Kit};
    
    #[test]
    fn test_tank_volume() {
        let tank1 = Tank {
            service_pressure: 3442,
            capacity_cuft: 101.3,
            gauge_pressure: 750.0,
            f_o2: 0.21,
            f_n2: 0.79,
        };
        let vol = tank1.gas_volume_cuft();
        match vol {
            Some(v) => assert_float_relative_eq!(v , 22.0729, 0.03),
            None => panic!("Values don't convert cleanly")
        };       
    }

    #[test]
    fn test_tank_breathable() {
        let t50 = Tank {
            service_pressure: 3442,
            capacity_cuft: 101.3,
            gauge_pressure: 3200.0,
            f_o2: 0.5,
            f_n2: 0.5,
        };
        assert_eq!(t50.breathable_at(19.0), false);
        assert_eq!(t50.breathable_at(18.0), true);
    }

    #[test]
    fn test_rock_bottom() {
        let d = Diver {
            name: String::from("Tyler"),
            rmv: 0.7,
            kit: Kit{
                tanks: vec![Tank{
                    service_pressure: 3442,
                    capacity_cuft: 101.3,
                    gauge_pressure: 750.0,
                    f_o2: 0.21,
                    f_n2: 0.79,
                }]
            }
        };
        let rb_tanks = d.rock_bottom_pressure_rec(30.0)
        .expect("Rock bottom shouldn't fail with a rec diving config");
        assert_eq!(rb_tanks.len(), 1);
        ()
    }

} 