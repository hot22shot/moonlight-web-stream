
# Moonlight Web
A web client for [Moonlight](https://moonlight-stream.org/) using a hosted web server and a [WebRTC Media Stream](https://webrtc.org/).

## Limitations
- Requires a host for the web server which is capable of being a Moonlight Client (compiling [moonlight-common-sys](#moonlight-common-sys))
- Only one active stream per web server

## Installation
TODO

## Building
A valid [Rust](https://www.rust-lang.org/tools/install) installation

### Moonlight Common Sys
[moonlight-common-sys](./moonlight-common-sys/) are rust bindings to the cpp [moonlight-common-c](https://github.com/moonlight-stream/moonlight-common-c) library.

Requires:
- A [CMake installation](https://cmake.org/download/) which will automatically compile the [moonlight-common-c](https://github.com/moonlight-stream/moonlight-common-c) library
- [openssl-sys](https://docs.rs/openssl-sys/0.9.109/openssl_sys/): For information on building openssl sys go to the [openssl docs](https://docs.rs/openssl/latest/openssl/)

### Moonlight Web
Go into moonlight-web directory `cd moonlight-web`
- Make sure [moonlight-common-sys](#moonlight-common-sys) compiled correctly
- Build the frontend with `npm run build-web`

## Interesting
- WebRTC Signaling: https://developer.mozilla.org/en-US/docs/Web/API/WebRTC_API/Signaling_and_video_calling