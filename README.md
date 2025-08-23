
# Moonlight Web
An unofficial [Moonlight](https://moonlight-stream.org/) Client allowing you to use Moonlight in the Web.
It's hosted on a Web Server which will reroute [Sunshine](https://docs.lizardbyte.dev/projects/sunshine/latest/) traffic to a Browser using the [WebRTC Api](https://webrtc.org/) for minimal latency.

## Limitations
- Only one active stream per web server
- Controllers only work when in a [Secure Context](https://developer.mozilla.org/en-US/docs/Web/Security/Secure_Contexts) because of the [Gamepad Api](https://developer.mozilla.org/en-US/docs/Web/API/Gamepad_API)
  - [How to configure a Secure Context / https](#configuring-https)

## Setup
1. Download the [compressed archive](TODO:LINK) for your platform and uncompress it or [build it yourself](#building)

2. Run the executable

3. Change your [access credentials](#credentials) in the newly generated `server/config.json` (all changes require a restart)

4. Go to `localhost:8080` and view the web interface. You can also the change [bind address](#bind-address).

More optional configurations:
- [Streaming over the Internet](#streaming-over-the-internet)
- [Configuring https](#configuring-https)
- [Proxying via Apache 2](#proxying-via-apache-2)

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

4. Configure WebRTC to advertise your [public ip](https://whatismyipaddress.com/) (Optional: WebRTC Stun will automatically detect them):
```json
{
    ..
    "webrtc_nat_1to1_ips": "74.125.224.72"
    ..
}
```

If you're Apache 2, you can also proxy the webpage with it: [Proxying via Apache 2](#proxying-via-apache-2).

### Configuring https
You can configure https directly with the Moonlight Web Server.

1. You'll need a private key and a certificate.

You can generate a self signed certificate with this python script [moonlight-web/generate_certificate.py](moonlight-web/generate_certificate.py):

```sh
pip install pyOpenSSL
python ./moonlight-web/generate_certificate.py
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

4. Change config to include the prefixed path
TODO: LINK TO CONFIG

5. Use https with a certificate (Optional)

## Config
The config file is under `server/config.json` relative to the executable.
Here are the most important settings for configuring [Moonlight Web](#moonlight-web).

For a full list of values look into the [Rust Config module](moonlight-web/src/config.rs).

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

### WebRTC Ip
This will advertise the ip as a ice candidate on the web server.
This is mostly optional because stun server can figure out the public ip.

```json
{
    ..
    "webrtc_nat_1to1_ips": "74.125.224.72"
    ..
}
```

### Web Path Prefix
This is useful when rerouting the web page using services like [Apache 2](#proxying-via-apache-2).
Will always append the prefix to all requests made by the website.

```json
{
    ..
    "web_path_prefix": "/test"
    ..
}
```

## Building
Make sure you've cloned this repo with all it's submodules
```sh
git clone --recursive TODO:URL
```
A [Rust](https://www.rust-lang.org/tools/install) [nightly](https://rust-lang.github.io/rustup/concepts/channels.html) installation is required.

There are 2 ways to build [Moonlight Web](#moonlight-web):
- Build it on your system

  When you want to build it on your system take a look at how to compile the crates:
  - [moonlight common sys](#crate-moonlight-common-sys)
  - [moonlight web](#crate-moonlight-web)

- Compile using [Cargo Cross](https://github.com/cross-rs/cross)

  After you've got a successful installation of cross just run the command in the [moonlight web](moonlight-web/) directory
  ```sh
  cross build --release --target YOUR_TARGET
  ```

### Crate: Moonlight Common Sys
[moonlight-common-sys](./moonlight-common-sys/) are rust bindings to the cpp [moonlight-common-c](https://github.com/moonlight-stream/moonlight-common-c) library.

Requires:
- A [CMake installation](https://cmake.org/download/) which will automatically compile the [moonlight-common-c](https://github.com/moonlight-stream/moonlight-common-c) library
- [openssl-sys](https://docs.rs/openssl-sys/0.9.109/openssl_sys/): For information on building openssl sys go to the [openssl docs](https://docs.rs/openssl/latest/openssl/)

Build with:
```sh
cargo build --release
```

### Crate: Moonlight Web
This is the main Moonlight Web project

Required:
- [moonlight-common-sys](#moonlight-common-sys)
- [npm](https://docs.npmjs.com/downloading-and-installing-node-js-and-npm)

Build the executable with:
```sh
cargo build --release
```

Build the web frontend with:
```sh
npm run install
npm run build
```
The build output will be in `moonlight-web/dist`. The dist folder needs to be called `static` and in the same directory as the executable.