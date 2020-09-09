include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

use crate::RadioRx;

use failure::{format_err, Error};
use num::complex::Complex;

use std::ffi::CString;

/// Generate a new multi_usrp object. `args` gives the address of the USRP, `rate` is the
/// number of samples per second, `freq` is the requested center frequency in Hz, `gain` is the
/// gain in dB (uncaliberated units), and `bw` is the analog bandwidth of the receiver in Hz, `tx`
/// tells us whether to configure this as a transmitter or a receiver
fn new_generic(args: &str, rate: u64, freq: f64, gain: f64, bw: f64, tx: bool) -> *mut MultiUsrp {
    // So that CString is not deallocated too early
    let args_ptr = CString::new(args).unwrap().into_raw();
    unsafe {
        let res = new_usrp(args_ptr, rate as f64, freq, gain, bw, tx);
        // So that it is deallocated and doesn't leak memory
        let _tmp = CString::from_raw(args_ptr);
        return res;
    }
}

/// Various ways the usrp can take its clock
#[allow(dead_code)]
pub enum ClockSource {
    Internal,
    /// Make this one's clock a slave to the one it is connected to, via a MIMO cable (if
    /// available)
    Mimo,
    /// From an external (10MHz, I think) clock source
    External,
    /// A GPS disciplined clock (if available)
    Gpsdo,
}

/// Set the clock source of a usrp for the given motherboard
fn set_clock_source_wrapper(
    usrp: *mut MultiUsrp,
    clk_src: ClockSource,
    mboard: usize,
) -> Result<(), Error> {
    let code = match clk_src {
        ClockSource::Internal => 0,
        ClockSource::Mimo => 1,
        ClockSource::External => 2,
        ClockSource::Gpsdo => 3,
    };
    let err_code = unsafe { set_clock_source(usrp, code, mboard) };
    if err_code < 0 {
        return Err(format_err!(
            "Error in setting clock source of tx: {}",
            err_code
        ));
    }
    Ok(())
}

/// A new Tx USRP. `args` gives the address of the USRP, `rate` is the number of samples per
/// second, `freq` is the requested center frequency in Hz, `gain` is the gain in dB (uncaliberated
/// units), and `bw` is the analog bandwidth of the receiver in Hz.
pub fn new_tx_usrp(
    args: &str,
    rate: u64,
    freq: f64,
    gain: f64,
    bw: f64,
    clk_src: ClockSource,
) -> Result<UsrpTxSingleStream, Error> {
    let usrp = new_generic(args, rate, freq, gain, bw, true);
    set_clock_source_wrapper(usrp, clk_src, 0)?;
    Ok(UsrpTxSingleStream {
        usrp,
        streamer: None,
        buf: Vec::new(),
    })
}

/// A new Rx USRP. `args` gives the address of the USRP, `rate` is the number of samples per
/// second, `freq` is the requested center frequency in Hz, `gain` is the gain in dB
/// (uncaliberated units), and `bw` is the analog bandwidth of the receiver in Hz. If `print_samps`
/// is true, prints every n^th sample
pub fn new_rx_usrp(
    args: &str,
    rate: u64,
    freq: f64,
    gain: f64,
    bw: f64,
    print_samples: Option<usize>,
    clk_src: ClockSource,
) -> Result<UsrpRxSingleStream, Error> {
    let usrp = new_generic(args, rate, freq, gain, bw, false);
    set_clock_source_wrapper(usrp, clk_src, 0)?;
    Ok(UsrpRxSingleStream {
        usrp,
        print_samples,
        streamer: None,
        buf: Vec::new(),
        tot_num_samps: 0,
        ret_buf: Vec::new(),
    })
}

/// A single channel receive usrp streamer
pub struct UsrpRxSingleStream {
    usrp: *mut MultiUsrp,
    /// If `Some`, print one out of every `n` samples
    print_samples: Option<usize>,
    /// The streamer may or may not have been initialized
    streamer: Option<*mut RxStream>,
    /// Buffer for temporarily storing the values
    buf: Vec<f32>,
    /// Total number of samples returned so far
    tot_num_samps: u64,
    /// We return pointers to this buffer to give data back to the caller
    ret_buf: Vec<Complex<f32>>,
}

/// A single channel transmit usrp streamer
pub struct UsrpTxSingleStream {
    usrp: *mut MultiUsrp,
    /// The streamer may or may not have been initialized
    streamer: Option<*mut TxStream>,
    /// Buffer for temporarily storing values
    buf: Vec<f32>,
}

/// Should be fine, but who knows really?
unsafe impl Send for UsrpRxSingleStream {}
unsafe impl Send for UsrpTxSingleStream {}

#[allow(dead_code)]
impl UsrpRxSingleStream {
    /// Get the gain in (uncalibrated) dB
    pub fn get_gain(&mut self) -> f64 {
        unsafe { get_rx_gain(self.usrp, 0) }
    }

    /// Set the gain in (uncalibrated) dB
    pub fn set_gain(&mut self, gain: f64) {
        unsafe {
            set_rx_gain(self.usrp, gain);
        }
    }
}

impl RadioRx for UsrpRxSingleStream {
    fn set_time_now(&mut self, now: f64) {
        unsafe {
            set_time_now(self.usrp, now);
        }
    }

    /// Receive at-most `len` samples from the USRP. Returns the exactly `len` samples, the
    /// timestamp (in microseconds) of the first sample
    fn recv<'a>(&'a mut self, len: usize) -> Result<(&'a [Complex<f32>], u64), Error> {
        // TODO: This additional copy is no longer required as the C wrapper itself does one copy
        self.buf.resize(len * 2, 0.);

        // If streamer doesn't exist, create it now
        if self.streamer.is_none() {
            self.streamer = Some(unsafe { get_rx_streamer(self.usrp) });
        }

        // Call the C-wrapper function
        let returned: i64 = unsafe {
            let buf_arr: *mut f32 = self.buf.as_mut_ptr();
            recv(self.streamer.unwrap(), buf_arr, len, 1)
        };

        // See if we had an error
        if returned < 0 {
            return Err(format_err!("Error in receiving. Got code: {}", returned));
        }
        let time_spec = returned as u64;

        // Copy data into a Complex<f32> array
        self.ret_buf.resize(len, Complex::new(0., 0.));
        self.ret_buf.clear();
        for i in 0..len {
            self.ret_buf
                .push(Complex::new(self.buf[2 * i], self.buf[2 * i + 1]));
        }

        if let Some(n) = self.print_samples {
            let mut t = self.tot_num_samps;
            for x in &self.ret_buf {
                if t % n as u64 == 0 {
                    println!("Sample: {} {}", x.norm(), x.arg());
                }
                t += 1;
            }
        }

        // Keep track of the number of samples returned so far
        self.tot_num_samps += len as u64;

        Ok((&self.ret_buf, time_spec))
    }

    fn tot_num_samps(&self) -> u64 {
        self.tot_num_samps
    }

    fn set_freq(&mut self, freq: f64) -> Result<(), Error> {
        unsafe { set_rx_freq(self.usrp, freq); }
        Ok(())
    }
}

#[allow(dead_code)]
impl RadioRx for UsrpTxSingleStream {
    /// Send the given samples through the transmit USRP
    fn send(&mut self, data: &[Complex<f32>]) -> Result<(), Error> {
        // Copy data into temporary buffer after making sure it is large enough
        self.buf.resize(2 * data.len(), 0.);
        for i in 0..data.len() {
            self.buf[2 * i] = data[i].re;
            self.buf[2 * i + 1] = data[i].im;
        }

        // If streamer doesn't exist, create one now
        if self.streamer.is_none() {
            self.streamer = Some(unsafe { get_tx_streamer(self.usrp) });
        }

        // Send the data
        let err_code = unsafe { send(self.streamer.unwrap(), self.buf.as_mut_ptr(), data.len()) };

        // Interpret error code
        if err_code < 0 {
            return Err(format_err!(
                "Error in transmission. Error code: {}",
                err_code
            ));
        }
        return Ok(());
    }

    /// Set the center frequency (in Hz)
    fn set_freq(&mut self, freq: f64) -> Result<(), Error> {
        unsafe { set_tx_freq(self.usrp, freq); };
        Ok(())
    }
}

impl Drop for UsrpRxSingleStream {
    fn drop(&mut self) {
        unsafe {
            if let Some(streamer) = self.streamer {
                delete_rx_stream(streamer);
            }
            delete_usrp(self.usrp);
        }
    }
}

impl Drop for UsrpTxSingleStream {
    fn drop(&mut self) {
        unsafe {
            if let Some(streamer) = self.streamer {
                delete_tx_stream(streamer);
            }
            delete_usrp(self.usrp);
        }
    }
}
