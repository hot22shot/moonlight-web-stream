
# Moonlight Web
A web client for [Moonlight](https://moonlight-stream.org/) using a hosted web server and a [WebRTC Media Stream](https://webrtc.org/).

## Limitations
- Requires a host for the web server which is capable of being a Moonlight Client (compiling [moonlight-common-sys](#moonlight-common-sys))
- Only one active stream per web server

## Installation
TODO

If you're already running a service like [Apache 2](https://httpd.apache.org/) you can [proxy the request to your Moonlight server](#proxying-via-apache-2)

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

## Proxying via Apache 2
If you want to host this on a path on your apache 2 server you'll need to do these steps:

1) Enable the modules `mod_proxy`, `mod_proxy_wstunnel`

`sudo a2enmod mod_proxy mod_proxy_wstunnel`

2) Create a new file under `/etc/apache2/conf-available/moonlight-web.conf` with the content:
```
# Example subpath "/moonlight" -> To connect you'd go to "http://yourip.com/moonlight/"
Define MOONLIGHT_SUBPATH /moonlight
# The address and port of your Moonlight Web server
Define MOONLIGHT_DEV YOUR_LOCAL_IP:YOUR_PORT

ProxyPreserveHost on
        
<Location ${MOONLIGHT_SUBPATH}/api/host/stream>
        ProxyPass ws://${MOONLIGHT_DEV}/api/host/stream
        ProxyPassReverse ws://${MOONLIGHT_DEV}/api/host/stream
</Location>

ProxyPass ${MOONLIGHT_SUBPATH}/ http://${MOONLIGHT_DEV}/
ProxyPassReverse ${MOONLIGHT_SUBPATH}/ http://${MOONLIGHT_DEV}/
```
Enable it with `sudo a2enconf moonlight-web`.

3) Change config to include the prefixed path
TODO: LINK TO CONFIG

## Interesting
- WebRTC Signaling: https://developer.mozilla.org/en-US/docs/Web/API/WebRTC_API/Signaling_and_video_calling