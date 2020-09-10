mod simulator;
#[cfg(feature = "rpi")]
mod usrp;

pub use simulator::{create_simulator, RadioSimulatorConfig, SimulatedRadioRx, SimulatedRadioTx};
#[cfg(feature = "rpi")]
pub use usrp::{new_rx_usrp, new_tx_usrp, ClockSource, UsrpRxSingleStream, UsrpTxSingleStream};

use failure::Error;
use num::complex::Complex;

/// Receive sample from real or simulated radio
pub trait RadioRx {
    fn set_time_now(&mut self, now: f64);
    /// Return a buffer containing *exactly* `len` samples, the timestamp (in microseconds) of the
    /// first sample. This buffer isn't guaranteed to be constant across multiple calls to `recv`,
    /// since the same piece of memory may be used over and over (and because Rust is awesome, the
    /// type system will automatically capture this error at compile time)
    fn recv<'a>(&'a mut self, len: usize) -> Result<(&'a [Complex<f32>], u64), Error>;
    /// Returns count of the number of samples returned since the beginning of the struct
    fn tot_num_samps(&self) -> u64;
    /// Change the center frequency. The oscillator might take some time to settle to the new
    /// frequency. Ideally, we should check lo_lock before assuming the change is complete, but
    /// waiting for a bit could also work
    fn set_freq(&mut self, freq: f64) -> Result<(), Error>;
}

pub trait RadioTx {
    fn send(&mut self, data: &[Complex<f32>]) -> Result<(), Error>;
    fn set_freq(&mut self, freq: f64) -> Result<(), Error>;
}
