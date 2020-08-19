#include <complex>
#include <iostream>
#include <vector>

#include <uhd/usrp/multi_usrp.hpp>
#include <uhd/utils/thread_priority.hpp>

#include "wrapper.hpp"

// A multi-USRP object
struct MultiUsrp {
  uhd::usrp::multi_usrp::sptr usrp;
};

// A receive stream
struct RxStream {
  uhd::rx_streamer::sptr streamer;
  // To hold values temporarily
  std::complex<float>* buf;
  size_t buf_len;
};

// A transmit stream
struct TxStream {
  uhd::tx_streamer::sptr streamer;
  std::complex<float>* buf;
  size_t buf_len;
};

using namespace std;

// Create and do basic configuration of a multi-USRP object. `rate` is in
// samples per second, `freq` is the center frequency in hertz, `gain` is in dB
// in uncalibrated units, `bw` is the analog bandwidth in Hz and `tx` indicates
// whether it is a transmit or a receive. `addr` is the uhd representation, and
// controls the number of channels.
MultiUsrp* new_usrp(char* args, double rate, double freq,
  double gain, double bw, bool tx) {
  uhd::set_thread_priority_safe();
  assert(sizeof(float) == 4); // Because in Rust, we use f32

  // Create a usrp device
  uhd::usrp::multi_usrp::sptr usrp;
  try {
    usrp = uhd::usrp::multi_usrp::make(string(args));
  }
  catch (const uhd::key_error& e) {cerr << "No device found." << endl; return 0;}
  catch (const uhd::index_error & e) {cerr << "Fewer devices found than expected." << endl; return 0;}
  catch (...) {cerr << "Unknown exception" << endl; return 0;}

  cout << "Using Device: " << usrp->get_pp_string() << endl;
  // Call different functions depending on whether we are transmitting or
  // receiving
  if (tx) {
    // Set configurations per channel. If more control over the mapping of
    // subdevices over channels is desired, the call `set_tx_subdev_spec` and
    // `set_rx_subdev_spec`
    for (size_t chan = 0; chan < usrp->get_tx_num_channels(); ++chan) {
      cout << "Num Tx Channels: " << usrp->get_tx_num_channels() << endl;

      // Set sample rate
      usrp->set_tx_rate(rate, chan);
      cout << "Actual TX Rate: " << usrp->get_tx_rate(chan)/1e6 << " Msps" << endl << endl;

      // Set frequency. Don't use manual LO offset since it was creating
      // problems
      uhd::tune_request_t tune_request(freq);
      usrp->set_tx_freq(tune_request, chan);
      cout << "Actual TX Freq: " << usrp->get_tx_freq(chan)/1e6 << " MHz" << endl << endl;

      // Set Rx gain
      usrp->set_tx_gain(gain, chan);
      cout << "Actual Tx Gain: " << usrp->get_tx_gain(chan) << " dB" << endl << endl;

      // Set analog bandwidth
      usrp->set_tx_bandwidth(bw, chan);
      cout << "Actual Tx Bandwidth: " << usrp->get_tx_bandwidth(chan)/1e6 << " MHz" << endl << endl;

      // Set this antenna by default
      usrp->set_tx_antenna("TX/RX", chan);
      cout << "Actual TX Antenna: " << usrp->get_tx_antenna(chan) << endl;

    }
  }
  else {
    // Set configurations per channel. If more control over the mapping of
    // subdevices over channels is desired, the call `set_tx_subdev_spec` and
    // `set_rx_subdev_spec`
    for (size_t chan = 0; chan < usrp->get_rx_num_channels(); ++chan) {
      cout << "Num Rx Channels: " << usrp->get_rx_num_channels() << endl;

      // Set sample rate
      usrp->set_rx_rate(rate, chan);
      cout << "Actual RX Rate: " << usrp->get_rx_rate(chan)/1e6 << " Msps" << endl << endl;

      // Set frequency and LO offset (to avoid DC offset)
      uhd::tune_request_t tune_request(freq);
      // Integer (vs. fractional) mode sacrifices tuning accuracy for lower
      // spurs (spurious side frequencies)
      tune_request.args = uhd::device_addr_t("mode_n=integer");
      usrp->set_rx_freq(tune_request, chan);
      cout << "Actual RX Freq: " << usrp->get_rx_freq(chan)/1e6 << " MHz" << endl << endl;

      // Turn off device's AGC
      usrp->set_rx_agc(false, chan);

      // Set Rx gain
      usrp->set_rx_gain(gain, chan);
      cout << "Actual Rx Gain: " << usrp->get_rx_gain(chan) << " dB" << endl << endl;

      // Set analog bandwidth
      usrp->set_rx_bandwidth(bw, chan);
      cout << "Actual Rx Bandwidth: " << usrp->get_rx_bandwidth(chan)/1e6 << " MHz" << endl << endl;

      // Set this antenna by default
      usrp->set_rx_antenna("TX/RX", chan);
      cout << "Actual RX Antenna: " << usrp->get_rx_antenna(chan) << endl;
    }
  }

  // TODO: Check lo_locked and throw an error if it isn't

  MultiUsrp* res = new MultiUsrp;
  res->usrp = usrp;
  return res;
}

// Set the clock source (and time source) for this motherboard. `source` is an
// enum. 0: "internal", 1: "mimo". Return 0 if success.
int32_t set_clock_source(MultiUsrp* usrp, uint8_t source, size_t mboard) {
  cout << "Available clock sources: [";
  for (auto s : usrp->usrp->get_clock_sources(mboard)) {
    cout << s << ", ";
  }
  cout << "]" << endl;
  switch (source) {
    case 0:
      usrp->usrp->set_clock_source("internal", mboard);
      break;
      //usrp->usrp->set_time_source("internal", mboard);
    case 1:
      usrp->usrp->set_clock_source("mimo", mboard);
      break;
      //usrp->usrp->set_time_source("mimo", mboard);
    case 2:
      usrp->usrp->set_clock_source("external", mboard);
      break;
    case 3:
      usrp->usrp->set_clock_source("gpsdo", mboard);
      break;
    default:
      return -1;
  }
  cout << "Actual Clock Source: " << usrp->usrp->get_clock_source(mboard) << " " << (int)source << endl;
  return 0;
}

// Set the gain (in uncaliberated dB) for the receiver for all channels
void set_rx_gain(MultiUsrp* usrp, double gain) {
  for (size_t chan = 0; chan < usrp->usrp->get_rx_num_channels(); ++chan)
    usrp->usrp->set_rx_gain(gain, chan);
}

// Set the gain (in uncaliberated dB) for the receiver for the given channel
double get_rx_gain(MultiUsrp* usrp, size_t channel) {
  return usrp->usrp->get_rx_gain(channel);
}

void set_tx_freq(MultiUsrp* usrp, double freq) {
  uhd::tune_request_t tune_request(freq);
  usrp->usrp->set_tx_freq(tune_request, 0);
}

void set_rx_freq(MultiUsrp* usrp, double freq) {
  uhd::tune_request_t tune_request(freq);
  usrp->usrp->set_rx_freq(tune_request, 0);
}

// Get an Rx streamer from the USRP
RxStream* get_rx_streamer(MultiUsrp* usrp) {
  // Make the channels (in some arbitrary order)
  vector<size_t> channels;
  for (size_t i = 0; i < usrp->usrp->get_rx_num_channels(); ++i)
    channels.push_back(i);

  // Create a stream argument
  uhd::stream_args_t stream_args("fc32"); // Complex floats
  stream_args.channels = channels;

  // Create the streamer
  uhd::rx_streamer::sptr rx_stream = usrp->usrp->get_rx_stream(stream_args);

  // Start streaming
  uhd::stream_cmd_t stream_cmd(uhd::stream_cmd_t::STREAM_MODE_START_CONTINUOUS);
  // Start immediately
  stream_cmd.stream_now = true;
  // Issue command
  rx_stream->issue_stream_cmd(stream_cmd);

  RxStream* res = new RxStream;
  res->streamer = rx_stream;
  res->buf = nullptr;
  res->buf_len = 0;
  return res;
}

// Get a Tx streamer from the USRP
TxStream* get_tx_streamer(MultiUsrp* usrp) {
  // Make the channels (in some arbitrary order)
  vector<size_t> channels;
  for (size_t i = 0; i < usrp->usrp->get_tx_num_channels(); ++i)
    channels.push_back(i);
  // Create a stream argument
  uhd::stream_args_t stream_args("fc32"); // Complex floats
  stream_args.channels = channels;

  // Create the streamer
  uhd::tx_streamer::sptr tx_stream = usrp->usrp->get_tx_stream(stream_args);

  TxStream* res = new TxStream;
  res->streamer = tx_stream;
  res->buf = nullptr;
  res->buf_len = 0;
  return res;
}

// Immediately reset the time (on all motherboards) to the given time `now` (in
// seconds)
void set_time_now(MultiUsrp* usrp, double now) {
  usrp->usrp->set_time_now(now);
}

// Get `num_samples` number of elements from the stream, one for each channel.
// So `buf` must be at-least `2 * num_samples * num_channels` long. We require
// `num_channels` simply so this function can check that the caller and streamer
// have the same notion of what the number of channels is. If the returned value
// is negative, this indicates an error code. Else it is a time_spec (in
// microseconds).

// Elements [2 * n * num_samples, 2 * (n + 1) * num_samples - 1] contain the
// samples for the n^th channel. Within a channel, elements (2*i, 2*i+1) contain
// the real and imaginary parts of the i^th sample.

// This function is *not* thread safe
int64_t recv(RxStream* streamer, float* buf, size_t num_samples,
  size_t num_channels) {
  // We are assuming `float` is a 32 bit value
  if (sizeof(float) != 4)
    return -2;
  // Check that caller and streamer have same notion of the number of channels
  if (streamer->streamer->get_num_channels() != num_channels)
    return -3;

  // Ensure `stream->buf` has enough space
  if (streamer->buf_len < num_samples * num_channels) {
    if (streamer->buf != nullptr)
      delete[] streamer->buf;
    streamer->buf = new complex<float>[num_samples * num_channels];
    streamer->buf_len = num_samples * num_channels;
  }

  // Create the vector of buffers, each pointing to an appropriate place in
  // `streamer->buf`
  vector<complex<float>*> buffs;
  for (size_t i = 0; i < num_channels; ++i)
    buffs.push_back(streamer->buf + (i * num_samples));

  uint64_t time_spec = 0;
  uhd::rx_metadata_t md;
  size_t num_recvd = 0;
  while (num_recvd < num_samples) {
    // Get the data
    size_t num_new_recvd = streamer->streamer->recv(
      buffs,
      num_samples - num_recvd,
      md, 1.0);
    num_recvd += num_new_recvd;

    // Update our buffers
    for (size_t i = 0; i < num_channels; ++i)
      buffs[i] += num_new_recvd;

    // Parse error code
    switch (md.error_code) {
      case uhd::rx_metadata_t::ERROR_CODE_NONE:
        break; // Yay!
      case uhd::rx_metadata_t::ERROR_CODE_TIMEOUT:
        // Timed out, no packets received from USRP
        return -4;
      case uhd::rx_metadata_t::ERROR_CODE_LATE_COMMAND:
        return -5;
      case uhd::rx_metadata_t::ERROR_CODE_BROKEN_CHAIN:
        return -6;
      case uhd::rx_metadata_t::ERROR_CODE_OVERFLOW:
        return -7;
      case uhd::rx_metadata_t::ERROR_CODE_ALIGNMENT:
        return -8;
      case uhd::rx_metadata_t::ERROR_CODE_BAD_PACKET:
        return -9;
      default:
        return -10; // Weird
    }

    if (num_new_recvd == 0) {
      // Weird, we should have got some error code
      return -11;
    }
    if (!md.has_time_spec)
      return -12;
    if (md.out_of_sequence)
      return -13;

    // If this is the first round, take the time_spec
    if (time_spec == 0)
      time_spec = md.time_spec.to_ticks(1000000);
  }

  assert(num_recvd == num_samples);
  // Copy data into `buf`
  for (size_t chan = 0; chan < num_channels; ++ chan) {
    for (size_t i = 0; i < num_recvd; ++i) {
      auto val = streamer->buf[chan * num_samples + i];
      buf[chan * 2 * num_samples + 2 * i] = val.real();
      buf[chan * 2 * num_samples + 2 * i + 1] = val.imag();
    }
  }

  return (int64_t)time_spec;
}

// Send `num_samples` in `buf` to the Tx usrp. Elements (2*i, 2*i+1) are the
// real and imaginary parts of the i^th sample respectively. Currently only a
// single channel transmission is supported. Return value is negative in case of
// an error
int32_t send(TxStream* streamer, float* buf, size_t num_samples) {
  // Check that there is only a single channel
  if (streamer->streamer->get_num_channels() != 1)
    return -2;

  // Ensure `stream->buf` has enough space
  if (streamer->buf_len < num_samples) {
    if (streamer->buf != nullptr)
      delete[] streamer->buf;
    streamer->buf = new complex<float>[num_samples];
    streamer->buf_len = num_samples;
  }

  // Copy data into streamer->buf
  for (size_t i = 0; i < num_samples; ++i)
    streamer->buf[i] = complex<float>(buf[2*i], buf[2*i + 1]);

  // Dummy metadata
  uhd::tx_metadata_t md;
  // Dummy set of buffers
  vector<complex<float>*> buffs;
  buffs.push_back(streamer->buf);

  // Send the data
  size_t num_sent = 0;
  while (num_sent < num_samples) {
    size_t num_new_sent = streamer->streamer->send(buffs, num_samples, md, 0.1);
    num_sent += num_new_sent;
    buffs[0] += num_new_sent;

    if (num_new_sent == 0) {
      // Probably timed out before we could send any packets
      return -3;
    }
  }

  return 0;
}

void delete_usrp(MultiUsrp* usrp) {
  // The sptr's destructor should be called automatically
  delete usrp;
}

void delete_rx_stream(RxStream* streamer) {
  delete[] streamer->buf;
  delete streamer;
}

void delete_tx_stream(TxStream* streamer) {
  delete[] streamer->buf;
  delete streamer;
}
