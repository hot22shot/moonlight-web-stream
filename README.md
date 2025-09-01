
# Moonlight Web
An unofficial [Moonlight](https://moonlight-stream.org/) Client allowing you to use Moonlight in the Web.
It's hosted on a Web Server which will reroute [Sunshine](https://docs.lizardbyte.dev/projects/sunshine/latest/) traffic to a Browser using the [WebRTC Api](https://webrtc.org/) for minimal latency.

## Limitations
- Controllers only work when in a [Secure Context](https://developer.mozilla.org/en-US/docs/Web/Security/Secure_Contexts) because of the [Gamepad Api](https://developer.mozilla.org/en-US/docs/Web/API/Gamepad_API)
  - [How to configure a Secure Context / https](#configuring-https)

## Overview

- [Setup](#setup)
  - [Streaming over the Internet](#streaming-over-the-internet)
  - [Configuring https](#configuring-https)
  - [Proxying via Apache 2](#proxying-via-apache-2)
- [Config](#config)
- [Building](#building)

## Setup

1. Download the [compressed archive](https://github.com/MrCreativ3001/moonlight-web-stream/releases) for your platform and uncompress it or [build it yourself](#building)

2. Run the "web-server" executable

3. Change your [access credentials](#credentials) in the newly generated `server/config.json` (all changes require a restart)

4. Go to `localhost:8080` and view the web interface. You can also the change [bind address](#bind-address).

Add your pc:

1. Add a new pc (<img src="moonlight-web/web-server/web/resources/ic_add_to_queue_white_48px.svg" alt="icon" style="height:1em; vertical-align:middle;">) with the address as `localhost` and leave the port empty (if you've got the default port)

2. Pair your pc by clicking on the host (<img src="moonlight-web/web-server/web/resources/desktop_windows-48px.svg" alt="icon" style="height:1em; vertical-align:middle;">) and entering the code in sunshine

3. Launch an app

### Streaming over the Internet
When in a local network the WebRTC Peers will negotatiate without any problems. However when you want to play over the internet without being in the same network as Moonlight Web, you'll have to configure it and forward ports.

1. Forward the web server port (default is 8080)

2. Set the port range used by the WebRTC Peer to a fixed range in the [config](#config):
```json
{
    ..
    "webrtc_port_range": {
        "min": 40000,
        "max": 40010
    }
    ..
}
```

3. Forward the port range specified in the previous step as `udp` and `tcp` ports.

4. Configure [WebRTC Nat 1 To 1](#webrtc-nat-11-ips) to advertise your [public ip](https://whatismyipaddress.com/) (Optional: WebRTC stun servers can usually automatically detect them):
```json
{
    ..
    "webrtc_nat_1to1": {
        "ice_candidate_type": "host",
        "ips": [
            "74.125.224.72"
        ]
    }
    ..
}
```

If you're using Apache 2, you can also proxy the webpage with it: [Proxying via Apache 2](#proxying-via-apache-2).

### Configuring https
You can configure https directly with the Moonlight Web Server.

1. You'll need a private key and a certificate.

You can generate a self signed certificate with this python script [moonlight-web/web-server/generate_certificate.py](moonlight-web/web-server/generate_certificate.py):

```sh
pip install pyOpenSSL
python ./moonlight-web/web-server/generate_certificate.py
```

2. Copy the files `server/key.pem` and `server/cert.pem` into your `server` directory.

3. Modify the [config](#config) to enable https using the certificates
```json
{
    ..
    "certificate": {
        "private_key_pem": "./server/key.pem",
        "certificate_pem": "./server/cert.pem"
    }
    ..
}
```

### Proxying via Apache 2
It's possible to proxy the Moonlight Website using [Apache 2](https://httpd.apache.org/).

Note:
When you want to use https, the Moonlight Website should use http so that Apache 2 will handle all the https encryption.

1. Enable the modules `mod_proxy`, `mod_proxy_wstunnel`

```sh
sudo a2enmod mod_proxy mod_proxy_wstunnel
```

2. Create a new file under `/etc/apache2/conf-available/moonlight-web.conf` with the content:
```
# Example subpath "/moonlight" -> To connect you'd go to "http://yourip.com/moonlight/"
Define MOONLIGHT_SUBPATH /moonlight
# The address and port of your Moonlight Web server
Define MOONLIGHT_DEV YOUR_LOCAL_IP:YOUR_PORT

ProxyPreserveHost on
        
# Important: This WebSocket will help negotiate the WebRTC Peers
<Location ${MOONLIGHT_SUBPATH}/api/host/stream>
        ProxyPass ws://${MOONLIGHT_DEV}/api/host/stream
        ProxyPassReverse ws://${MOONLIGHT_DEV}/api/host/stream
</Location>

ProxyPass ${MOONLIGHT_SUBPATH}/ http://${MOONLIGHT_DEV}/
ProxyPassReverse ${MOONLIGHT_SUBPATH}/ http://${MOONLIGHT_DEV}/
```

3. Enable the created config file
```sh
sudo a2enconf moonlight-web
```

4. Change [config](#config) to include the [prefixed path](#web-path-prefix)
```json
{
    ..
    "web_path_prefix": "/moonlight"
    ..
}
```

5. Use https with a certificate (Optional)

## Config
The config file is under `server/config.json` relative to the executable.
Here are the most important settings for configuring Moonlight Web.

For a full list of values look into the [Rust Config module](moonlight-web/common/src/config.rs).

### Credentials
The credentials the Website will prompt you to enter.

```json
{
    ..
    "credentials": "default"
    ..
}
```

### Bind Address 
The address and port the website will run on

```json
{
    ..
    "bind_address": "127.0.0.1:8080"
    ..
}
```

### Https Certificates
If enabled the web server will use https with the provided certificate data

```json
{
    ..
    "certificate": {
        "private_key_pem": "./server/key.pem",
        "certificate_pem": "./server/cert.pem"
    }
    ..
}
```

### WebRTC Port Range
This will set the port range on the web server used to communicate when using WebRTC

```json
{
    ..
    "webrtc_port_range": {
        "min": 40000,
        "max": 40010
    }
    ..
}
```

### WebRTC Ice Servers
A list of ice server for webrtc to use.
Is force set to empty if WebRTC Nat 1 To 1 Ice Candidate Type is set to host.

```json
{
    ..
    "webrtc_ice_servers": [
        {
            "urls": [
                    "stun:l.google.com:19302",
                    "stun:stun.l.google.com:19302",
                    "stun:stun1.l.google.com:19302",
                    "stun:stun2.l.google.com:19302",
                    "stun:stun3.l.google.com:19302",
                    "stun:stun4.l.google.com:19302",
            ]
        }
    ]
    ..
}
```

### WebRTC Nat 1 to 1 ips
This will advertise the ip as an ice candidate on the web server.
It's recommended to set this but stun servers should figure out the public ip.
Is force set the WebRTC ice servers to empty if the Ice Candidate Type is set to host.

- host -> This is the ip address of the server and the client can connect to
- srflx -> This is the public ip address of this server, like a ice candidate added from a stun server.

```json
{
    ..
    "webrtc_nat_1to1": {
        "ice_candidate_type": "host", // "srflx" or "host"
        "ips": [
            "74.125.224.72"
        ]
    }
    ..
}
```

### WebRTC Network Types
This will set the network types allowed by webrtc.
<br>Allowed values:
- udp4: All udp with ipv4
- udp6: All udp with ipv6
- tcp4: All tcp with ipv4
- tcp6: All tcp with ipv6

```json
{
    ..
    "webrtc_network_types": [
        "udp4",
        "udp6",
    ]
    ..
}
```

### Web Path Prefix
This is useful when rerouting the web page using services like [Apache 2](#proxying-via-apache-2).
Will always append the prefix to all requests made by the website.

```json
{
    ..
    "web_path_prefix": "/moonlight"
    ..
}
```

## Building
Make sure you've cloned this repo with all it's submodules
```sh
git clone --recursive https://github.com/MrCreativ3001/moonlight-web-stream.git
```
A [Rust](https://www.rust-lang.org/tools/install) [nightly](https://rust-lang.github.io/rustup/concepts/channels.html) installation is required.

There are 2 ways to build Moonlight Web:
- Build it on your system

  When you want to build it on your system take a look at how to compile the crates:
  - [moonlight common sys](#crate-moonlight-common-sys)
  - [moonlight web](#crate-moonlight-web)

- Compile using [Cargo Cross](https://github.com/cross-rs/cross)

  After you've got a successful installation of cross just run the command in the project root directory
  ```sh
  cross build --release --target YOUR_TARGET
  ```
  Note: windows only has the gnu target `x86_64-pc-windows-gnu`

### Crate: Moonlight Common Sys
[moonlight-common-sys](./moonlight-common-sys/) are rust bindings to the cpp [moonlight-common-c](https://github.com/moonlight-stream/moonlight-common-c) library.

Requires:
- A [CMake installation](https://cmake.org/download/) which will automatically compile the [moonlight-common-c](https://github.com/moonlight-stream/moonlight-common-c) library
- [openssl-sys](https://docs.rs/openssl-sys/0.9.109/openssl_sys/): For information on building openssl sys go to the [openssl docs](https://docs.rs/openssl/latest/openssl/)

### Crate: Moonlight Web
This is the main Moonlight Web project

Required:
- [moonlight-common-sys](#moonlight-common-sys)
- [npm](https://docs.npmjs.com/downloading-and-installing-node-js-and-npm)

Build the executables in the root directory with (builds streamer and web-server):
```sh
cargo build --release
```

Build the web frontend with:
```sh
npm run install
npm run build
```
The build output will be in `moonlight-web/dist`. The dist folder needs to be called `static` and in the same directory as the executable.