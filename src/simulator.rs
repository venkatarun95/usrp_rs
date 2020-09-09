//! Test on a simulation of a USRP rather than a real device. `SimulatedRadioRx` and
//! `SimulatedRadioTx` are generated based on parameters in `RadioSimulatorConfig` by
//! `create_simulator`.

use crate::{RadioRx, RadioTx};
use failure::Error;
use num::{Complex, Zero};
use rand::{distributions::Distribution, Rng};
use rand_distr::Normal;
use std::collections::VecDeque;
use std::f32::consts::PI;
use std::sync::mpsc::{channel, Receiver, Sender};

pub struct RadioSimulatorConfig {
    /// To simulate the fact that the Tx and Rx start producing samples at different times, the Rx
    /// will produce N pure noise values before including signal from the Tx. Here, N is sampled
    /// uniformly from [0, start_time_offset]. Units are in samples
    max_start_time_offset: u64,
    /// Sample rate (in samples/sec)
    samp_rate: u64,
    /// Frequency of operation at which we start. Can be editied *ONLY* using `RadioRx::set_freq`
    start_freq: f32,
    /// The Tx and Rx will start with a random CFO in [-cfo, cfo] radians/sample. 1 radian/sec is
    /// equivalent to a CFO of (sample_rate / (2\pi) Hz). In addition, the CFO may drift as a
    /// random walk
    max_cfo: f32,
    /// In addition to the random starting point, the CFO will exhibit a bounded (by `max_cfo`)
    /// random walk with the given standard deviation. This models cfo drift. For real clocks,
    /// it is probably the case that `cfo_drift` << `phase_noise` << `max_cfo`
    cfo_drift: f32,
    /// In addition to the cfo, the phase shift per sample will have a random component that is
    /// normally distributed with a standard deviation of phase_noise radians per sample. In the
    /// real world it is probably the case that `phase_noise` << `max_cfo`
    phase_noise: f32,
    /// Standard deviation of the gaussian noise that will be added to the signal
    noise: f32,
    /// The multipath components (in addition to 0 delay of course) in secs (hence the number of
    /// samples offset changes with `freq`). The complex component specifies the attenuation and
    /// phase offset
    multipath: Vec<(f32, Complex<f32>)>,
}

pub struct SimulatedRadioRx<R: Rng> {
    config: RadioSimulatorConfig,
    rng: R,
    /// Samples coming in from the Tx
    receiver: Receiver<Complex<f32>>,
    /// The current CFO per sample (may drift as a random walk)
    cur_cfo: Complex<f32>,
    /// Cumulative phase offset so far due to cfo (starts off with a random phase)
    cum_phase_offset: Complex<f32>,
    /// Number of samples before we start returning samples from the Tx
    samps_before_start: u64,
    /// Number of samples we have sent out of Rx so far
    tot_num_samps: u64,
    /// The current frequency at which we are receiving. Calling SimulatedRadioRx::set_freq sets
    /// the frequency immediately for both the tx and rx. We don't model imperfections here
    cur_freq: f32,
    /// The maximum multipath delay (in config.multipath) in secs
    max_multipath: f32,
    /// Past samples so we can calculate multipath effects
    past_samps: VecDeque<Complex<f32>>,
    /// Buffer to store samples for returning via `RadioRx::recv`
    buf: Vec<Complex<f32>>,
}

pub struct SimulatedRadioTx {
    sender: Sender<Complex<f32>>,
}

impl<R: Rng> SimulatedRadioRx<R> {
    /// Drift the CFO as a bounded random walk and update `cum_phase_offset`
    fn update_cum_phase_offset(&mut self) {
        // CFO drift random walk
        let distr1 = Normal::new(0., self.config.cfo_drift as f64).unwrap();
        self.cur_cfo *= Complex::new(0., distr1.sample(&mut self.rng) as f32).exp();

        // Bound the CFO random walk
        if self.cur_cfo.arg() < -self.config.max_cfo {
            self.cur_cfo = Complex::new(0., -self.config.max_cfo).exp();
        } else if self.cur_cfo.arg() > self.config.max_cfo {
            self.cur_cfo = Complex::new(0., self.config.max_cfo);
        }
        self.cur_cfo /= self.cur_cfo.norm();

        // Phase noise
        let distr2 = Normal::new(0., self.config.phase_noise as f64).unwrap();
        self.cum_phase_offset *= Complex::new(0., distr2.sample(&mut self.rng) as f32).exp();

        // Update the cumulative phase offset due to CFO
        self.cum_phase_offset *= self.cur_cfo;
        self.cum_phase_offset /= self.cum_phase_offset.norm();
    }

    /// Return the next sample
    fn next_sample(&mut self) -> Result<Complex<f32>, Error> {
        if self.tot_num_samps < self.samps_before_start {
            Ok(Complex::zero())
        } else {
            let mut samp = self.receiver.recv()?;

            // Record past samples
            assert!(self.max_multipath * self.config.samp_rate < 1e6); // Keep it sane!
            let max_past_samples = (self.max_multipath * self.config.samp_rate).ceil() as usize;
            self.past_samps.push_front(samp);
            while self.past_samps.len() >= max_past_samples {
                self.past_samps.pop_back();
            }

            // Include multipath effects
            for (d, attn) in &self.config.multipath {
                let i = (d * self.cur_freq).round() as usize;
                // Phase factor that accumulates assuming that radio travelled for d * (speed of
                // light) distance
                //let dist_phase = Complex::new(0., );

                // `self.past_samps` may be too short if it hasn't accumulated samples from the
                // start yet or `self.freq` increased recently
                if i < self.past_samps.len() {
                    samp += attn * self.past_samps[i];
                }
            }

            // CFO
            self.update_cum_phase_offset();
            samp *= self.cum_phase_offset;

            // Noise
            let distr = Normal::new(0., self.config.noise)?;
            samp += Complex::new(distr.sample(&mut self.rng), distr.sample(&mut self.rng));
            Ok(samp)
        }
    }
}

impl<R: Rng> RadioRx for SimulatedRadioRx<R> {
    fn set_time_now(&mut self, _now: f64) {}
    fn tot_num_samps(&self) -> u64 {
        self.tot_num_samps
    }

    fn recv<'a>(&'a mut self, len: usize) -> Result<(&'a [Complex<f32>], u64), Error> {
        if self.buf.len() < len {
            self.buf.resize(len, Complex::zero());
        }

        for i in 0..len {
            self.buf[i] = self.next_sample()?;
        }

        Ok((&self.buf, len as u64))
    }

    fn set_freq(&mut self, freq: f64) -> Result<(), Error> {
        self.cur_freq = freq as f32;
        Ok(())
    }
}

impl RadioTx for SimulatedRadioTx {
    fn send(&mut self, data: &[Complex<f32>]) -> Result<(), Error> {
        for samp in data {
            self.sender.send(*samp)?;
        }
        Ok(())
    }

    fn set_freq(&mut self, _freq: f64) -> Result<(), Error> {
        Ok(())
    }
}

use float_ord::FloatOrd;

pub fn create_simulator(
    config: &RadioSimulatorConfig,
) -> (SimulatedRadioTx, SimulatedRadioRx<rand::ThreadRng>) {
    let (sender, receiver) = channel();
    let rng = rand::thread_rng();
    let max_multipath = config
        .multipath
        .iter()
        .map(|x| x.0.into::<FloatOrd>())
        .max()
        .into::<f32>();

    let rx = SimulatedRadioRx {
        config: config.clone(),
        rng,
        receiver,
        cur_cfo: 2. * rng.gen() * config.max_cfo - config.max_cfo,
        cum_phase_offset: Complex::from_polar(&1., &(rng.gen() * 2. * PI)),
        samps_before_start: rng.gen() % config.max_start_time_offset,
        tot_num_samps: 0,
        cur_freq: config.start_freq,
        max_multipath,
        past_samps: VecDeque::new(),
        buf: Vec::new(),
    };

    let tx = SimulatedRadioTx { sender };

    (tx, rx)
}
