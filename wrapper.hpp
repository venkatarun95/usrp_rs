// See wrapper.cpp for documentation on what the functions do. This file only
// contains definitions in a bindgen friendly format

// For `size_t`, `uint32_t` etc.
#include <cstdint>
#include <cstdlib>

struct MultiUsrp;
struct RxStream;
struct TxStream;

MultiUsrp* new_usrp(char* args, double rate, double freq, double gain,
  double bw, bool tx);
int32_t set_clock_source(MultiUsrp* usrp, uint8_t source, size_t mboard);
void set_rx_gain(MultiUsrp* usrp, double gain);
double get_rx_gain(MultiUsrp* usrp, size_t chan);
void set_tx_freq(MultiUsrp* usrp, double freq);
void set_rx_freq(MultiUsrp* usrp, double freq);
void set_time_now(MultiUsrp* usrp, double now);
RxStream* get_rx_streamer(MultiUsrp* usrp);
TxStream* get_tx_streamer(MultiUsrp* usrp);
int64_t recv(RxStream* streamer, float* buf, size_t num_samples,
  size_t num_channels);
int32_t send(TxStream* streamer, float* buf, size_t num_samples);
void delete_usrp(MultiUsrp* usrp);
void delete_rx_stream(RxStream* streamer);
void delete_tx_stream(TxStream* streamer);
