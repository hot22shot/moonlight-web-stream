
# Moonlight Web
A web client for [Moonlight](https://moonlight-stream.org/) using a hosted web server.

## Limitations
- Requires a host for the web server which is capable of being a Moonlight Client
- Only one active stream per web server

## Installation
TODO

## Building
A valid [Rust](https://www.rust-lang.org/tools/install) installation

### Moonlight Common Sys
[moonlight-common-sys](./moonlight-common-sys/) are rust bindings to the cpp [moonlight-common-c](https://github.com/moonlight-stream/moonlight-common-c) library.

Requires:
- A [cmake installation](https://cmake.org/download/) which will automatically compile the [moonlight-common-c](https://github.com/moonlight-stream/moonlight-common-c) library
- OpenSSL `crypto` library.
  - Download OpenSSL from https://openssl-library.org/source/
  - Build it with the instructions for your system https://github.com/openssl/openssl#build-and-install
  - Set the environment variable `OPENSSL_LIB_DIR` to the directory which contains the `crypto` library

### Moonlight Web
Go into moonlight-web directory `cd moonlight-web`
- Make sure [moonlight-common-sys](#moonlight-common-sys) compiled correctly
- Build the frontend with `npm run build-web`